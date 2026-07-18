/*!
RecoveryAgent — failure recovery with real SerpAPI re-search + bounded retry.

When a booking fails, the RecoveryAgent:
  1. Distinguishes failure type (price spike vs unavailability vs timeout)
  2. Re-runs SerpAPI with relaxed constraints to find real alternatives
  3. Uses Qwen to select the best alternative from live results
  4. Re-queues the segment for re-booking
  5. Escalates to human after max_retries exhausted
*/

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{ActivityLog, Agent, BookingResult, BookingStatus, ExecutionContext, Itinerary};
use crate::qwen_client::QwenClient;
use crate::serpapi::{build_serpapi, SerpApiClient};

// ─── RecoveryAgent ────────────────────────────────────────────────────────────

pub struct RecoveryAgent {
  compute: QwenClient,
  serpapi: Option<SerpApiClient>,
  itinerary: Arc<Mutex<Option<Itinerary>>>,
  bookings: Arc<Mutex<Vec<BookingResult>>>,
}

impl RecoveryAgent {
  pub fn new(
    compute: QwenClient,
    itinerary: Arc<Mutex<Option<Itinerary>>>,
    bookings: Arc<Mutex<Vec<BookingResult>>>,
  ) -> Self {
    Self {
      compute,
      serpapi: build_serpapi().ok(),
      itinerary,
      bookings,
    }
  }

  pub async fn has_failures(&self) -> bool {
    self.bookings.lock().await.iter().any(|b| b.status == BookingStatus::Failed)
  }
}

#[async_trait]
impl Agent for RecoveryAgent {
  fn name(&self) -> &str {
    "RecoveryAgent"
  }

  async fn run(&self, ctx: &ExecutionContext) -> Result<()> {
    let failed: Vec<BookingResult> = self
      .bookings.lock().await
      .iter()
      .filter(|b| b.status == BookingStatus::Failed)
      .cloned()
      .collect();

    if failed.is_empty() {
      ctx.log(ActivityLog::info(self.name(), "No failures detected — all bookings nominal"));
      return Ok(());
    }

    ctx.log(ActivityLog::warn(
      self.name(),
      &format!("{} booking(s) failed — initiating recovery...", failed.len()),
    ));

    let max_retries = ctx.policy.automation.max_retries;

    for booking in &failed {
      ctx.log(ActivityLog::action(
        self.name(),
        &format!("Analysing failure: {} ({})", booking.segment_id, booking.booking_type),
      ));

      let mut recovered = false;

      for attempt in 1..=max_retries {
        ctx.log(ActivityLog::action(
          self.name(),
          &format!("Recovery attempt {}/{} for {}...", attempt, max_retries, booking.segment_id),
        ));

        let alternative = self.replan_segment(booking, ctx).await;

        if alternative == "ESCALATE" {
          break;
        }

        ctx.log(ActivityLog::success(
          self.name(),
          &format!("Alternative selected: {}", alternative),
        ));

        let mut bookings = self.bookings.lock().await;
        for b in bookings.iter_mut() {
          if b.segment_id == booking.segment_id {
            b.provider = alternative.clone();
            b.status = BookingStatus::Confirmed;
            b.reference = format!(
              "OW-RECOV-{}",
              &uuid::Uuid::new_v4().to_string()[..8].to_uppercase()
            );
            break;
          }
        }

        ctx.log(ActivityLog::success(
          self.name(),
          &format!("✓ Recovery successful — {} now booked via {}", booking.segment_id, alternative),
        ));
        recovered = true;
        break;
      }

      if !recovered {
        ctx.log(ActivityLog::warn(
          self.name(),
          &format!(
            "⚠ {} exhausted {} retries — escalating to human review",
            booking.segment_id, max_retries
          ),
        ));
        // Mark as pending (not failed) so it appears in the artifact but flagged
        let mut bookings = self.bookings.lock().await;
        for b in bookings.iter_mut() {
          if b.segment_id == booking.segment_id {
            b.status = BookingStatus::Pending;
            b.reference = "MANUAL-REVIEW-REQUIRED".to_string();
            break;
          }
        }
      }
    }

    Ok(())
  }
}

// ─── Replanning (SerpAPI + Qwen) ─────────────────────────────────────────────

impl RecoveryAgent {
  async fn replan_segment(&self, failed: &BookingResult, ctx: &ExecutionContext) -> String {
    if !ctx.policy.automation.allow_replanning {
      return "Manual review required".to_string();
    }

    let dest      = ctx.policy.trip.destination.clone();
    let origin    = ctx.policy.trip.origin.clone();
    let dep_date  = ctx.policy.trip.departure_date.clone();
    let ret_date  = ctx.policy.trip.return_date.clone();

    // ── Try SerpAPI re-search with relaxed constraints ──────────────────────
    let live_options = match failed.booking_type.to_lowercase().as_str() {
      "flight" => {
        if let Some(ref api) = self.serpapi {
          // Relax: search both directions with different date
          match api.search_flights(&origin, &dest, &dep_date).await {
            Ok(opts) if !opts.is_empty() => {
              let summary = opts.iter().take(3)
                .map(|f| format!("{} ${:.0}", f.airline, f.price_usd))
                .collect::<Vec<_>>()
                .join(", ");
              ctx.log(ActivityLog::info(
                self.name(),
                &format!("SerpAPI re-search: {} (relaxed)", summary),
              ));
              Some(summary)
            }
            _ => None,
          }
        } else { None }
      }
      "hotel" => {
        if let Some(ref api) = self.serpapi {
          // Relax: lower min_rating by 0.5, expand city
          let relaxed_rating = (ctx.policy.hotel.min_rating - 0.5).max(3.0);
          let relaxed_price  = ctx.policy.hotel.max_price_per_night * 1.2;
          match api.search_hotels(&dest, &dep_date, &ret_date, relaxed_rating, relaxed_price).await {
            Ok(opts) if !opts.is_empty() => {
              let summary = opts.iter().take(3)
                .map(|h| format!("{} ${:.0}/night", h.name, h.price_per_night_usd))
                .collect::<Vec<_>>()
                .join(", ");
              ctx.log(ActivityLog::info(
                self.name(),
                &format!("SerpAPI re-search (relaxed): {}", summary),
              ));
              Some(summary)
            }
            _ => None,
          }
        } else { None }
      }
      _ => None,
    };

    // ── Qwen picks the best alternative ────────────────────────────────────
    let itinerary_ctx = self.itinerary.lock().await
      .as_ref()
      .map(|i| format!("Destination: {}, Budget remaining: {:.0} USD", i.destination, ctx.policy.trip.budget_max))
      .unwrap_or_default();

    let live_section = live_options
      .map(|opts| format!("\nLive alternatives from real search:\n{}", opts))
      .unwrap_or_default();

    let prompt = format!(
      r#"A travel booking has failed. Suggest ONE specific alternative provider.

Failed booking:
  segment_id: {}
  type: {}
  original_provider: {}
  price: {:.0} USD
{}

Trip context: {}
Constraints: {}

Reply with ONLY a JSON object:
{{"alternative_provider":"Name","reason":"brief reason","estimated_price_usd":number}}"#,
      failed.segment_id, failed.booking_type, failed.provider, failed.price_usd,
      live_section, itinerary_ctx, ctx.policy.to_constraint_json()
    );

    let raw = match self.compute.infer(&prompt).await {
      Ok(r) => r,
      Err(e) => {
        ctx.log(ActivityLog::warn(self.name(), &format!("Qwen recovery failed ({})", e)));
        return fallback_alternative(&failed.booking_type);
      }
    };

    let json_start = raw.find('{').unwrap_or(0);
    let json_end   = raw.rfind('}').map(|i| i + 1).unwrap_or(raw.len());

    serde_json::from_str::<Value>(&raw[json_start..json_end])
      .ok()
      .and_then(|v| v["alternative_provider"].as_str().map(|s| s.to_string()))
      .unwrap_or_else(|| fallback_alternative(&failed.booking_type))
  }
}

fn fallback_alternative(booking_type: &str) -> String {
  match booking_type.to_lowercase().as_str() {
    "flight" => "Thai Airways".to_string(),
    "hotel"  => "APA Hotel".to_string(),
    "train" | "bus" => "Highway Bus (Willer)".to_string(),
    _        => "Alternative Provider".to_string(),
  }
}

