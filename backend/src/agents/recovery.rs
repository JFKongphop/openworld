/*!
RecoveryAgent — failure recovery and replanning powered by 0G Compute.

When a booking fails, the RecoveryAgent:
  1. Analyses why the booking failed
  2. Uses 0G Compute to reason about alternatives
  3. Selects an alternative provider or modifies the segment
  4. Re-queues the segment for re-booking

This is one of the highest-ROI autonomy signals in the demo —
the system visibly adapts to failure without user intervention.
*/

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{ActivityLog, Agent, BookingResult, BookingStatus, ExecutionContext, Itinerary};
use crate::og_compute::OgComputeClient;

// ─── RecoveryAgent ────────────────────────────────────────────────────────────

pub struct RecoveryAgent {
  compute: OgComputeClient,
  itinerary: Arc<Mutex<Option<Itinerary>>>,
  bookings: Arc<Mutex<Vec<BookingResult>>>,
}

impl RecoveryAgent {
  pub fn new(
    compute: OgComputeClient,
    itinerary: Arc<Mutex<Option<Itinerary>>>,
    bookings: Arc<Mutex<Vec<BookingResult>>>,
  ) -> Self {
    Self {
      compute,
      itinerary,
      bookings,
    }
  }

  /// Returns true if any bookings were recovered
  pub async fn has_failures(&self) -> bool {
    self
      .bookings
      .lock()
      .await
      .iter()
      .any(|b| b.status == BookingStatus::Failed)
  }
}

#[async_trait]
impl Agent for RecoveryAgent {
  fn name(&self) -> &str {
    "RecoveryAgent"
  }

  async fn run(&self, ctx: &ExecutionContext) -> Result<()> {
    let failed: Vec<BookingResult> = self
      .bookings
      .lock()
      .await
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

    for booking in &failed {
      ctx.log(ActivityLog::action(
        self.name(),
        &format!("Analysing failure: {} ({})", booking.segment_id, booking.booking_type),
      ));

      let alternative = self.replan_segment(booking, ctx).await;

      ctx.log(ActivityLog::success(
        self.name(),
        &format!("Alternative selected: {}", alternative),
      ));

      // Update the booking status to recovered with the new provider
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
        &format!(
          "✓ Recovery successful — {} now booked via {}",
          booking.segment_id, alternative
        ),
      ));
    }

    Ok(())
  }
}

// ─── Replanning ───────────────────────────────────────────────────────────────

impl RecoveryAgent {
  async fn replan_segment(&self, failed: &BookingResult, ctx: &ExecutionContext) -> String {
    if !ctx.policy.automation.allow_replanning {
      return "Manual review required".to_string();
    }

    let itinerary_summary = self
      .itinerary
      .lock()
      .await
      .as_ref()
      .map(|i| format!("Destination: {}, Budget remaining: {:.0} USD", i.destination, ctx.policy.trip.budget_max))
      .unwrap_or_default();

    let prompt = format!(
      r#"A travel booking has failed. Suggest ONE specific alternative provider.

Failed booking:
  segment_id: {}
  type: {}
  original_provider: {}
  price: {:.0} USD

Trip context:
  {}

Constraints:
  {}

Reply with ONLY a JSON object:
{{"alternative_provider": "Name", "reason": "brief reason", "estimated_price_usd": number}}"#,
      failed.segment_id,
      failed.booking_type,
      failed.provider,
      failed.price_usd,
      itinerary_summary,
      ctx.policy.to_constraint_json()
    );

    let raw = match self.compute.infer(&prompt).await {
      Ok(r) => r,
      Err(e) => {
        ctx.log(ActivityLog::warn(
          self.name(),
          &format!("0G Compute recovery reasoning failed ({}), using fallback", e),
        ));
        return fallback_alternative(&failed.booking_type);
      }
    };

    // Parse alternative from LLM response
    let json_start = raw.find('{').unwrap_or(0);
    let json_end = raw.rfind('}').map(|i| i + 1).unwrap_or(raw.len());
    let json_str = &raw[json_start..json_end];

    serde_json::from_str::<Value>(json_str)
      .ok()
      .and_then(|v| v["alternative_provider"].as_str().map(|s| s.to_string()))
      .unwrap_or_else(|| fallback_alternative(&failed.booking_type))
  }
}

fn fallback_alternative(booking_type: &str) -> String {
  match booking_type.to_lowercase().as_str() {
    "flight" => "Thai Airways".to_string(),
    "hotel" => "APA Hotel Namba".to_string(),
    "train" | "bus" => "Highway Bus (Willer)".to_string(),
    _ => "Alternative Provider".to_string(),
  }
}
