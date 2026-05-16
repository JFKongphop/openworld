/*!
Travel specification parser — reads travel.md YAML into a TravelPolicy.

travel.md format:
  trip:
    destination: Japan        # country  → multi-city itinerary
    # OR use an IATA city code for a single-city trip:
    # destination: TYO        # Tokyo only  (no inter-city travel)
    # destination: BKK        # Bangkok only
    duration_days: 5
    budget_max: 1200 USD
  flight:
    max_stops: 1
    avoid_red_eye: true
    preferred_airlines: [ANA, JAL]
  hotel:
    min_rating: 4.0
    max_price_per_night: 120 USD
    near_station: true
  transport:
    prefer_train: true
    avoid_overnight_bus: true
  automation:
    auto_reserve: true
    retry_on_failure: true
    allow_replanning: true
  vault:
    auto_payment: true
    max_single_transaction: 300 USD
*/

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ─── Policy Structs ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TripConfig {
  pub destination: String,
  /// Departure city / airport (e.g. "Bangkok", "BKK"). Defaults to "Bangkok".
  #[serde(default = "default_origin")]
  pub origin: String,
  #[serde(default)]
  pub departure_date: String,
  #[serde(default)]
  pub return_date: String,
  #[serde(default = "default_duration")]
  pub duration_days: u32,
  /// Budget in USD, parsed from "1200 USD" or plain "1200"
  #[serde(default = "default_budget", deserialize_with = "parse_budget")]
  pub budget_max: f64,
  /// Owner wallet address — the EVM address that will receive the journey NFT.
  /// If omitted, the operator wallet is used as the owner.
  #[serde(default)]
  pub owner: Option<String>,
}

fn default_origin() -> String {
  "Bangkok".to_string()
}

fn default_duration() -> u32 {
  5
}
fn default_budget() -> f64 {
  1200.0
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlightConfig {
  #[serde(default)]
  pub max_stops: u32,
  #[serde(default)]
  pub avoid_red_eye: bool,
  #[serde(default)]
  pub preferred_airlines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HotelConfig {
  #[serde(default = "default_rating")]
  pub min_rating: f64,
  /// Max price per night in USD
  #[serde(default = "default_hotel_price", deserialize_with = "parse_budget")]
  pub max_price_per_night: f64,
  #[serde(default)]
  pub near_station: bool,
}

fn default_rating() -> f64 {
  3.5
}
fn default_hotel_price() -> f64 {
  150.0
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransportConfig {
  #[serde(default)]
  pub prefer_train: bool,
  #[serde(default)]
  pub avoid_overnight_bus: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutomationConfig {
  #[serde(default = "default_true")]
  pub auto_reserve: bool,
  #[serde(default = "default_true")]
  pub retry_on_failure: bool,
  #[serde(default = "default_true")]
  pub allow_replanning: bool,
  #[serde(default = "default_retries")]
  pub max_retries: u32,
}

fn default_true() -> bool {
  true
}
fn default_retries() -> u32 {
  3
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultConfig {
  #[serde(default = "default_true")]
  pub auto_payment: bool,
  /// Max single transaction in USD
  #[serde(default = "default_tx_limit", deserialize_with = "parse_budget")]
  pub max_single_transaction: f64,
}

fn default_tx_limit() -> f64 {
  300.0
}

// ─── Top-level Policy ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TravelPolicy {
  pub trip: TripConfig,
  #[serde(default)]
  pub flight: FlightConfig,
  #[serde(default)]
  pub hotel: HotelConfig,
  #[serde(default)]
  pub transport: TransportConfig,
  #[serde(default)]
  pub automation: AutomationConfig,
  #[serde(default)]
  pub vault: VaultConfig,
}

impl TravelPolicy {
  /// Validate the policy and return errors if any constraints are invalid
  pub fn validate(&self) -> Vec<String> {
    let mut errors = Vec::new();
    if self.trip.destination.is_empty() {
      errors.push("trip.destination is required".to_string());
    }
    if self.trip.budget_max <= 0.0 {
      errors.push("trip.budget_max must be greater than 0".to_string());
    }
    if self.vault.max_single_transaction > self.trip.budget_max {
      errors.push(
        "vault.max_single_transaction cannot exceed trip.budget_max".to_string(),
      );
    }
    errors
  }

  /// Returns true when destination is an IATA city/airport code (2–4 uppercase letters/digits, e.g. "TYO", "BKK").
  /// A code triggers single-city mode — no inter-city transport is planned.
  pub fn is_city_code(&self) -> bool {
    let d = self.trip.destination.trim();
    d.len() >= 2
      && d.len() <= 4
      && d.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
  }

  /// Resolve a known IATA city code to its full city name for display/prompt purposes.
  /// Falls back to the code itself when unknown.
  pub fn resolved_city_name(&self) -> &str {
    if !self.is_city_code() {
      return &self.trip.destination;
    }
    match self.trip.destination.trim() {
      "BKK" => "Bangkok",
      "TYO" | "NRT" | "HND" => "Tokyo",
      "OSA" | "KIX" | "ITM" => "Osaka",
      "SIN" => "Singapore",
      "KUL" => "Kuala Lumpur",
      "HKG" => "Hong Kong",
      "NYC" | "JFK" | "LGA" | "EWR" => "New York",
      "LAX" => "Los Angeles",
      "LHR" | "LGW" | "STN" => "London",
      "CDG" | "ORY" => "Paris",
      "DXB" | "AUH" => "Dubai",
      "SYD" => "Sydney",
      "ICN" | "GMP" => "Seoul",
      "TPE" | "TSA" => "Taipei",
      "SFO" => "San Francisco",
      "ORD" | "MDW" => "Chicago",
      "MIA" => "Miami",
      "AMS" => "Amsterdam",
      "FRA" => "Frankfurt",
      "BCN" => "Barcelona",
      "MAD" => "Madrid",
      "FCO" | "CIA" => "Rome",
      "IST" => "Istanbul",
      "MNL" => "Manila",
      "CGK" | "HLP" => "Jakarta",
      "BOM" => "Mumbai",
      "DEL" => "Delhi",
      "PEK" | "PKX" => "Beijing",
      "PVG" | "SHA" => "Shanghai",
      code => code,
    }
  }

  /// Serialise the policy to a compact JSON constraint object (for agent prompts)
  pub fn to_constraint_json(&self) -> String {
    serde_json::json!({
      "origin": self.trip.origin,
      "departure_date": self.trip.departure_date,
      "return_date": self.trip.return_date,
      "destination": self.trip.destination,
      "destination_city": self.resolved_city_name(),
      "single_city_mode": self.is_city_code(),
      "duration_days": self.trip.duration_days,
      "budget_max_usd": self.trip.budget_max,
      "max_stops": self.flight.max_stops,
      "avoid_red_eye": self.flight.avoid_red_eye,
      "preferred_airlines": self.flight.preferred_airlines,
      "hotel_min_rating": self.hotel.min_rating,
      "hotel_max_per_night_usd": self.hotel.max_price_per_night,
      "near_station": self.hotel.near_station,
      "prefer_train": self.transport.prefer_train,
      "avoid_overnight_bus": self.transport.avoid_overnight_bus,
      "auto_reserve": self.automation.auto_reserve,
      "retry_on_failure": self.automation.retry_on_failure,
      "allow_replanning": self.automation.allow_replanning,
      "max_single_transaction_usd": self.vault.max_single_transaction
    })
    .to_string()
  }
}

// ─── Parser ───────────────────────────────────────────────────────────────────

/// Parse travel.md YAML content into a TravelPolicy
pub fn parse_travel_md(content: &str) -> Result<TravelPolicy> {
  serde_yaml::from_str(content).context("Failed to parse travel.md YAML")
}

// ─── Budget deserialiser ("1200 USD" → 1200.0) ────────────────────────────────

fn parse_budget<'de, D>(deserializer: D) -> std::result::Result<f64, D::Error>
where
  D: serde::Deserializer<'de>,
{
  use serde::de::Error;
  let value = serde_json::Value::deserialize(deserializer)?;
  match value {
    serde_json::Value::Number(n) => n
      .as_f64()
      .ok_or_else(|| D::Error::custom("invalid number")),
    serde_json::Value::String(s) => {
      let cleaned = s.split_whitespace().next().unwrap_or("0");
      cleaned
        .replace(',', "")
        .parse::<f64>()
        .map_err(|_| D::Error::custom(format!("cannot parse budget: {}", s)))
    }
    _ => Err(D::Error::custom("budget must be a number or 'NNN USD'")),
  }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_full_policy() {
    let yaml = r#"
trip:
  destination: Japan
  duration_days: 5
  budget_max: "1200 USD"
flight:
  max_stops: 1
  avoid_red_eye: true
  preferred_airlines: [ANA, JAL]
hotel:
  min_rating: 4.0
  max_price_per_night: "120 USD"
  near_station: true
transport:
  prefer_train: true
  avoid_overnight_bus: true
automation:
  auto_reserve: true
  retry_on_failure: true
  allow_replanning: true
vault:
  auto_payment: true
  max_single_transaction: "300 USD"
"#;
    let policy = parse_travel_md(yaml).unwrap();
    assert_eq!(policy.trip.destination, "Japan");
    assert_eq!(policy.trip.budget_max, 1200.0);
    assert!(policy.flight.avoid_red_eye);
    assert!(policy.transport.prefer_train);
  }
}
