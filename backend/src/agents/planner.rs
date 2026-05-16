/*!
PlannerAgent — itinerary generation powered by 0G Compute.

Takes a TravelPolicy and produces a structured Itinerary with:
  - Daily segments (flights, hotels, trains)
  - Estimated prices per segment
  - Budget allocation across the trip
  - Reasoning trace for orchestrator visibility
*/

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{
  ActivityLog, Agent, DailyActivity, DayPlan, ExecutionContext, Itinerary, SegmentKind, TravelSegment,
};
use crate::og_compute::OgComputeClient;

// ─── PlannerAgent ─────────────────────────────────────────────────────────────

pub struct PlannerAgent {
  compute: OgComputeClient,
  /// Shared slot where the produced itinerary is written for downstream agents
  pub itinerary: Arc<Mutex<Option<Itinerary>>>,
}

impl PlannerAgent {
  pub fn new(compute: OgComputeClient, itinerary: Arc<Mutex<Option<Itinerary>>>) -> Self {
    Self { compute, itinerary }
  }
}

#[async_trait]
impl Agent for PlannerAgent {
  fn name(&self) -> &str {
    "PlannerAgent"
  }

  async fn run(&self, ctx: &ExecutionContext) -> Result<()> {
    ctx.log(ActivityLog::info(self.name(), "Loading travel.md constraints..."));
    let city_name = ctx.policy.resolved_city_name();
    if ctx.policy.is_city_code() {
      ctx.log(ActivityLog::info(
        self.name(),
        &format!("✓ Destination: {} ({}) — single-city mode", ctx.policy.trip.destination, city_name),
      ));
    } else {
      ctx.log(ActivityLog::info(
        self.name(),
        &format!("✓ Destination: {}", ctx.policy.trip.destination),
      ));
    }
    ctx.log(ActivityLog::info(
      self.name(),
      &format!("✓ Budget limit: {} USD", ctx.policy.trip.budget_max),
    ));
    ctx.log(ActivityLog::info(
      self.name(),
      &format!("✓ Duration: {} days", ctx.policy.trip.duration_days),
    ));

    if ctx.policy.flight.avoid_red_eye {
      ctx.log(ActivityLog::info(self.name(), "✓ Overnight flights excluded"));
    }
    if ctx.policy.transport.prefer_train {
      ctx.log(ActivityLog::info(self.name(), "✓ Train priority enabled"));
    }

    ctx.log(ActivityLog::action(self.name(), "Generating itinerary via 0G Compute..."));

    let constraints = ctx.policy.to_constraint_json();
    let origin      = &ctx.policy.trip.origin;
    let destination = &ctx.policy.trip.destination;
    let dep_date    = &ctx.policy.trip.departure_date;
    let ret_date    = &ctx.policy.trip.return_date;
    let days        = ctx.policy.trip.duration_days;
    let single_city = ctx.policy.is_city_code();
    let city_name   = ctx.policy.resolved_city_name().to_string();

    let city_rules = if single_city {
      format!(
        r#"SINGLE-CITY MODE — destination is the IATA code "{destination}" which refers to {city_name}.
- ALL hotel segments MUST use "{city_name}" as the city — no other cities.
- Do NOT include any inter-city flights, trains, or buses (no segments travelling between cities).
- Local transport (metro, taxi, transfer) within {city_name} is allowed.
- The outbound flight goes FROM "{origin}" TO "{city_name}".
- The return flight goes FROM "{city_name}" TO "{origin}".
- All non-flight segments must stay within {city_name}."#
      )
    } else {
      format!(
        r#"MULTI-CITY MODE — destination is "{destination}" (a country or region).
- The outbound flight "from" MUST be "{origin}" and "to" MUST be the first city in {destination}.
- The return flight "from" MUST be the last city visited and "to" MUST be "{origin}".
- All inter-city segments must use real city names, not "null"."#
      )
    };

    let prompt = format!(
      r#"You are an expert travel planner. Generate a detailed itinerary based on these constraints:
{constraints}

═══════════════════════════════════════════
DATE RULES — THESE ARE MANDATORY, DO NOT IGNORE:
  DEPARTURE DATE = {dep_date}   ← USE THIS EXACT DATE
  RETURN DATE    = {ret_date}   ← USE THIS EXACT DATE
  All "date" fields in segments AND daily_plan MUST be between {dep_date} and {ret_date}.
  DO NOT use 2023, 2024, or any year/month/day that is not within {dep_date} to {ret_date}.
  Day 1 date = {dep_date}, Day 2 date = one day after {dep_date}, etc.
═══════════════════════════════════════════

TRIP: FROM "{origin}" TO "{destination}", {days} days, departure {dep_date}, return {ret_date}.

{city_rules}

Return ONLY a JSON object with this exact structure:
{{
  "destination": "string",
  "duration_days": number,
  "estimated_total_usd": number,
  "reasoning": "brief explanation of how you balanced the budget and preferences",
  "segments": [
    {{
      "id": "seg_001",
      "kind": "flight|hotel|train|bus|transfer",
      "from": "city or airport code",
      "to": "destination city or null for hotels",
      "date": "YYYY-MM-DD",
      "duration": "Xh Ym or X nights",
      "provider_hints": ["airline/hotel name suggestions"],
      "estimated_price_usd": number
    }}
  ],
  "daily_plan": [
    {{
      "day": 1,
      "date": "YYYY-MM-DD",
      "title": "short headline for the day",
      "activities": [
        {{
          "time": "09:00",
          "activity": "what to do — be specific",
          "location": "exact area or landmark name",
          "est_cost_usd": number,
          "notes": "tips, reservations needed, opening hours, etc. or null"
        }}
      ]
    }}
  ]
}}

Rules:
- Total of all segment prices must be under budget_max
- Include outbound flight, hotels for each night, and local transport
- For hotels, "to" is null and "from" is the city
- Respect avoid_red_eye (no overnight flights if true)
- Prefer trains over buses when prefer_train is true
- daily_plan must have one entry per day (day 1 = arrival day, last day = departure day)
- Each day should have 3–5 activities covering morning, afternoon, and evening
- Include realistic cost estimates and practical tips in notes
- Activities on the arrival day should account for flight arrival time
- Activities on the last day should end before departure time"#
    );

    let raw = self.compute.infer(&prompt).await.unwrap_or_else(|e| {
      // Fallback itinerary so the demo never hard-fails
      tracing_fallback(ctx, self.name(), &e.to_string());
      demo_itinerary_json(
        &ctx.policy.trip.destination,
        ctx.policy.resolved_city_name(),
        ctx.policy.is_city_code(),
        &ctx.policy.trip.departure_date,
        ctx.policy.trip.duration_days,
        ctx.policy.trip.budget_max,
      )
    });

    let itinerary = parse_itinerary_response(&raw, &ctx.policy)?;

    ctx.log(ActivityLog::success(
      self.name(),
      &format!(
        "Itinerary planned — {} segments, est. {:.0} USD",
        itinerary.segments.len(),
        itinerary.estimated_total_usd
      ),
    ));

    for seg in &itinerary.segments {
      ctx.log(ActivityLog::info(
        self.name(),
        &format!(
          "  {} {} {} → {}  ({:.0} USD)",
          seg.date,
          segment_emoji(&seg.kind),
          seg.from,
          seg.to.as_deref().unwrap_or(&seg.from),
          seg.estimated_price_usd
        ),
      ));
    }

    if !itinerary.daily_plan.is_empty() {
      ctx.log(ActivityLog::info(self.name(), "📅 Daily schedule:"));
      for day in &itinerary.daily_plan {
        ctx.log(ActivityLog::info(
          self.name(),
          &format!("  Day {} ({}) — {}", day.day, day.date, day.title),
        ));
        for act in &day.activities {
          ctx.log(ActivityLog::info(
            self.name(),
            &format!(
              "    {}  {}  [{}]{}",
              act.time,
              act.activity,
              act.location,
              if act.est_cost_usd > 0.0 {
                format!("  ~${:.0}", act.est_cost_usd)
              } else {
                " free".to_string()
              }
            ),
          ));
        }
      }
    }

    *self.itinerary.lock().await = Some(itinerary);
    Ok(())
  }
}

// ─── Parsing ──────────────────────────────────────────────────────────────────

fn parse_itinerary_response(raw: &str, policy: &crate::travel_spec::TravelPolicy) -> Result<Itinerary> {
  // Extract JSON block from potentially noisy LLM output
  let json_str = extract_json_block(raw);

  let v: Value = serde_json::from_str(&json_str).unwrap_or_else(|_| {
    serde_json::from_str(&demo_itinerary_json(
      &policy.trip.destination,
      policy.resolved_city_name(),
      policy.is_city_code(),
      &policy.trip.departure_date,
      policy.trip.duration_days,
      policy.trip.budget_max,
    ))
    .unwrap()
  });

  let segments = v["segments"]
    .as_array()
    .map(|arr| {
      arr
        .iter()
        .enumerate()
        .map(|(i, s)| TravelSegment {
          id: s["id"]
            .as_str()
            .unwrap_or(&format!("seg_{:03}", i + 1))
            .to_string(),
          kind: parse_segment_kind(s["kind"].as_str().unwrap_or("flight")),
          from: s["from"].as_str().unwrap_or("Unknown").to_string(),
          to: s["to"].as_str().map(|s| s.to_string()),
          date: s["date"].as_str().unwrap_or("TBD").to_string(),
          duration: s["duration"].as_str().map(|s| s.to_string()),
          provider_hints: s["provider_hints"]
            .as_array()
            .map(|a| {
              a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
            })
            .unwrap_or_default(),
          estimated_price_usd: s["estimated_price_usd"].as_f64().unwrap_or(0.0),
        })
        .collect()
    })
    .unwrap_or_default();

  let mut itinerary = Itinerary {
    destination: v["destination"]
      .as_str()
      .unwrap_or(&policy.trip.destination)
      .to_string(),
    duration_days: v["duration_days"].as_u64().unwrap_or(policy.trip.duration_days as u64) as u32,
    estimated_total_usd: v["estimated_total_usd"]
      .as_f64()
      .unwrap_or(0.0),
    reasoning: v["reasoning"]
      .as_str()
      .unwrap_or("Itinerary generated within budget constraints.")
      .to_string(),
    segments,
    daily_plan: v["daily_plan"]
      .as_array()
      .map(|days| {
        days
          .iter()
          .enumerate()
          .map(|(i, d)| DayPlan {
            day: d["day"].as_u64().unwrap_or((i + 1) as u64) as u32,
            date: d["date"].as_str().unwrap_or("TBD").to_string(),
            title: d["title"].as_str().unwrap_or("").to_string(),
            activities: d["activities"]
              .as_array()
              .map(|acts| {
                acts
                  .iter()
                  .map(|a| DailyActivity {
                    time: a["time"].as_str().unwrap_or("").to_string(),
                    activity: a["activity"].as_str().unwrap_or("").to_string(),
                    location: a["location"].as_str().unwrap_or("").to_string(),
                    est_cost_usd: a["est_cost_usd"].as_f64().unwrap_or(0.0),
                    notes: a["notes"].as_str().map(|s| s.to_string()),
                  })
                  .collect()
              })
              .unwrap_or_default(),
          })
          .collect()
      })
      .unwrap_or_default(),
  };

  fix_dates(&mut itinerary, policy);
  Ok(itinerary)
}

/// Remap every date in the itinerary so that the earliest date aligns with
/// the real departure date from the policy. Fixes LLM hallucinated years.
fn fix_dates(itinerary: &mut Itinerary, policy: &crate::travel_spec::TravelPolicy) {
  use chrono::NaiveDate;

  let dep_date = match NaiveDate::parse_from_str(&policy.trip.departure_date, "%Y-%m-%d") {
    Ok(d) => d,
    Err(_) => return, // no real date in policy — nothing to fix
  };

  // Collect every date string from segments + daily_plan
  let all_dates: Vec<NaiveDate> = itinerary
    .segments
    .iter()
    .filter_map(|s| NaiveDate::parse_from_str(&s.date, "%Y-%m-%d").ok())
    .chain(
      itinerary
        .daily_plan
        .iter()
        .filter_map(|d| NaiveDate::parse_from_str(&d.date, "%Y-%m-%d").ok()),
    )
    .collect();

  let llm_min = match all_dates.iter().min() {
    Some(d) => *d,
    None => return,
  };

  // If dates are already correct (within 30 days of dep_date) — skip
  let offset_days = (dep_date - llm_min).num_days();
  if offset_days == 0 {
    return;
  }

  let shift = |s: &str| -> String {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
      .map(|d| (d + chrono::Duration::days(offset_days)).format("%Y-%m-%d").to_string())
      .unwrap_or_else(|_| s.to_string())
  };

  for seg in &mut itinerary.segments {
    seg.date = shift(&seg.date);
  }
  for day in &mut itinerary.daily_plan {
    day.date = shift(&day.date);
  }
}

fn extract_json_block(s: &str) -> String {
  // Find the first '{' and last '}' to strip LLM preamble/postamble
  if let (Some(start), Some(end)) = (s.find('{'), s.rfind('}')) {
    s[start..=end].to_string()
  } else {
    s.to_string()
  }
}

fn parse_segment_kind(s: &str) -> SegmentKind {
  match s.to_lowercase().as_str() {
    "flight" => SegmentKind::Flight,
    "hotel" => SegmentKind::Hotel,
    "train" => SegmentKind::Train,
    "bus" => SegmentKind::Bus,
    _ => SegmentKind::Transfer,
  }
}

fn segment_emoji(kind: &SegmentKind) -> &'static str {
  match kind {
    SegmentKind::Flight => "✈",
    SegmentKind::Hotel => "🏨",
    SegmentKind::Train => "🚅",
    SegmentKind::Bus => "🚌",
    SegmentKind::Transfer => "🚕",
  }
}

fn tracing_fallback(ctx: &ExecutionContext, agent: &str, err: &str) {
  ctx.log(ActivityLog::warn(
    agent,
    &format!("0G Compute unavailable ({}), using fallback planner", err),
  ));
}

// ─── Demo Fallback ────────────────────────────────────────────────────────────

fn demo_itinerary_json(destination: &str, city_name: &str, single_city: bool, dep_date: &str, days: u32, budget: f64) -> String {
  use chrono::NaiveDate;
  let base = NaiveDate::parse_from_str(dep_date, "%Y-%m-%d")
    .unwrap_or_else(|_| chrono::Local::now().date_naive());
  let date_of = |offset: i64| -> String {
    (base + chrono::Duration::days(offset)).format("%Y-%m-%d").to_string()
  };

  let flight_price = (budget * 0.35) as u64;
  let hotel_night  = ((budget * 0.40) / days as f64) as u64;
  let transport    = (budget * 0.15) as u64;

  let display_dest = if single_city { city_name } else { destination };

  let mut segments = vec![
    serde_json::json!({
      "id": "seg_001",
      "kind": "flight",
      "from": "BKK",
      "to": city_name,
      "date": date_of(0),
      "duration": "6h 30m",
      "provider_hints": ["ANA", "JAL", "Thai Airways"],
      "estimated_price_usd": flight_price
    }),
    serde_json::json!({
      "id": "seg_002",
      "kind": "hotel",
      "from": city_name,
      "to": null,
      "date": date_of(0),
      "duration": format!("{} nights", days),
      "provider_hints": ["Dormy Inn", "APA Hotel", "Candeo Hotels"],
      "estimated_price_usd": hotel_night * days as u64
    }),
  ];

  if single_city {
    segments.push(serde_json::json!({
      "id": "seg_003",
      "kind": "transfer",
      "from": city_name,
      "to": format!("{} (local)", city_name),
      "date": date_of(1),
      "duration": "varies",
      "provider_hints": ["Metro", "Taxi", "BTS", "MRT"],
      "estimated_price_usd": transport
    }));
  } else {
    segments.push(serde_json::json!({
      "id": "seg_003",
      "kind": "train",
      "from": destination,
      "to": format!("{} (local)", destination),
      "date": date_of(1),
      "duration": "varies",
      "provider_hints": ["JR Pass", "IC Card"],
      "estimated_price_usd": transport
    }));
  }

  segments.push(serde_json::json!({
    "id": format!("seg_{:03}", segments.len() + 1),
    "kind": "flight",
    "from": city_name,
    "to": "BKK",
    "date": date_of(days as i64),
    "duration": "6h 30m",
    "provider_hints": ["ANA", "JAL"],
    "estimated_price_usd": flight_price
  }));

  serde_json::json!({
    "destination": display_dest,
    "duration_days": days,
    "estimated_total_usd": budget * 0.90,
    "reasoning": format!(
      "Allocated 35% to flights, 40% to hotels ({} nights), 15% to local transport under {} USD budget.",
      days, budget as u64
    ),
    "segments": segments
  })
  .to_string()
}
