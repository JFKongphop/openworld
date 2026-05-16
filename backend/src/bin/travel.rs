/*!
OpenWorld — Terminal CLI

Run the full agentic travel pipeline directly in your terminal.
No server needed — logs stream live with colour.

Usage (quick — positional args):
  cargo run --bin travel -- <from> <dest> <days> <budget>
  cargo run --bin travel -- <from> <dest> <dep_date> <days> <budget>
  cargo run --bin travel -- <from> <dest> <dep_date> <ret_date> <budget>

Usage (file — full control via YAML):
  cargo run --bin travel -- trip.md
  cargo run --bin travel -- examples/tokyo.md

File mode reads a travel.md YAML file directly, giving you full control over
hotel rating, preferred airlines, per-night limits, and all policy settings.
See examples/trip.md for the template.

Examples:
  cargo run --bin travel -- "Bangkok" "TYO" 5 1200
  cargo run --bin travel -- "Bangkok" "TYO" "2026-06-01" 5 1200
  cargo run --bin travel -- "Bangkok" "Japan" "2026-07-10" "2026-07-17" 2500
  cargo run --bin travel -- examples/trip.md
*/

use openworld::{
  create_session, load_env, parse_travel_md, run_session, ActivityLog, LogType, SessionState,
};
use tokio::sync::broadcast;

// ─── ANSI colours ─────────────────────────────────────────────────────────────

const RESET:   &str = "\x1b[0m";
const BOLD:    &str = "\x1b[1m";
const DIM:     &str = "\x1b[2m";
const RED:     &str = "\x1b[31m";
const GREEN:   &str = "\x1b[32m";
const YELLOW:  &str = "\x1b[33m";
const BLUE:    &str = "\x1b[34m";
const MAGENTA: &str = "\x1b[35m";
const CYAN:    &str = "\x1b[36m";

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn agent_colour(agent: &str) -> &'static str {
  match agent {
    "PlannerAgent"     => MAGENTA,
    "SearchAgent"      => CYAN,
    "ReservationAgent" => YELLOW,
    "RecoveryAgent"    => RED,
    "VaultAgent"       => GREEN,
    "ArtifactAgent"    => BLUE,
    _                  => DIM,
  }
}

fn log_icon(kind: &LogType) -> &'static str {
  match kind {
    LogType::Info    => "·",
    LogType::Success => "✓",
    LogType::Warning => "⚠",
    LogType::Error   => "✗",
    LogType::Action  => "▶",
  }
}

fn print_log(log: &ActivityLog) {
  let colour = agent_colour(&log.agent);
  let icon   = log_icon(&log.log_type);
  println!(
    "  {DIM}{ts}{RESET}  {icon}  {colour}{agent:<18}{RESET}  {msg}",
    DIM    = DIM,
    RESET  = RESET,
    ts     = log.timestamp,
    icon   = icon,
    colour = colour,
    agent  = log.agent,
    msg    = log.message,
  );
}

fn build_travel_md(origin: &str, destination: &str, from_date: &str, to_date: &str, days: u32, budget: u32) -> String {
  let per_night = (budget / days / 3).max(50);
  let max_tx    = (budget / 3).max(100);
  format!(
    "trip:\n\
     \x20 origin: {origin}\n\
     \x20 destination: {destination}\n\
     \x20 departure_date: \"{from_date}\"\n\
     \x20 return_date: \"{to_date}\"\n\
     \x20 duration_days: {days}\n\
     \x20 budget_max: \"{budget} USD\"\n\
     flight:\n\
     \x20 max_stops: 1\n\
     \x20 avoid_red_eye: true\n\
     \x20 preferred_airlines: [ANA, JAL, Emirates, Singapore Airlines]\n\
     hotel:\n\
     \x20 min_rating: 4.0\n\
     \x20 max_price_per_night: \"{per_night} USD\"\n\
     \x20 near_station: true\n\
     transport:\n\
     \x20 prefer_train: true\n\
     \x20 avoid_overnight_bus: true\n\
     automation:\n\
     \x20 auto_reserve: true\n\
     \x20 retry_on_failure: true\n\
     \x20 allow_replanning: true\n\
     vault:\n\
     \x20 auto_payment: true\n\
     \x20 max_single_transaction: \"{max_tx} USD\"\n",
  )
}

// ─── Date helpers ─────────────────────────────────────────────────────────────

/// Today as YYYY-MM-DD using chrono local time.
fn today_iso() -> String {
  chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Add `days` calendar days to a YYYY-MM-DD string.
fn add_days(date_iso: &str, days: i64) -> String {
  use chrono::NaiveDate;
  if let Ok(d) = NaiveDate::parse_from_str(date_iso, "%Y-%m-%d") {
    (d + chrono::Duration::days(days)).format("%Y-%m-%d").to_string()
  } else {
    date_iso.to_string()
  }
}

/// Count days between two YYYY-MM-DD strings.
fn days_between(from: &str, to: &str) -> u32 {
  use chrono::NaiveDate;
  if let (Ok(f), Ok(t)) = (
    NaiveDate::parse_from_str(from, "%Y-%m-%d"),
    NaiveDate::parse_from_str(to, "%Y-%m-%d"),
  ) {
    (t - f).num_days().max(1) as u32
  } else {
    5
  }
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
  load_env();

  // ── Parse CLI args ─────────────────────────────────────────────────────────
  let args: Vec<String> = std::env::args().collect();

  // ── Detect file mode: first arg is a .md / .yaml / .yml file ──────────────
  let first = args.get(1).map(String::as_str).unwrap_or("");
  let is_file_mode = (first.ends_with(".md") || first.ends_with(".yaml") || first.ends_with(".yml"))
    && std::path::Path::new(first).exists();

  let travel_md: String;
  let (origin, destination, from_date, to_date, days, budget);

  if is_file_mode {
    // ── File mode: read travel.md directly ──────────────────────────────────
    travel_md = match std::fs::read_to_string(first) {
      Ok(s) => s,
      Err(e) => {
        eprintln!("  {RED}✗ Cannot read file '{first}': {e}{RESET}");
        std::process::exit(1);
      }
    };
    // Parse to extract values for banner display
    let policy = match parse_travel_md(&travel_md) {
      Ok(p) => p,
      Err(e) => {
        eprintln!("  {RED}✗ Invalid travel.md: {e}{RESET}");
        std::process::exit(1);
      }
    };
    let today = today_iso();
    origin      = policy.trip.origin.clone();
    destination = policy.trip.destination.clone();
    from_date   = if policy.trip.departure_date.is_empty() { today.clone() } else { policy.trip.departure_date.clone() };
    to_date     = if policy.trip.return_date.is_empty() { add_days(&from_date, policy.trip.duration_days as i64) } else { policy.trip.return_date.clone() };
    days        = policy.trip.duration_days;
    budget      = policy.trip.budget_max as u32;
  } else {
    // ── Positional arg mode (existing behaviour) ───────────────────────────
    let o = args.get(1).map(String::as_str).unwrap_or("Bangkok").to_string();
    let d = args.get(2).map(String::as_str).unwrap_or("Tokyo").to_string();

    // Three supported forms:
    //   A) <days> <budget>                           — departs today
    //   B) <dep_date YYYY-MM-DD> <days> <budget>     — explicit departure
    //   C) <dep_date YYYY-MM-DD> <ret_date> <budget> — fully explicit dates
    let today = today_iso();
    let a3 = args.get(3).map(String::as_str).unwrap_or("");
    let a4 = args.get(4).map(String::as_str).unwrap_or("");
    let a5 = args.get(5).map(String::as_str).unwrap_or("");

    let is_date = |s: &str| s.len() == 10 && s.chars().nth(4) == Some('-');

    let (fd, td, dy, bg) = if let Ok(n) = a3.parse::<u32>() {
      let d = n.max(1);
      let b: u32 = a4.parse().unwrap_or(1200);
      (today.clone(), add_days(&today, d as i64), d, b)
    } else if is_date(a3) && !is_date(a4) {
      let dep = a3.to_string();
      let dy: u32 = a4.parse().unwrap_or(5).max(1);
      let b: u32 = a5.parse().unwrap_or(1200);
      (dep.clone(), add_days(&dep, dy as i64), dy, b)
    } else {
      let dep = if is_date(a3) { a3.to_string() } else { today.clone() };
      let ret = if is_date(a4) { a4.to_string() } else { add_days(&dep, 5) };
      let b: u32 = a5.parse().unwrap_or(1200);
      let dy = days_between(&dep, &ret);
      (dep, ret, dy, b)
    };

    origin      = o;
    destination = d;
    from_date   = fd;
    to_date     = td;
    days        = dy;
    budget      = bg;
    travel_md   = build_travel_md(&origin, &destination, &from_date, &to_date, days, budget);
  }

  // ── Banner ─────────────────────────────────────────────────────────────────
  println!();
  println!("  {BOLD}╔══════════════════════════════════════════════════╗{RESET}");
  println!("  {BOLD}║       🌏  OpenWorld Agentic Travel CLI           ║{RESET}");
  println!("  {BOLD}╚══════════════════════════════════════════════════╝{RESET}");
  println!();
  if is_file_mode {
    println!("  {CYAN}Mode        :{RESET} {DIM}file → {first}{RESET}");
  }
  println!("  {CYAN}From        :{RESET} {BOLD}{origin}{RESET}");
  println!("  {CYAN}Destination :{RESET} {BOLD}{destination}{RESET}");
  println!("  {CYAN}Dates       :{RESET} {from_date} → {to_date}  ({days} days)");
  println!("  {CYAN}Budget      :{RESET} ${budget} USD");
  println!();
  let session = match create_session(&travel_md) {
    Ok(s) => s,
    Err(e) => {
      eprintln!("  {RED}✗ Failed to create session: {e}{RESET}");
      std::process::exit(1);
    }
  };

  let short_id = &session.session_id.to_string()[..8];
  println!("  {DIM}Session : {short_id}{RESET}");
  println!();

  // ── Subscribe to live logs before starting ─────────────────────────────────
  let mut log_rx: broadcast::Receiver<ActivityLog> = session.subscribe();

  // ── Launch pipeline ────────────────────────────────────────────────────────
  println!("  {DIM}Launching agent pipeline...{RESET}");
  println!();
  run_session(session.clone());

  // ── Stream logs until complete ─────────────────────────────────────────────
  loop {
    match log_rx.recv().await {
      Ok(log) => {
        print_log(&log);
      }
      Err(broadcast::error::RecvError::Lagged(n)) => {
        println!("  {YELLOW}⚠  Missed {n} log entries (buffer overflow){RESET}");
      }
      Err(broadcast::error::RecvError::Closed) => {
        // Sender dropped — pipeline finished
        break;
      }
    }

    // Stop when session reaches a terminal state
    let state = session.current_state().await;
    if state == SessionState::Complete || state == SessionState::Failed {
      break;
    }
  }

  // Drain any final messages
  while let Ok(log) = log_rx.try_recv() {
    print_log(&log);
  }

  println!();

  // ── Final state ────────────────────────────────────────────────────────────
  let final_state = session.current_state().await;

  if final_state == SessionState::Failed {
    println!("  {RED}{BOLD}✗ Session failed{RESET}");
    std::process::exit(1);
  }

  // ── Summary ────────────────────────────────────────────────────────────────
  let artifact = session.artifact.lock().await.clone();
  if let Some(art) = &artifact {
    println!("  {GREEN}{BOLD}✓ Trip complete!{RESET}");
    println!();
    println!("  {BOLD}Summary{RESET}");
    println!("  ────────────────────────────────────");
    println!("  Destination   {BOLD}{}{RESET}", art.destination);
    println!("  Duration      {} days", art.duration_days);
    println!("  Total Spent   {GREEN}{BOLD}${:.2} USD{RESET}", art.total_spent_usd);
    println!("  Budget Left   ${:.2} USD",
      budget as f64 - art.total_spent_usd);
    println!("  Bookings      {} confirmed", art.bookings.len());
    println!("  Artifact ID   {DIM}{}{RESET}", art.artifact_id);
    if let Some(root) = &art.storage_root_hash {
      println!("  0G Artifact   {CYAN}{root}{RESET}");
    }
    if let Some(root) = &art.report_root_hash {
      println!("  0G Report     {CYAN}{root}{RESET}");
    }

    if let Some(tx) = &art.on_chain_tx {
      println!();
      println!("  {BOLD}On-Chain Proof (ERC-7857){RESET}");
      println!("  ────────────────────────────────────");
      println!("  Contract   {DIM}0xAF2699e9d306b57F5541aE3f04C43586589fD455{RESET}");
      println!("  Tx Hash    {GREEN}{tx}{RESET}");
      println!("  Explorer   {DIM}https://chainscan-galileo.0g.ai/tx/{tx}{RESET}");
    }

    if let Some(path) = &art.report_path {
      println!();
      println!("  {BOLD}Report saved:{RESET}");
      println!("  {DIM}{path}{RESET}");
      println!();


    }
  } else {
    println!("  {GREEN}{BOLD}✓ Session complete{RESET}  {DIM}(no artifact){RESET}");
  }

  println!();
}
