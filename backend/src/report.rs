/*!
Travel report generator — produces a structured Markdown document from a completed session.

The report captures:
  - Trip overview and budget summary
  - Cost breakdown by category with visual bars
  - Daily spending summary
  - AI-generated itinerary (from PlannerAgent)
  - Daily activity schedule with per-day cost totals
  - Confirmed flight / hotel / transport bookings
  - AI-generated destination guide (currency, transport, weather, etiquette, emergency)
  - Applied travel policy constraints
  - Execution proof (hashes, 0G Storage root, on-chain TX)
*/

use chrono::Local;

use crate::agents::{BookingResult, BookingStatus, Itinerary, SegmentKind};
use crate::travel_spec::TravelPolicy;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Generate a Markdown travel report from session data.
///
/// Returns the raw Markdown string; the caller is responsible for writing it to disk.
pub fn generate_travel_report(
  policy: &TravelPolicy,
  itinerary: &Option<Itinerary>,
  bookings: &[BookingResult],
  artifact_id: &str,
  session_id: &str,
  execution_logs_hash: &str,
  storage_root_hash: Option<&str>,
  report_root_hash: Option<&str>,
  on_chain_tx: Option<&str>,
  travel_tips: Option<&str>,
) -> String {
  let mut md = String::with_capacity(8192);

  let dest = &policy.trip.destination;
  let city_name = policy.resolved_city_name();
  let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
  let short_session = short(session_id, 8);
  let short_artifact = short(artifact_id, 8);

  let confirmed: Vec<&BookingResult> =
    bookings.iter().filter(|b| b.status == BookingStatus::Confirmed).collect();
  let failed: Vec<&BookingResult> =
    bookings.iter().filter(|b| b.status == BookingStatus::Failed).collect();

  let total_spent: f64 = confirmed.iter().map(|b| b.price_usd).sum();
  let budget = policy.trip.budget_max;
  let savings = budget - total_spent;
  let pct_used = if budget > 0.0 { (total_spent / budget * 100.0).round() as u32 } else { 0 };
  let status_badge = if failed.is_empty() { "✅ Complete" } else { "⚠️ Partial" };

  // Category totals
  let flight_total: f64 = confirmed
    .iter()
    .filter(|b| b.booking_type.to_lowercase().contains("flight"))
    .map(|b| b.price_usd)
    .sum();
  let hotel_total: f64 = confirmed
    .iter()
    .filter(|b| b.booking_type.to_lowercase().contains("hotel"))
    .map(|b| b.price_usd)
    .sum();
  let transport_total: f64 = confirmed
    .iter()
    .filter(|b| {
      let t = b.booking_type.to_lowercase();
      t.contains("train") || t.contains("bus") || t.contains("transfer")
    })
    .map(|b| b.price_usd)
    .sum();

  // ── Header ──────────────────────────────────────────────────────────────────
  md.push_str("# 🌏 OpenWorld Travel Report\n\n");
  md.push_str("> Autonomous travel planning and reservation — powered by 0G Compute & 0G Storage\n\n");

  md.push_str("| | |\n|---|---|\n");
  push_row(&mut md, "**Origin**", &policy.trip.origin);
  push_row(&mut md, "**Destination**", &format!("{} ({})", dest, city_name));
  push_row(&mut md, "**Dates**", &format!("{} → {}", policy.trip.departure_date, policy.trip.return_date));
  push_row(&mut md, "**Status**", status_badge);
  push_row(&mut md, "**Generated**", &now);
  push_row(&mut md, "**Session**", &format!("`{}`", short_session));
  push_row(&mut md, "**Artifact**", &format!("`{}`", short_artifact));
  md.push('\n');

  md.push_str("---\n\n");

  // ── Trip Overview ───────────────────────────────────────────────────────────
  md.push_str("## 📋 Trip Overview\n\n");
  md.push_str("| Field | Value |\n|---|---|\n");
  push_row(&mut md, "Origin", &policy.trip.origin);
  push_row(&mut md, "Destination", &format!("{} ({})", dest, city_name));
  push_row(&mut md, "Departure", &policy.trip.departure_date);
  push_row(&mut md, "Return", &policy.trip.return_date);
  push_row(&mut md, "Duration", &format!("{} days", policy.trip.duration_days));
  push_row(&mut md, "Budget", &format!("${:.2} USD", budget));
  push_row(&mut md, "**Total Spent**", &format!("**${:.2} USD**", total_spent));
  push_row(&mut md, "Remaining", &format!("${:.2} USD  *({}% used)*", savings, pct_used));
  push_row(&mut md, "Confirmed Bookings", &confirmed.len().to_string());
  if !failed.is_empty() {
    push_row(&mut md, "⚠️ Failed Bookings", &failed.len().to_string());
  }
  md.push('\n');

  // Budget bar
  md.push_str("```\n");
  md.push_str(&format!(
    "Budget : {:>10}  |{}|\n",
    format!("${:.0}", budget),
    "█".repeat(20)
  ));
  let filled = (pct_used as usize * 20 / 100).min(20);
  md.push_str(&format!(
    "Spent  : {:>10}  |{}{}|  {}%\n",
    format!("${:.0}", total_spent),
    "█".repeat(filled),
    "░".repeat(20 - filled),
    pct_used
  ));
  md.push_str("```\n\n");

  md.push_str("---\n\n");

  // ── Cost Breakdown by Category ──────────────────────────────────────────────
  md.push_str("## 💰 Cost Breakdown\n\n");
  md.push_str("| Category | Amount | % of Budget |\n|----------|--------|-------------|\n");

  let categories: &[(&str, &str, f64)] = &[
    ("✈️", "Flights",   flight_total),
    ("🏨", "Hotels",    hotel_total),
    ("🚄", "Transport", transport_total),
  ];
  for (icon, label, amt) in categories {
    let amt_display = amt.max(0.0); // guard against -0.0
    if amt_display == 0.0 { continue; } // skip empty categories
    let pct = if budget > 0.0 { amt_display / budget * 100.0 } else { 0.0 };
    md.push_str(&format!("| {} {} | ${:.2} | {:.1}% |\n", icon, label, amt_display, pct));
  }
  let remaining_pct = if budget > 0.0 { savings / budget * 100.0 } else { 0.0 };
  md.push_str(&format!("| 💵 Remaining | ${:.2} | {:.1}% |\n", savings, remaining_pct));
  md.push_str(&format!("| **Total** | **${:.2}** | **{:.0}%** |\n\n", total_spent, pct_used));

  // Category bars
  md.push_str("```\n");
  for (icon, label, amt) in categories {
    let amt_display = amt.max(0.0);
    if amt_display == 0.0 { continue; }
    let pct = if budget > 0.0 { (amt_display / budget * 100.0).round() as usize } else { 0 };
    let bars = (pct * 20 / 100).min(20);
    md.push_str(&format!(
      "{} {:<10}: {:>8}  |{}{}|  {:.1}%\n",
      icon, label, format!("${:.0}", amt_display),
      "█".repeat(bars), "░".repeat(20 - bars), pct as f64
    ));
  }
  md.push_str("```\n\n");

  md.push_str("---\n\n");

  // ── Daily Spending Summary ──────────────────────────────────────────────────
  if let Some(itin) = itinerary {
    if !itin.daily_plan.is_empty() {
      md.push_str("## 📊 Daily Spending Summary\n\n");
      md.push_str("| Day | Date | Theme | Activities Est. | Booked Segments | Day Total |\n");
      md.push_str("|-----|------|-------|-----------------|-----------------|-----------|\n");

      for day in &itin.daily_plan {
        let activity_cost: f64 = day.activities.iter().map(|a| a.est_cost_usd).sum();
        // booked segments whose date matches this day — exclude flights (shown in separate section)
        let booked_day: f64 = itin
          .segments
          .iter()
          .filter(|s| s.date == day.date && !matches!(s.kind, SegmentKind::Flight))
          .map(|s| s.estimated_price_usd)
          .sum();
        let day_total = activity_cost + booked_day;
        md.push_str(&format!(
          "| {} | {} | {} | ~${:.0} | ${:.0} | **~${:.0}** |\n",
          day.day, day.date, day.title, activity_cost, booked_day, day_total
        ));
      }
      md.push('\n');
      md.push_str("---\n\n");
    }
  }

  // ── AI-Generated Itinerary ──────────────────────────────────────────────────
  md.push_str("## 🤖 AI-Generated Itinerary\n\n");
  md.push_str("*Planned by **PlannerAgent** via 0G Compute*\n\n");

  if let Some(itin) = itinerary {
    if !itin.reasoning.is_empty() {
      md.push_str(&format!("> {}\n\n", itin.reasoning.replace('\n', "\n> ")));
    }

    md.push_str("### Planned Segments\n\n");
    md.push_str("| # | Type | Route | Date | Duration | Est. Cost |\n");
    md.push_str("|---|------|-------|------|----------|-----------|\n");

    for (i, seg) in itin.segments.iter().enumerate() {
      let icon = kind_icon(&seg.kind);
      let route = match &seg.to {
        Some(to) if to != &seg.from => format!("{} → {}", seg.from, to),
        _ => seg.from.clone(),
      };
      let duration = seg.duration.as_deref().unwrap_or("—");
      md.push_str(&format!(
        "| {} | {} | {} | {} | {} | ${:.2} |\n",
        i + 1, icon, route, seg.date, duration, seg.estimated_price_usd,
      ));
    }

    md.push_str(&format!(
      "\n| | |\n|---|---|\n| **Planner Estimate** | USD {:.2} |\n| **Confirmed Actual** | USD {:.2} |\n\n",
      itin.estimated_total_usd, total_spent
    ));
    if (itin.estimated_total_usd - total_spent).abs() > 1.0 {
      let diff = itin.estimated_total_usd - total_spent;
      if diff > 0.0 {
        md.push_str(&format!("> 💡 Saved USD {:.0} vs initial plan through optimised pricing.\n\n", diff));
      } else {
        md.push_str(&format!("> ⚠️ Actual cost USD {:.0} over initial plan estimate.\n\n", diff.abs()));
      }
    }

    // ── Daily Activity Schedule ────────────────────────────────────────────────
    if !itin.daily_plan.is_empty() {
      md.push_str("---\n\n");
      md.push_str("## 📅 Daily Activity Schedule\n\n");
      for day in &itin.daily_plan {
        let day_activity_total: f64 = day.activities.iter().map(|a| a.est_cost_usd).sum();
        md.push_str(&format!(
          "### Day {} — {} &nbsp; *{}*\n\n",
          day.day, day.date, day.title
        ));
        md.push_str("| Time | Activity | Location | Est. Cost | Notes |\n");
        md.push_str("|------|----------|----------|-----------|-------|\n");
        for act in &day.activities {
          let cost = if act.est_cost_usd > 0.0 {
            format!("~${:.0}", act.est_cost_usd)
          } else {
            "Free".to_string()
          };
          let notes = act.notes.as_deref().unwrap_or("—");
          md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            act.time, act.activity, act.location, cost, notes
          ));
        }
        if day_activity_total > 0.0 {
          md.push_str(&format!(
            "| | **Day total (activities)** | | **~${:.0}** | |\n",
            day_activity_total
          ));
        }
        md.push('\n');
      }
    }
  } else {
    md.push_str("*Itinerary data unavailable.*\n\n");
  }

  md.push_str("---\n\n");

  // ── Flight Bookings ─────────────────────────────────────────────────────────
  let flights: Vec<&&BookingResult> = confirmed
    .iter()
    .filter(|b| b.booking_type.to_lowercase().contains("flight"))
    .collect();

  md.push_str("## ✈️ Flight Bookings\n\n");
  if flights.is_empty() {
    md.push_str("*No flight bookings.*\n\n");
  } else {
    md.push_str("| Airline / Provider | Booking Ref | Price (USD) | Confirmation | Status |\n");
    md.push_str("|--------------------|-------------|-------------|--------------|--------|\n");
    for b in &flights {
      let conf = b
        .confirmation_url
        .as_deref()
        .filter(|u| !u.is_empty())
        .map(|u| format!("[View]({})", u))
        .unwrap_or_else(|| format!("`{}`", b.reference));
      md.push_str(&format!(
        "| {} | `{}` | ${:.2} | {} | ✅ Confirmed |\n",
        b.provider, b.reference, b.price_usd, conf
      ));
    }
    md.push('\n');
  }

  md.push_str("---\n\n");

  // ── Hotel Bookings ──────────────────────────────────────────────────────────
  let hotels: Vec<&&BookingResult> = confirmed
    .iter()
    .filter(|b| b.booking_type.to_lowercase().contains("hotel"))
    .collect();

  md.push_str("## 🏨 Hotel Bookings\n\n");
  if hotels.is_empty() {
    md.push_str("*No hotel bookings.*\n\n");
  } else {
    md.push_str("| Hotel / Provider | Booking Ref | Price (USD) | Confirmation | Status |\n");
    md.push_str("|------------------|-------------|-------------|--------------|--------|\n");
    for b in &hotels {
      let conf = b
        .confirmation_url
        .as_deref()
        .filter(|u| !u.is_empty())
        .map(|u| format!("[View]({})", u))
        .unwrap_or_else(|| format!("`{}`", b.reference));
      md.push_str(&format!(
        "| {} | `{}` | ${:.2} | {} | ✅ Confirmed |\n",
        b.provider, b.reference, b.price_usd, conf
      ));
    }
    md.push('\n');
  }

  md.push_str("---\n\n");

  // ── Transport Bookings ──────────────────────────────────────────────────────
  let transport: Vec<&&BookingResult> = confirmed
    .iter()
    .filter(|b| {
      let t = b.booking_type.to_lowercase();
      t.contains("train") || t.contains("bus") || t.contains("transfer") || t.contains("transport")
    })
    .collect();

  if !transport.is_empty() {
    md.push_str("## 🚄 Transport Bookings\n\n");
    md.push_str("| Provider | Type | Booking Ref | Price (USD) | Confirmation | Status |\n");
    md.push_str("|----------|------|-------------|-------------|--------------|--------|\n");
    for b in &transport {
      let conf = b
        .confirmation_url
        .as_deref()
        .map(|u| format!("[View]({})", u))
        .unwrap_or_else(|| "—".to_string());
      md.push_str(&format!(
        "| {} | {} | `{}` | ${:.2} | {} | ✅ Confirmed |\n",
        b.provider, b.booking_type, b.reference, b.price_usd, conf
      ));
    }
    md.push('\n');
    md.push_str("---\n\n");
  }

  // ── Failed Bookings (if any) ────────────────────────────────────────────────
  if !failed.is_empty() {
    md.push_str("## ⚠️ Failed Bookings\n\n");
    md.push_str("| Type | Provider | Ref | Price |\n");
    md.push_str("|------|----------|-----|-------|\n");
    for b in &failed {
      md.push_str(&format!(
        "| {} | {} | `{}` | ${:.2} |\n",
        b.booking_type, b.provider, b.reference, b.price_usd
      ));
    }
    md.push('\n');
    md.push_str("---\n\n");
  }

  // ── Travel Policy Applied ───────────────────────────────────────────────────
  md.push_str("## 🔧 Travel Policy Applied\n\n");
  md.push_str("| Setting | Value |\n|---------|-------|\n");

  // Flight
  push_row(&mut md, "✈️ Max Stops", &policy.flight.max_stops.to_string());
  push_row(&mut md, "✈️ Avoid Red-Eye", bool_str(policy.flight.avoid_red_eye));
  if !policy.flight.preferred_airlines.is_empty() {
    push_row(&mut md, "✈️ Preferred Airlines", &policy.flight.preferred_airlines.join(", "));
  }
  // Hotel
  push_row(&mut md, "🏨 Min Rating", &format!("⭐ {:.1}", policy.hotel.min_rating));
  push_row(
    &mut md,
    "🏨 Max per Night",
    &format!("${:.2}", policy.hotel.max_price_per_night),
  );
  push_row(&mut md, "🏨 Near Station", bool_str(policy.hotel.near_station));
  // Transport
  push_row(&mut md, "🚄 Prefer Train", bool_str(policy.transport.prefer_train));
  push_row(
    &mut md,
    "🚄 Avoid Overnight Bus",
    bool_str(policy.transport.avoid_overnight_bus),
  );
  // Automation
  push_row(&mut md, "🤖 Auto Reserve", bool_str(policy.automation.auto_reserve));
  push_row(&mut md, "🤖 Retry on Failure", bool_str(policy.automation.retry_on_failure));
  push_row(&mut md, "🤖 Allow Replanning", bool_str(policy.automation.allow_replanning));
  // Vault
  push_row(&mut md, "💰 Auto Payment", bool_str(policy.vault.auto_payment));
  push_row(
    &mut md,
    "💰 Max Single TX",
    &format!("${:.2}", policy.vault.max_single_transaction),
  );
  md.push('\n');

  md.push_str("---\n\n");

  // ── Destination Guide (AI-generated tips) ───────────────────────────────────
  md.push_str("## 🌍 Destination Guide\n\n");
  md.push_str(&format!("*AI-generated travel tips for {} — powered by 0G Compute*\n\n", city_name));
  if let Some(tips) = travel_tips {
    md.push_str(tips.trim());
    md.push_str("\n\n");
  } else {
    md.push_str("*Travel tips unavailable for this session.*\n\n");
  }

  md.push_str("---\n\n");

  // ── Pre-Trip Checklist ──────────────────────────────────────────────────────
  md.push_str("## ✅ Pre-Trip Checklist\n\n");
  md.push_str("**Documents**\n");
  md.push_str("- [ ] Passport valid for 6+ months beyond return date\n");
  md.push_str(&format!("- [ ] Visa checked for {} (Thai passport holders)\n", city_name));
  md.push_str("- [ ] Travel insurance purchased and documents saved\n");
  md.push_str("- [ ] Flight e-tickets printed / saved offline\n");
  md.push_str("- [ ] Hotel booking confirmations saved\n");
  md.push_str("\n**Money**\n");
  md.push_str("- [ ] Notify bank / credit card of travel dates\n");
  md.push_str("- [ ] Exchange or withdraw local currency before departure\n");
  md.push_str("- [ ] Load backup card (Wise / Revolut) for emergencies\n");
  md.push_str("\n**Tech & Connectivity**\n");
  md.push_str("- [ ] Rent pocket WiFi or buy local SIM at airport\n");
  md.push_str("- [ ] Download Google Maps offline for destination city\n");
  md.push_str("- [ ] Download Google Translate + offline language pack\n");
  md.push_str("- [ ] Install local transit app (e.g. Suica for Tokyo)\n");
  md.push_str("- [ ] Charge all devices and pack universal adapter\n");
  md.push_str("\n**Health & Safety**\n");
  md.push_str("- [ ] Pack any prescription medications (enough for trip + spare)\n");
  md.push_str("- [ ] Travel health kit (pain reliever, antidiarrheal, plasters)\n");
  md.push_str("- [ ] Save local emergency numbers (police, ambulance, embassy)\n");
  md.push_str("- [ ] Share itinerary with someone at home\n");
  md.push_str("\n---\n\n");

  // ── Execution Proof ─────────────────────────────────────────────────────────
  md.push_str("## 🔐 Execution Proof\n\n");
  md.push_str("| Field | Value |\n|-------|-------|\n");
  push_row(&mut md, "Artifact ID", &format!("`{}`", artifact_id));
  push_row(&mut md, "Session ID", &format!("`{}`", session_id));
  push_row(&mut md, "Execution Hash", &format!("`{}`", execution_logs_hash));
  push_row(
    &mut md,
    "0G Artifact Root",
    &storage_root_hash
      .map(|h| format!("`{}`", h))
      .unwrap_or_else(|| "*(not stored)*".to_string()),
  );
  push_row(
    &mut md,
    "0G Report Root",
    &report_root_hash
      .map(|h| format!("`{}`", h))
      .unwrap_or_else(|| "*(not stored)*".to_string()),
  );
  if let Some(tx) = on_chain_tx {
    let explorer_url = format!("https://chainscan-galileo.0g.ai/tx/{}", tx);
    push_row(&mut md, "On-Chain TX", &format!("`{}`", tx));
    push_row(&mut md, "Explorer", &format!("[View on 0G Testnet]({})", explorer_url));
  } else {
    push_row(&mut md, "On-Chain TX", "*(not minted)*");
  }
  md.push('\n');

  md.push_str("---\n\n");

  // ── Footer ──────────────────────────────────────────────────────────────────
  md.push_str(&format!(
    "*Generated by **OpenWorld** Agentic Travel System · {} · Powered by [0G Compute](https://0g.ai) · [0G Storage](https://0g.ai)*\n",
    now
  ));

  md
}

/// Build the canonical report filename for a session.
///
/// Format: `TRAVEL_{Destination}_{YYYYMMDD_HHMMSS}_{short_session}.md`
pub fn report_filename(destination: &str, session_id: &str) -> String {
  let dest_slug: String = destination
    .chars()
    .filter(|c| c.is_alphanumeric() || c.is_whitespace())
    .collect::<String>()
    .split_whitespace()
    .collect::<Vec<_>>()
    .join("_");

  let date_slug = Local::now().format("%Y%m%d_%H%M%S").to_string();
  format!("TRAVEL_{}_{}_{}.md", dest_slug, date_slug, short(session_id, 8))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn push_row(md: &mut String, key: &str, val: &str) {
  md.push_str(&format!("| {} | {} |\n", key, val));
}

fn short(s: &str, n: usize) -> &str {
  &s[..s.len().min(n)]
}

fn bool_str(v: bool) -> &'static str {
  if v { "Yes" } else { "No" }
}

fn kind_icon(kind: &SegmentKind) -> &'static str {
  match kind {
    SegmentKind::Flight => "✈️ Flight",
    SegmentKind::Hotel => "🏨 Hotel",
    SegmentKind::Train => "🚄 Train",
    SegmentKind::Bus => "🚌 Bus",
    SegmentKind::Transfer => "🚗 Transfer",
  }
}
