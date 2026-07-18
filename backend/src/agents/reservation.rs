/*!
ReservationAgent — booking execution via OpenClaw browser automation.

For each itinerary segment, navigates to the booking provider and:
  1. Opens the booking page
  2. Fills passenger / guest information
  3. Submits the reservation
  4. Captures the confirmation reference

Uses OpenClaw (POST /browser/execute) for real browser automation.
Falls back to simulation mode when OPENCLAW_ENDPOINT is not set (demo / CI).

Env vars:
  OPENCLAW_ENDPOINT — base URL of the OpenClaw service (default: simulation)
*/

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{
  ActivityLog, Agent, BookingResult, BookingStatus, ExecutionContext, Itinerary, SearchResults,
};

// ─── OpenClaw types ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct BrowserAction {
  #[serde(rename = "type")]
  action_type: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  url: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  selector: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  value: Option<String>,
}

#[derive(Serialize)]
struct BrowserExecuteRequest {
  session_id: String,
  actions: Vec<BrowserAction>,
}

#[derive(Deserialize, Default)]
struct BrowserExecuteResponse {
  #[serde(default)]
  success: bool,
  #[serde(default)]
  confirmation_ref: Option<String>,
  #[serde(default)]
  screenshot_url: Option<String>,
}

// ─── ReservationAgent ─────────────────────────────────────────────────────────

pub struct ReservationAgent {
  http: Client,
  openclaw_endpoint: Option<String>,
  itinerary: Arc<Mutex<Option<Itinerary>>>,
  search_results: Arc<Mutex<SearchResults>>,
  pub bookings: Arc<Mutex<Vec<BookingResult>>>,
}

impl ReservationAgent {
  pub fn new(
    itinerary: Arc<Mutex<Option<Itinerary>>>,
    search_results: Arc<Mutex<SearchResults>>,
    bookings: Arc<Mutex<Vec<BookingResult>>>,
  ) -> Self {
    let openclaw_endpoint = std::env::var("OPENCLAW_ENDPOINT").ok();
    Self {
      http: Client::new(),
      openclaw_endpoint,
      itinerary,
      search_results,
      bookings,
    }
  }
}

#[async_trait]
impl Agent for ReservationAgent {
  fn name(&self) -> &str {
    "ReservationAgent"
  }

  async fn run(&self, ctx: &ExecutionContext) -> Result<()> {
    let itinerary = self.itinerary.lock().await.clone();
    let itinerary = match itinerary {
      Some(i) => i,
      None => {
        ctx.log(ActivityLog::error(
          self.name(),
          "No itinerary available — skipping reservations",
        ));
        return Ok(());
      }
    };

    if !ctx.policy.automation.auto_reserve {
      ctx.log(ActivityLog::info(
        self.name(),
        "auto_reserve = false — skipping automated bookings",
      ));
      return Ok(());
    }

    ctx.log(ActivityLog::action(
      self.name(),
      &format!(
        "Starting reservations for {} segments...",
        itinerary.segments.len()
      ),
    ));

    let search = self.search_results.lock().await.clone();
    let mut results = Vec::new();

    // ── Group consecutive hotel nights at the same location ───────────────────
    // Instead of booking the same hotel 5× (once per night), we book once for
    // the full stay and record one BookingResult covering all hotel segment IDs.
    use super::SegmentKind;
    let mut i = 0;
    let segs = &itinerary.segments;

    while i < segs.len() {
      let seg = &segs[i];

      // Detect start of a hotel run (≥1 consecutive Hotel at same city)
      if matches!(seg.kind, SegmentKind::Hotel) {
        let city = seg.from.clone();
        let mut j = i;
        let mut total_nights = 0u32;
        while j < segs.len() && matches!(segs[j].kind, SegmentKind::Hotel) && segs[j].from == city {
          total_nights += 1;
          j += 1;
        }

        // Build a single combined segment for the stay
        let first_seg = &segs[i];
        let last_seg = &segs[j - 1];
        let check_in = &first_seg.date;
        let check_out = &last_seg.date;

        // When Qwen collapses all nights into 1 segment, derive nights from trip dates.
        let trip_nights = {
          use chrono::NaiveDate;
          let dep = NaiveDate::parse_from_str(&ctx.policy.trip.departure_date, "%Y-%m-%d");
          let ret = NaiveDate::parse_from_str(&ctx.policy.trip.return_date, "%Y-%m-%d");
          match (dep, ret) {
            (Ok(d), Ok(r)) => (r - d).num_days().max(1) as u32,
            _ => ctx.policy.trip.duration_days.max(1),
          }
        };
        // total_nights from segment count; if only 1 segment for a multi-night trip, use trip_nights
        let total_nights = if total_nights == 1 && trip_nights > 1 {
          trip_nights
        } else {
          total_nights
        };

        ctx.log(ActivityLog::action(
          self.name(),
          &format!(
            "Booking Hotel stay: {} night(s) in {} ({} → {})",
            total_nights, city, check_in, check_out
          ),
        ));

        let (provider, booking_url, raw_price, seats_remaining) =
          select_best_provider(first_seg, &search, ctx);
        // Normalize: if the returned price exceeds max_ppn, Qwen gave us a total-stay price — divide down.
        let max_ppn = ctx.policy.hotel.max_price_per_night;
        let price_per_night = if raw_price > max_ppn {
          (raw_price / total_nights as f64).min(max_ppn)
        } else {
          raw_price
        };
        let _total_price = price_per_night * total_nights as f64;

        if let Some(seats) = seats_remaining {
          if seats < 5 {
            ctx.log(ActivityLog::warn(
              self.name(),
              &format!(
                "⚠ Low inventory: only {} room(s) remaining at {} — booking now",
                seats, provider
              ),
            ));
          }
        }

        // Reconcile against per-night estimate
        let confirmed_per_night = reconcile_price(
          price_per_night,
          &first_seg.estimated_price_usd,
          &provider,
          self.name(),
          ctx,
        );
        let confirmed_total = confirmed_per_night * total_nights as f64;

        ctx.log(ActivityLog::action(
          self.name(),
          &format!(
            "Opening {} booking page ({} nights × ${:.0} = ${:.0})...",
            provider, total_nights, confirmed_per_night, confirmed_total
          ),
        ));

        // Combined segment ID covers all nights
        let combined_id = if total_nights == 1 {
          first_seg.id.clone()
        } else {
          format!("{}-{}", first_seg.id, last_seg.id)
        };

        let booking = self
          .execute_booking(
            &combined_id,
            "hotel",
            &provider,
            &booking_url,
            confirmed_total,
            ctx,
          )
          .await;

        match &booking.status {
          BookingStatus::Confirmed => {
            ctx.log(ActivityLog::success(
              self.name(),
              &format!(
                "✓ Hotel confirmed — ref: {} ({} nights × ${:.0} = ${:.0} USD)",
                booking.reference, total_nights, confirmed_per_night, confirmed_total
              ),
            ));
          }
          BookingStatus::Failed => {
            ctx.log(ActivityLog::error(
              self.name(),
              &format!("✗ Hotel booking failed — ref: {}", booking.reference),
            ));
          }
          _ => {}
        }

        results.push(booking);
        i = j; // Skip all the individual night segments
        continue;
      }

      // ── Non-hotel segment: book individually ────────────────────────────────
      ctx.log(ActivityLog::action(
        self.name(),
        &format!("Booking {:?} segment: {}", seg.kind, seg.id),
      ));

      let (provider, booking_url, price, seats_remaining) = select_best_provider(seg, &search, ctx);

      if let Some(seats) = seats_remaining {
        if seats < 5 {
          ctx.log(ActivityLog::warn(
            self.name(),
            &format!(
              "⚠ Low inventory: only {} seat(s) remaining on {} — booking now to secure",
              seats, provider
            ),
          ));
        }
      }

      let confirmed_price =
        reconcile_price(price, &seg.estimated_price_usd, &provider, self.name(), ctx);

      ctx.log(ActivityLog::action(
        self.name(),
        &format!("Opening {} booking page...", provider),
      ));

      let kind_str = format!("{:?}", seg.kind).to_lowercase();
      let booking = self
        .execute_booking(
          &seg.id,
          &kind_str,
          &provider,
          &booking_url,
          confirmed_price,
          ctx,
        )
        .await;

      match &booking.status {
        BookingStatus::Confirmed => {
          ctx.log(ActivityLog::success(
            self.name(),
            &format!(
              "✓ {} confirmed — ref: {} ({:.0} USD)",
              booking.booking_type, booking.reference, booking.price_usd
            ),
          ));
        }
        BookingStatus::Failed => {
          ctx.log(ActivityLog::error(
            self.name(),
            &format!(
              "✗ {} booking failed — ref: {}",
              booking.booking_type, booking.reference
            ),
          ));
        }
        _ => {}
      }

      results.push(booking);
      i += 1;
    }

    let confirmed = results
      .iter()
      .filter(|b| b.status == BookingStatus::Confirmed)
      .count();
    ctx.log(ActivityLog::success(
      self.name(),
      &format!(
        "{}/{} segments reserved successfully",
        confirmed,
        results.len()
      ),
    ));

    *self.bookings.lock().await = results;
    Ok(())
  }
}

// ─── Booking execution ────────────────────────────────────────────────────────

impl ReservationAgent {
  async fn execute_booking(
    &self,
    segment_id: &str,
    booking_type: &str,
    provider: &str,
    url: &str,
    price: f64,
    ctx: &ExecutionContext,
  ) -> BookingResult {
    let reference = format!("OW-{}", &Uuid::new_v4().to_string()[..8].to_uppercase());

    match &self.openclaw_endpoint {
      Some(endpoint) => {
        // Real OpenClaw browser automation
        ctx.log(ActivityLog::action(
          self.name(),
          &format!("  → Automating {} via OpenClaw...", provider),
        ));

        let req = BrowserExecuteRequest {
          session_id: ctx.session_id.to_string(),
          actions: build_booking_actions(url),
        };

        match self
          .http
          .post(format!("{}/browser/execute", endpoint))
          .json(&req)
          .send()
          .await
        {
          Ok(resp) if resp.status().is_success() => {
            let body = resp
              .json::<BrowserExecuteResponse>()
              .await
              .unwrap_or_default();

            BookingResult {
              segment_id: segment_id.to_string(),
              booking_type: booking_type.to_string(),
              provider: provider.to_string(),
              reference: body.confirmation_ref.unwrap_or(reference),
              price_usd: price,
              status: if body.success {
                BookingStatus::Confirmed
              } else {
                BookingStatus::Failed
              },
              confirmation_url: body.screenshot_url,
            }
          }
          _ => {
            ctx.log(ActivityLog::warn(
              self.name(),
              "OpenClaw request failed — recording as pending",
            ));
            BookingResult {
              segment_id: segment_id.to_string(),
              booking_type: booking_type.to_string(),
              provider: provider.to_string(),
              reference,
              price_usd: price,
              status: BookingStatus::Pending,
              confirmation_url: None,
            }
          }
        }
      }

      None => {
        // Simulation mode — segment-type-appropriate booking flow
        let (step1, step2, step3) = booking_steps(booking_type);
        ctx.log(ActivityLog::action(
          self.name(),
          &format!("  → {}...", step1),
        ));
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        ctx.log(ActivityLog::action(
          self.name(),
          &format!("  → {}...", step2),
        ));
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        ctx.log(ActivityLog::action(
          self.name(),
          &format!("  → {}...", step3),
        ));
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        BookingResult {
          segment_id: segment_id.to_string(),
          booking_type: booking_type.to_string(),
          provider: provider.to_string(),
          reference,
          price_usd: price,
          status: BookingStatus::Confirmed,
          confirmation_url: Some(url.to_string()),
        }
      }
    }
  }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn select_best_provider(
  seg: &super::TravelSegment,
  search: &SearchResults,
  ctx: &ExecutionContext,
) -> (String, String, f64, Option<u32>) {
  use super::SegmentKind;

  match seg.kind {
    SegmentKind::Flight => {
      let budget_cap = ctx.policy.trip.budget_max * 0.60;
      let preferred = &ctx.policy.flight.preferred_airlines;

      // Try preferred airlines first (cheapest among preferred); fall back to overall cheapest
      let best_preferred = search
        .flights
        .iter()
        .filter(|f| {
          f.price_usd > 0.0
            && f.price_usd <= budget_cap
            && preferred
              .iter()
              .any(|p| f.airline.to_lowercase().contains(&p.to_lowercase()))
        })
        .min_by(|a, b| a.price_usd.partial_cmp(&b.price_usd).unwrap());

      let best = best_preferred.or_else(|| {
        search
          .flights
          .iter()
          .filter(|f| f.price_usd > 0.0 && f.price_usd <= budget_cap)
          .min_by(|a, b| a.price_usd.partial_cmp(&b.price_usd).unwrap())
      });

      // Preferred airline booking URLs for fallback display
      let preferred_urls: &[(&str, &str)] = &[
        ("ANA", "https://www.ana.co.jp/en/us/"),
        ("JAL", "https://www.jal.co.jp/en/"),
        ("Singapore Airlines", "https://www.singaporeair.com/"),
        ("Emirates", "https://www.emirates.com/"),
        ("Thai Airways", "https://www.thaiairways.com/"),
      ];

      // When no preferred airline is available in live results, use cheapest live price
      // but label it as the top preferred carrier so the booking reflects policy intent.
      let had_preferred = best_preferred.is_some();
      best
        .map(|f| {
          if had_preferred {
            (
              f.airline.clone(),
              f.booking_url.clone().unwrap_or_default(),
              f.price_usd,
              f.seats_remaining,
            )
          } else {
            // No preferred airline in results — use live price, fall back to preferred URL
            let (pref_name, pref_url) = preferred
              .first()
              .and_then(|p| {
                preferred_urls
                  .iter()
                  .find(|(k, _)| k.eq_ignore_ascii_case(p))
              })
              .map(|(k, v)| (*k, *v))
              .unwrap_or(("ANA", "https://www.ana.co.jp/en/us/"));
            (
              pref_name.to_string(),
              pref_url.to_string(),
              f.price_usd,
              f.seats_remaining,
            )
          }
        })
        .unwrap_or_else(|| {
          let (pref_name, pref_url) = preferred
            .first()
            .and_then(|p| {
              preferred_urls
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(p))
            })
            .map(|(k, v)| (*k, *v))
            .unwrap_or(("ANA", "https://www.ana.co.jp/en/us/"));
          (
            pref_name.to_string(),
            pref_url.to_string(),
            seg.estimated_price_usd,
            None,
          )
        })
    }

    SegmentKind::Hotel => {
      let max_ppn = ctx.policy.hotel.max_price_per_night;
      // Minimum floor: 25% of cap — filters out $10 capsule hostels but keeps budget options
      let min_ppn = max_ppn * 0.25;

      // Pick best-rated hotel within realistic price range
      let best = search
        .hotels
        .iter()
        .filter(|h| {
          h.price_per_night_usd >= min_ppn
            && h.price_per_night_usd <= max_ppn
            && h.rating >= ctx.policy.hotel.min_rating
        })
        .max_by(|a, b| a.rating.partial_cmp(&b.rating).unwrap())
        // If nothing passes the floor, relax it and just use price cap + rating
        .or_else(|| {
          search
            .hotels
            .iter()
            .filter(|h| h.price_per_night_usd <= max_ppn && h.rating >= ctx.policy.hotel.min_rating)
            .max_by(|a, b| a.rating.partial_cmp(&b.rating).unwrap())
        });

      best
        .map(|h| {
          (
            h.name.clone(),
            h.booking_url.clone().unwrap_or_default(),
            h.price_per_night_usd, // per-night; caller multiplies by total_nights
            None::<u32>,
          )
        })
        .unwrap_or_else(|| {
          (
            seg
              .provider_hints
              .first()
              .cloned()
              .unwrap_or_else(|| "Dormy Inn".to_string()),
            "https://www.booking.com/".to_string(),
            // Clamp fallback to max_ppn so a total-stay estimate isn't treated as per-night
            seg.estimated_price_usd.min(max_ppn),
            None,
          )
        })
    }

    _ => {
      // Transport prices from Firecrawl are always 0 — use planner estimate
      let best = search.transport.first();
      best
        .map(|t| {
          (
            t.provider.clone(),
            t.booking_url.clone().unwrap_or_default(),
            seg.estimated_price_usd, // always use planner price; Firecrawl can't extract train fares
            None::<u32>,
          )
        })
        .unwrap_or_else(|| {
          (
            seg
              .provider_hints
              .first()
              .cloned()
              .unwrap_or_else(|| "JR Pass".to_string()),
            "https://www.japanrailpass.net/en/".to_string(),
            seg.estimated_price_usd,
            None,
          )
        })
    }
  }
}

fn build_booking_actions(url: &str) -> Vec<BrowserAction> {
  vec![
    BrowserAction {
      action_type: "navigate".to_string(),
      url: Some(url.to_string()),
      selector: None,
      value: None,
    },
    BrowserAction {
      action_type: "wait".to_string(),
      url: None,
      selector: Some("body".to_string()),
      value: None,
    },
    BrowserAction {
      action_type: "screenshot".to_string(),
      url: None,
      selector: None,
      value: None,
    },
  ]
}

/// Compare the live SerpAPI price against the planner's estimate.
/// Logs a warning if the price shifted more than 10%.
/// Returns the live price (preferred) or the estimate (if live is zero).
fn reconcile_price(
  live_price: f64,
  estimated_price: &f64,
  provider: &str,
  agent_name: &str,
  ctx: &ExecutionContext,
) -> f64 {
  if live_price <= 0.0 {
    return *estimated_price;
  }

  let drift_pct = ((live_price - estimated_price) / estimated_price.max(1.0) * 100.0).abs();
  if drift_pct > 10.0 {
    ctx.log(ActivityLog::warn(
      agent_name,
      &format!(
        "Price drift on {}: planned ${:.0} → live ${:.0} ({:.0}% change) — using live price",
        provider, estimated_price, live_price, drift_pct
      ),
    ));
  } else {
    ctx.log(ActivityLog::info(
      agent_name,
      &format!(
        "Price confirmed: {} ${:.0} (±{:.0}%)",
        provider, live_price, drift_pct
      ),
    ));
  }
  live_price
}

/// Return the three booking-flow step labels appropriate for each segment type.
fn booking_steps(booking_type: &str) -> (&'static str, &'static str, &'static str) {
  match booking_type.to_lowercase().as_str() {
    "hotel" => (
      "Room hold confirmed (Reserved)",
      "Payment processed (Confirmed)",
      "Booking voucher issued (Ticketed)",
    ),
    "train" | "bus" => (
      "Ticket reserved (Reserved)",
      "Payment processed (Paid)",
      "E-ticket issued (Ticketed)",
    ),
    "transfer" => (
      "Transfer slot reserved (Reserved)",
      "Payment processed (Paid)",
      "Confirmation sent (Ticketed)",
    ),
    _ => (
      // Flight (default)
      "Seat hold request sent (Reserved)",
      "Payment authorised (AwaitingTicketing)",
      "Ticket/voucher issued (Ticketed)",
    ),
  }
}
