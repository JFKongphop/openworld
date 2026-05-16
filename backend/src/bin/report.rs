//! OpenWorld Report Viewer — renders travel reports beautifully in the terminal

use std::fs;
use std::path::{Path, PathBuf};
use std::env;

const RESET:      &str = "\x1b[0m";
const BOLD:       &str = "\x1b[1m";
const DIM:        &str = "\x1b[2m";
const ITALIC:     &str = "\x1b[3m";
const FG_WHITE:   &str = "\x1b[97m";
const FG_CYAN:    &str = "\x1b[96m";
const FG_GREEN:   &str = "\x1b[92m";
const FG_YELLOW:  &str = "\x1b[93m";
const FG_BLUE:    &str = "\x1b[94m";
const FG_MAGENTA: &str = "\x1b[95m";
const FG_GRAY:    &str = "\x1b[90m";
const BG_DARK:    &str = "\x1b[48;5;235m";
const BG_CODE:    &str = "\x1b[48;5;236m";

fn term_width() -> usize {
    env::var("COLUMNS").ok().and_then(|s| s.parse().ok()).unwrap_or(100).max(60)
}

fn hr(ch: char, n: usize) -> String { std::iter::repeat(ch).take(n).collect() }

fn center_str(text: &str, width: usize) -> String {
    let vis = visible_len(text);
    if vis >= width { return text.to_string(); }
    format!("{}{}", " ".repeat((width - vis) / 2), text)
}

fn visible_len(s: &str) -> usize {
    let mut n = 0usize;
    let mut in_esc = false;
    for c in s.chars() {
        if in_esc { if c == 'm' { in_esc = false; } }
        else if c == '\x1b' { in_esc = true; }
        else { n += 1; }
    }
    n
}

fn char_display_width(c: char) -> usize {
    let cp = c as u32;
    // Truly zero-width: ZWJ, combining diacritical marks
    if cp == 0x200D || (cp >= 0x20D0 && cp <= 0x20FF) || cp == 0xFEFF
        || (cp >= 0xFE00 && cp <= 0xFE0E) {
        return 0;
    }
    // VS-16 (emoji presentation selector): makes preceding narrow symbol wide,
    // so we give it width 1 here (the base char stays at 1, total = 2)
    if cp == 0xFE0F { return 1; }

    // Wide CJK + Emoji ranges (East Asian Width = W or F)
    if (cp >= 0x1100 && cp <= 0x115F)    // Hangul Jamo lead
        || (cp >= 0x2E80 && cp <= 0x303F) // CJK Radicals, Kangxi, CJK Symbols & Punct
        || (cp >= 0x3041 && cp <= 0xA4CF) // Hiragana..Yi Radicals (Katakana, Bopomofo, Hangul Compat, CJK, etc.)
        || (cp >= 0xA960 && cp <= 0xA97F) // Hangul Jamo Extended-A
        || (cp >= 0xAC00 && cp <= 0xD7FF) // Hangul Syllables + Extended-B
        || (cp >= 0xF900 && cp <= 0xFAFF) // CJK Compat Ideographs
        || (cp >= 0xFE10 && cp <= 0xFE1F) // Vertical Forms
        || (cp >= 0xFE30 && cp <= 0xFE4F) // CJK Compat Forms
        || (cp >= 0xFF01 && cp <= 0xFF60) // Fullwidth Latin/punct
        || (cp >= 0xFFE0 && cp <= 0xFFE6) // Fullwidth signs
        || cp >= 0x1F300                   // All emoji blocks (1F300+)
    {
        return 2;
    }
    1
}

fn find_close(text: &str, start: usize, pat: &str) -> Option<usize> {
    text[start..].find(pat).map(|p| start + p)
}

// Convert [label](url) → "label ↗"
fn simplify_links(text: &str) -> String {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut out = String::new();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'[' {
            if let Some(te) = find_close(text, i + 1, "]") {
                let after = te + 1;
                if after < len && bytes[after] == b'(' {
                    if let Some(ue) = find_close(text, after + 1, ")") {
                        out.push_str(&text[i+1..te]);
                        out.push_str(" ↗");
                        i = ue + 1;
                        continue;
                    }
                }
            }
        }
        let c = text[i..].chars().next().unwrap();
        out.push(c);
        i += c.len_utf8();
    }
    out
}

// Strip all markdown → plain visible text
fn to_plain(s: &str) -> String {
    let s = simplify_links(s);
    let s = s.replace("***", "").replace("**", "").replace('*', "");
    let mut out = String::new();
    let mut in_code = false;
    for c in s.chars() {
        if c == '`' { in_code = !in_code; } else { out.push(c); }
    }
    out.trim().to_string()
}

fn truncate_plain(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let disp: usize = chars.iter().map(|&c| char_display_width(c)).sum();
    if disp <= max { return s.to_string(); }
    let mut out = String::new();
    let mut used = 0usize;
    for &c in &chars {
        let w = char_display_width(c);
        if used + w + 1 > max { break; } // leave room for …
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

// Render inline markup (for headings, paragraphs — not table cells)
fn render_inline(text: &str) -> String {
    let text = simplify_links(text);
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut out = String::new();
    let mut i = 0;
    while i < len {
        if i + 2 < len && &bytes[i..i+3] == b"***" {
            if let Some(end) = find_close(&text, i+3, "***") {
                out.push_str(BOLD); out.push_str(ITALIC);
                out.push_str(&render_inline(&text[i+3..end]));
                out.push_str(RESET); i = end + 3; continue;
            }
        }
        if i + 1 < len && &bytes[i..i+2] == b"**" {
            if let Some(end) = find_close(&text, i+2, "**") {
                out.push_str(BOLD); out.push_str(FG_WHITE);
                out.push_str(&render_inline(&text[i+2..end]));
                out.push_str(RESET); i = end + 2; continue;
            }
        }
        if bytes[i] == b'*' {
            if let Some(end) = find_close(&text, i+1, "*") {
                out.push_str(ITALIC); out.push_str(FG_CYAN);
                out.push_str(&render_inline(&text[i+1..end]));
                out.push_str(RESET); i = end + 1; continue;
            }
        }
        if bytes[i] == b'`' {
            if let Some(end) = find_close(&text, i+1, "`") {
                out.push_str(BG_CODE); out.push_str(FG_YELLOW);
                out.push(' '); out.push_str(&text[i+1..end]); out.push(' ');
                out.push_str(RESET); i = end + 1; continue;
            }
        }
        if i + 1 < len && &bytes[i..i+2] == b"~~" {
            if let Some(end) = find_close(&text, i+2, "~~") {
                out.push_str(DIM); out.push_str(FG_GRAY);
                out.push_str(&text[i+2..end]);
                out.push_str(RESET); i = end + 2; continue;
            }
        }
        let c = text[i..].chars().next().unwrap();
        out.push(c); i += c.len_utf8();
    }
    out
}

// Render a table cell: plain text, truncated, with colour hints
fn render_cell(raw: &str, max_w: usize, is_header: bool) -> (String, usize) {
    let plain = to_plain(raw);
    // Replace standalone null placeholders with em-dash
    let plain = if plain.trim() == "null" || plain.contains("→ null") || plain.contains("null →") {
        plain.replace("null", "—")
    } else { plain };
    // Collapse "confirmed", "complete" (with or without ✅) → just ✅
    // Collapse "failed", "error" (with or without ❌) → just ❌
    let plain = {
        let lower = plain.to_lowercase();
        if lower.contains("confirmed") || lower.contains("complete") {
            "✅".to_string()
        } else if lower.contains("failed") || lower.contains("error") {
            "❌".to_string()
        } else {
            plain
        }
    };
    let trunc = truncate_plain(&plain, max_w);
    // Use display-column width (emoji = 2 columns)
    let vis: usize = trunc.chars().map(char_display_width).sum();

    let rendered = if is_header {
        format!("{BOLD}{FG_CYAN}{trunc}{RESET}")
    } else if trunc.contains('✅') {
        format!("{FG_GREEN}{trunc}{RESET}")
    } else if trunc.contains('❌') {
        format!("\x1b[91m{trunc}{RESET}")
    } else if trunc.starts_with('$') {
        format!("{FG_YELLOW}{trunc}{RESET}")
    } else if trunc.starts_with("0x") {
        format!("{FG_CYAN}{trunc}{RESET}")
    } else if trunc.contains("OW-") {
        format!("{FG_MAGENTA}{trunc}{RESET}")
    } else if trunc.contains('↗') {
        format!("{FG_BLUE}{trunc}{RESET}")
    } else {
        trunc.clone()
    };
    (rendered, vis)
}

fn render_table(rows: &[Vec<String>], width: usize) {
    if rows.is_empty() { return; }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 { return; }

    // Natural widths from plain content (display columns, emoji = 2)
    let cell_display_w = |cell: &str| -> usize {
        to_plain(cell).chars().map(char_display_width).sum()
    };
    let mut col_w: Vec<usize> = vec![3; cols];
    for row in rows {
        for (c, cell) in row.iter().enumerate() {
            let is_sep = { let t = cell.trim(); !t.is_empty() && t.chars().all(|ch| ch=='-'||ch==':') };
            if !is_sep { col_w[c] = col_w[c].max(cell_display_w(cell)); }
        }
    }

    // Fit to terminal: borders(cols+1) + spaces(cols*2) + content
    let overhead = cols * 3 + 1;
    let avail    = width.saturating_sub(overhead);
    let total: usize = col_w.iter().sum();
    if total > avail && avail > 0 {
        let scale = avail as f64 / total as f64;
        col_w = col_w.iter().map(|&w| ((w as f64 * scale).floor() as usize).max(4)).collect();
    }

    let sep = |l: char, m: char, s: char, r: char| -> String {
        let mut o = String::new();
        o.push(l);
        for (i, &w) in col_w.iter().enumerate() {
            for _ in 0..w+2 { o.push(m); }
            o.push(if i+1 < cols { s } else { r });
        }
        o
    };

    println!("{FG_GRAY}{}{RESET}", sep('┌','─','┬','┐'));

    // If the first (header) row has all-empty cells, skip it and its separator
    let first_row = rows.iter().find(|r| {
        !r.iter().all(|c| { let t = c.trim(); !t.is_empty() && t.chars().all(|ch| ch=='-'||ch==':') })
    });
    let header_is_empty = first_row
        .map(|r| r.iter().all(|c| to_plain(c).is_empty()))
        .unwrap_or(false);

    let mut printed_header = false;
    let mut skip_next_sep  = header_is_empty;
    for row in rows {
        let is_sep_row = row.iter().all(|c| {
            let t = c.trim(); !t.is_empty() && t.chars().all(|ch| ch=='-'||ch==':')
        });
        if is_sep_row {
            if skip_next_sep {
                skip_next_sep  = false;
                printed_header = true;
            } else {
                println!("{FG_GRAY}{}{RESET}", sep('├','─','┼','┤'));
                printed_header = true;
            }
            continue;
        }

        let is_header = !printed_header;
        // Skip the empty header row itself
        if is_header && header_is_empty {
            printed_header = true;
            continue;
        }
        print!("{FG_GRAY}│{RESET}");
        for (c, cell) in row.iter().enumerate() {
            let w = col_w[c];
            let (rendered, vis) = render_cell(cell, w, is_header);
            let pad = w.saturating_sub(vis);
            if is_header {
                print!(" {rendered}{} {FG_GRAY}│{RESET}", " ".repeat(pad));
            } else {
                print!(" {rendered}{} {FG_GRAY}│{RESET}", " ".repeat(pad));
            }
        }
        println!();
    }

    println!("{FG_GRAY}{}{RESET}", sep('└','─','┴','┘'));
}

fn render_code_block(lines: &[&str]) {
    for line in lines {
        println!("  {FG_YELLOW}{line}{RESET}");
    }
}

fn render_markdown(content: &str) {
    let width = term_width();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line    = lines[i];
        let trimmed = line.trim();

        // Code block
        if trimmed.starts_with("```") {
            let lang = trimmed.trim_start_matches('`').trim();
            if !lang.is_empty() { println!("{DIM}{FG_GRAY}  ▸ {lang}{RESET}"); }
            i += 1;
            let mut block: Vec<&str> = Vec::new();
            while i < lines.len() && !lines[i].trim().starts_with("```") {
                block.push(lines[i]); i += 1;
            }
            render_code_block(&block);
            i += 1; println!(); continue;
        }

        // HR
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            println!("{FG_GRAY}{DIM}{}{RESET}", hr('─', width));
            println!(); i += 1; continue;
        }

        // H1
        if let Some(t) = trimmed.strip_prefix("# ") {
            println!();
            let banner = format!("  {}  ", render_inline(t));
            println!("{BG_DARK}{FG_CYAN}{BOLD}{}{RESET}", center_str(&banner, width));
            println!("{FG_CYAN}{}{RESET}", hr('═', width));
            println!(); i += 1; continue;
        }

        // H2
        if let Some(t) = trimmed.strip_prefix("## ") {
            println!();
            println!("{FG_MAGENTA}{BOLD}  {}{RESET}", render_inline(t));
            println!("{FG_MAGENTA}  {}{RESET}", hr('─', width.saturating_sub(2)));
            println!(); i += 1; continue;
        }

        // H3
        if let Some(t) = trimmed.strip_prefix("### ") {
            println!("{FG_YELLOW}{BOLD}  ◆ {}{RESET}", render_inline(t));
            println!(); i += 1; continue;
        }

        // H4
        if let Some(t) = trimmed.strip_prefix("#### ") {
            println!("{FG_BLUE}{BOLD}    ▸ {}{RESET}", render_inline(t));
            i += 1; continue;
        }

        // Blockquote
        if let Some(t) = trimmed.strip_prefix("> ") {
            println!("{FG_BLUE}  ┃ {ITALIC}{}{RESET}", render_inline(t));
            i += 1; continue;
        }

        // Table
        if trimmed.starts_with('|') {
            let mut table_rows: Vec<Vec<String>> = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('|') {
                let cells: Vec<String> = lines[i].trim()
                    .trim_start_matches('|').trim_end_matches('|')
                    .split('|').map(|c| c.trim().to_string()).collect();
                table_rows.push(cells);
                i += 1;
            }
            println!();
            render_table(&table_rows, width);
            println!();
            continue;
        }

        // Bullet list
        if let Some(t) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            println!("  {FG_CYAN}•{RESET} {}", render_inline(t));
            i += 1; continue;
        }

        // Numbered list
        if trimmed.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) && trimmed.contains(". ") {
            let mut parts = trimmed.splitn(2, ". ");
            let num  = parts.next().unwrap_or("");
            let text = parts.next().unwrap_or(trimmed);
            println!("  {FG_YELLOW}{num}.{RESET} {}", render_inline(text));
            i += 1; continue;
        }

        // Empty
        if trimmed.is_empty() { println!(); i += 1; continue; }

        // Paragraph with word-wrap
        let wrap_w = width.saturating_sub(4);
        let mut cur_line = String::new();
        let mut cur_len  = 0usize;
        print!("  ");
        for word in trimmed.split_whitespace() {
            let wl = word.chars().count();
            if cur_len + wl + 1 > wrap_w && !cur_line.is_empty() {
                println!("{}", render_inline(&cur_line));
                print!("  ");
                cur_line = word.to_string();
                cur_len  = wl;
            } else {
                if !cur_line.is_empty() { cur_line.push(' '); cur_len += 1; }
                cur_line.push_str(word);
                cur_len += wl;
            }
        }
        if !cur_line.is_empty() { println!("{}", render_inline(&cur_line)); }
        i += 1;
    }
}

fn find_reports(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else { return vec![]; };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    files.sort();
    files
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let reports_dir = PathBuf::from("reports");

    let file_path: PathBuf = if args.len() > 1 {
        let arg = &args[1];
        let p = PathBuf::from(arg);
        if p.exists() { p } else {
            find_reports(&reports_dir).into_iter()
                .find(|f| f.to_string_lossy().contains(arg.as_str()))
                .unwrap_or_else(|| { eprintln!("No report matching '{arg}'"); std::process::exit(1); })
        }
    } else {
        let mut r = find_reports(&reports_dir);
        if r.is_empty() { eprintln!("No reports found. Run: cargo run --bin travel -- <from> <to> <days> <budget>"); std::process::exit(1); }
        r.pop().unwrap()
    };

    let content = fs::read_to_string(&file_path)
        .unwrap_or_else(|e| { eprintln!("Cannot read {}: {e}", file_path.display()); std::process::exit(1); });

    let width = term_width();
    let fname = file_path.file_name().unwrap_or_default().to_string_lossy();

    println!();
    println!("{FG_GRAY}  {}{RESET}", hr('═', width - 2));
    println!("{}", center_str(&format!("{FG_CYAN}{BOLD}  🌏 OpenWorld Report Viewer  {RESET}"), width));
    println!("{FG_GRAY}  {DIM}{fname}{RESET}");
    println!("{FG_GRAY}  {}{RESET}", hr('─', width - 2));
    println!();

    let all = find_reports(&reports_dir);
    if all.len() > 1 {
        println!("  {FG_GRAY}Reports:{RESET}");
        for r in &all {
            let n = r.file_name().unwrap_or_default().to_string_lossy();
            if r == &file_path { println!("  {FG_GREEN}▶ {n}{RESET}"); }
            else               { println!("  {FG_GRAY}  {n}{RESET}"); }
        }
        println!("  {FG_GRAY}{}{RESET}", hr('─', width - 2));
        println!("  {DIM}Tip: cargo run --bin report -- <session_id>{RESET}");
        println!();
    }

    render_markdown(&content);

    println!();
    println!("{FG_GRAY}  {}{RESET}", hr('═', width - 2));
    println!();
}
