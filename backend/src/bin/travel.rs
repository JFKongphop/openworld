/*!
OpenWorld — Terminal CLI

Run the full agentic travel pipeline directly in your terminal.
No server needed — logs stream live with colour.

Usage:
  cargo run --bin travel -- <from> <dest> <days> <budget>
  cargo run --bin travel -- <from> <dest> <dep_date> <days> <budget>
  cargo run --bin travel -- <from> <dest> <dep_date> <ret_date> <budget>

Examples:
  cargo run --bin travel -- "Bangkok" "TYO" 5 1200             # departs today
  cargo run --bin travel -- "Bangkok" "TYO" "2026-06-01" 5 1200 # departs Jun 1
  cargo run --bin travel -- "Bangkok" "Japan" "2026-07-10" "2026-07-17" 2500

Args:
  <from>          Departure city   (default: Bangkok)
  <dest>          City code or country (e.g. TYO, BKK, Japan)
  <dep_date>      Optional departure date YYYY-MM-DD (default: today)
  <days>          Trip duration in days  (default: 5)
  <ret_date>      Optional return date   YYYY-MM-DD
  <budget>        Total budget in USD    (default: 1200)
*/

use openworld::{
  create_session, load_env, run_session, ActivityLog, LogType, SessionState,
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
  let origin      = args.get(1).map(String::as_str).unwrap_or("Bangkok");
  let destination = args.get(2).map(String::as_str).unwrap_or("Tokyo");

  // Three supported forms:
  //   A) <days> <budget>                     — departs today
  //   B) <dep_date YYYY-MM-DD> <days> <budget>  — departs on dep_date
  //   C) <dep_date YYYY-MM-DD> <ret_date YYYY-MM-DD> <budget>
  let today = today_iso();
  let a3 = args.get(3).map(String::as_str).unwrap_or("");
  let a4 = args.get(4).map(String::as_str).unwrap_or("");
  let a5 = args.get(5).map(String::as_str).unwrap_or("");

  let is_date = |s: &str| s.len() == 10 && s.chars().nth(4) == Some('-');

  let (from_date, to_date, days, budget) = if let Some(n) = a3.parse::<u32>().ok() {
    // Form A: <days> <budget>  — departs today
    let d = n.max(1);
    let budget: u32 = a4.parse().unwrap_or(1200);
    (today.clone(), add_days(&today, d as i64), d, budget)
  } else if is_date(a3) && !is_date(a4) {
    // Form B: <dep_date> <days> <budget>  — explicit departure, duration in days
    let dep = a3.to_string();
    let d: u32 = a4.parse().unwrap_or(5).max(1);
    let budget: u32 = a5.parse().unwrap_or(1200);
    (dep.clone(), add_days(&dep, d as i64), d, budget)
  } else {
    // Form C: <dep_date> <ret_date> <budget>  — fully explicit dates
    let dep = if is_date(a3) { a3.to_string() } else { today.clone() };
    let ret = if is_date(a4) { a4.to_string() } else { add_days(&dep, 5) };
    let budget: u32 = a5.parse().unwrap_or(1200);
    let d = days_between(&dep, &ret);
    (dep, ret, d, budget)
  };

  // ── Banner ─────────────────────────────────────────────────────────────────
  println!();
  println!("  {BOLD}╔══════════════════════════════════════════════════╗{RESET}");
  println!("  {BOLD}║       🌏  OpenWorld Agentic Travel CLI           ║{RESET}");
  println!("  {BOLD}╚══════════════════════════════════════════════════╝{RESET}");
  println!();
  println!("  {CYAN}From        :{RESET} {BOLD}{origin}{RESET}");
  println!("  {CYAN}Destination :{RESET} {BOLD}{destination}{RESET}");
  println!("  {CYAN}Dates       :{RESET} {from_date} → {to_date}  ({days} days)");
  println!("  {CYAN}Budget      :{RESET} ${budget} USD");
  println!();

  // ── Create session ─────────────────────────────────────────────────────────
  let travel_md = build_travel_md(origin, destination, &from_date, &to_date, days, budget);
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
      println!("  Contract   {DIM}0x770f6107934224882ce4919934eE5B2BfF7783aE{RESET}");
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
