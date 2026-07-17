/*!
SerpAPI Client — real flight and hotel search via Google (SerpAPI proxy).

Uses SerpAPI's Google Flights and Google Hotels engines to return
live prices and availability, replacing LLM-hallucinated prices
with real inventory data for the SearchAgent.

Env vars:
  SERPAPI_KEY — SerpAPI API key (bd6996...)
*/

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

use crate::agents::{FlightOption, HotelOption};

const SERPAPI_BASE: &str = "https://serpapi.com/search.json";

// ─── Client ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SerpApiClient {
  api_key: String,
  http: Client,
}

impl SerpApiClient {
  pub fn new(api_key: String) -> Self {
    Self { api_key, http: Client::new() }
  }
}

// ─── Google Flights ───────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
struct FlightsResponse {
  #[serde(default)]
  best_flights: Vec<FlightGroup>,
  #[serde(default)]
  other_flights: Vec<FlightGroup>,
}

#[derive(Deserialize, Debug)]
struct FlightGroup {
  #[serde(default)]
  flights: Vec<FlightLeg>,
  #[serde(default)]
  price: Option<f64>,
  #[serde(default)]
  layovers: Vec<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct FlightLeg {
  #[serde(default)]
  airline: Option<String>,
  #[serde(default)]
  departure_airport: Option<AirportInfo>,
  #[serde(default)]
  arrival_airport: Option<AirportInfo>,
  #[serde(default)]
  duration: Option<u32>,
  #[allow(dead_code)]
  #[serde(default)]
  flight_number: Option<String>,
}

#[derive(Deserialize, Debug)]
struct AirportInfo {
  #[serde(default)]
  time: Option<String>,
  #[allow(dead_code)]
  #[serde(default)]
  id: Option<String>,
}

impl SerpApiClient {
  /// Search Google Flights for one-way economy fares.
  /// `origin` and `dest` are IATA codes (e.g. "BKK", "NRT").
  /// Returns up to 5 real FlightOption entries.
  pub async fn search_flights(
    &self,
    origin: &str,
    dest: &str,
    date: &str,
  ) -> Result<Vec<FlightOption>> {
    let resp = self
      .http
      .get(SERPAPI_BASE)
      .query(&[
        ("engine",              "google_flights"),
        ("departure_id",        origin),
        ("arrival_id",          dest),
        ("outbound_date",       date),
        ("currency",            "USD"),
        ("hl",                  "en"),
        ("type",                "2"),  // one-way
        ("travel_class",        "1"),  // economy
        ("api_key",             &self.api_key),
      ])
      .send()
      .await
      .context("SerpAPI flights request failed")?;

    if !resp.status().is_success() {
      let status = resp.status();
      let body = resp.text().await.unwrap_or_default();
      anyhow::bail!("SerpAPI flights returned {}: {}", status, body);
    }

    let data: FlightsResponse = resp
      .json()
      .await
      .context("Failed to parse SerpAPI flights response")?;

    let mut options: Vec<FlightOption> = data
      .best_flights
      .iter()
      .chain(data.other_flights.iter())
      .filter_map(|g| flight_group_to_option(g, origin, dest))
      .take(5)
      .collect();

    // Deduplicate by airline+price
    options.dedup_by(|a, b| a.airline == b.airline && (a.price_usd - b.price_usd).abs() < 1.0);

    Ok(options)
  }
}

fn flight_group_to_option(g: &FlightGroup, origin: &str, dest: &str) -> Option<FlightOption> {
  let price = g.price?;
  if price <= 0.0 { return None; }

  let first_leg = g.flights.first()?;
  let airline   = first_leg.airline.clone().unwrap_or_else(|| "Unknown".to_string());
  let departure = first_leg
    .departure_airport
    .as_ref()
    .and_then(|a| a.time.clone())
    .unwrap_or_else(|| "—".to_string());

  let last_leg  = g.flights.last()?;
  let arrival   = last_leg
    .arrival_airport
    .as_ref()
    .and_then(|a| a.time.clone())
    .unwrap_or_else(|| "—".to_string());

  // Total duration = sum of all leg durations (minutes → "Xh Ym")
  let total_mins: u32 = g.flights.iter().filter_map(|l| l.duration).sum::<u32>()
    + g.layovers.iter()
        .filter_map(|l| l["duration"].as_u64())
        .sum::<u64>() as u32;
  let duration = if total_mins > 0 {
    format!("{}h{}m", total_mins / 60, total_mins % 60)
  } else {
    "—".to_string()
  };

  let stops = g.layovers.len() as u32;

  Some(FlightOption {
    airline,
    route: format!("{origin} → {dest}"),
    departure,
    arrival,
    stops,
    duration,
    price_usd: price,
    booking_url: None,
  })
}

// ─── Google Hotels ────────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
struct HotelsResponse {
  #[serde(default)]
  properties: Vec<HotelProperty>,
}

#[derive(Deserialize, Debug)]
struct HotelProperty {
  #[serde(default)]
  name: Option<String>,
  #[serde(default)]
  overall_rating: Option<f64>,
  #[serde(default)]
  rate_per_night: Option<NightRate>,
  #[serde(default)]
  neighborhood: Option<String>,
  #[serde(default)]
  link: Option<String>,
  #[serde(default)]
  amenities: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct NightRate {
  #[serde(default)]
  extracted_lowest: Option<f64>,
  #[allow(dead_code)]
  #[serde(default)]
  lowest: Option<String>,
}

impl SerpApiClient {
  /// Search Google Hotels for available rooms.
  /// `city` is a plain name (e.g. "Tokyo", "Kyoto").
  /// `checkin` / `checkout` are YYYY-MM-DD strings.
  /// Returns up to 5 real HotelOption entries.
  pub async fn search_hotels(
    &self,
    city: &str,
    checkin: &str,
    checkout: &str,
    min_rating: f64,
    max_per_night: f64,
  ) -> Result<Vec<HotelOption>> {
    let resp = self
      .http
      .get(SERPAPI_BASE)
      .query(&[
        ("engine",       "google_hotels"),
        ("q",            &format!("hotels in {city}")),
        ("check_in_date",  checkin),
        ("check_out_date", checkout),
        ("currency",     "USD"),
        ("hl",           "en"),
        ("adults",       "1"),
        ("api_key",      &self.api_key),
      ])
      .send()
      .await
      .context("SerpAPI hotels request failed")?;

    if !resp.status().is_success() {
      let status = resp.status();
      let body = resp.text().await.unwrap_or_default();
      anyhow::bail!("SerpAPI hotels returned {}: {}", status, body);
    }

    let data: HotelsResponse = resp
      .json()
      .await
      .context("Failed to parse SerpAPI hotels response")?;

    let options: Vec<HotelOption> = data
      .properties
      .iter()
      .filter_map(|p| hotel_property_to_option(p, city, min_rating, max_per_night))
      .take(5)
      .collect();

    Ok(options)
  }
}

fn hotel_property_to_option(
  p: &HotelProperty,
  city: &str,
  min_rating: f64,
  max_per_night: f64,
) -> Option<HotelOption> {
  let name   = p.name.clone()?;
  let rating = p.overall_rating.unwrap_or(0.0);
  if rating < min_rating { return None; }

  let price = p
    .rate_per_night
    .as_ref()
    .and_then(|r| r.extracted_lowest)
    .unwrap_or(0.0);
  if price <= 0.0 || price > max_per_night { return None; }

  let near_station = p.amenities.iter().any(|a| {
    let a = a.to_lowercase();
    a.contains("transit") || a.contains("station") || a.contains("metro")
  });

  let location = p
    .neighborhood
    .clone()
    .unwrap_or_else(|| city.to_string());

  Some(HotelOption {
    name,
    location,
    price_per_night_usd: price,
    rating,
    near_station,
    booking_url: p.link.clone(),
  })
}

// ─── Builder ──────────────────────────────────────────────────────────────────

/// Build SerpApiClient from SERPAPI_KEY env var.
pub fn build_serpapi() -> anyhow::Result<SerpApiClient> {
  let key = std::env::var("SERPAPI_KEY")
    .context("SERPAPI_KEY not set in .env")?;
  Ok(SerpApiClient::new(key))
}
