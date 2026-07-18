/*!
Open-Meteo Weather Client — free weather forecasts for PlannerAgent.

No API key required. Provides daily temperature and precipitation
data so PlannerAgent can warn about bad-weather activity days.

API docs: https://open-meteo.com/en/docs
*/

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const OPEN_METEO_BASE: &str = "https://api.open-meteo.com/v1/forecast";

// ─── Response types ───────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
struct WeatherResponse {
  daily: DailyData,
}

#[derive(Deserialize, Debug)]
struct DailyData {
  time:                  Vec<String>,
  temperature_2m_max:    Vec<Option<f64>>,
  temperature_2m_min:    Vec<Option<f64>>,
  precipitation_sum:     Vec<Option<f64>>,
  weathercode:           Vec<Option<u32>>,
}

// ─── Public types ─────────────────────────────────────────────────────────────

/// Weather summary for a single day.
#[derive(Debug, Clone, Serialize)]
pub struct WeatherDay {
  pub date: String,
  pub temp_max_c: f64,
  pub temp_min_c: f64,
  pub rain_mm: f64,
  pub condition: WeatherCondition,
  pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum WeatherCondition {
  Clear,
  PartlyCloudy,
  Overcast,
  Drizzle,
  Rain,
  Thunderstorm,
  Snow,
  Unknown,
}

impl WeatherCondition {
  pub fn as_emoji(&self) -> &str {
    match self {
      Self::Clear        => "☀️",
      Self::PartlyCloudy => "⛅",
      Self::Overcast     => "☁️",
      Self::Drizzle      => "🌦️",
      Self::Rain         => "🌧️",
      Self::Thunderstorm => "⛈️",
      Self::Snow         => "❄️",
      Self::Unknown      => "🌡️",
    }
  }

  /// True if outdoor activities should be warned about.
  pub fn is_adverse(&self) -> bool {
    matches!(self, Self::Rain | Self::Thunderstorm | Self::Snow)
  }
}

fn wmo_to_condition(code: u32) -> WeatherCondition {
  match code {
    0           => WeatherCondition::Clear,
    1..=2       => WeatherCondition::PartlyCloudy,
    3           => WeatherCondition::Overcast,
    45 | 48     => WeatherCondition::Overcast,        // fog
    51..=57     => WeatherCondition::Drizzle,
    61..=65     => WeatherCondition::Rain,
    71..=77     => WeatherCondition::Snow,
    80..=82     => WeatherCondition::Rain,
    85 | 86     => WeatherCondition::Snow,
    95..=99     => WeatherCondition::Thunderstorm,
    _           => WeatherCondition::Unknown,
  }
}

// ─── Client ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct WeatherClient {
  http: Client,
}

impl WeatherClient {
  pub fn new() -> Self {
    Self { http: Client::new() }
  }

  /// Fetch daily weather forecasts for a location by coordinates.
  /// Returns up to 16 days of forecast (Open-Meteo free tier limit).
  pub async fn forecast(
    &self,
    lat: f64,
    lon: f64,
    start_date: &str,
    end_date: &str,
  ) -> Result<Vec<WeatherDay>> {
    let resp = self
      .http
      .get(OPEN_METEO_BASE)
      .query(&[
        ("latitude",          &lat.to_string()),
        ("longitude",         &lon.to_string()),
        ("daily",             &"temperature_2m_max,temperature_2m_min,precipitation_sum,weathercode".to_string()),
        ("timezone",          &"auto".to_string()),
        ("start_date",        &start_date.to_string()),
        ("end_date",          &end_date.to_string()),
      ])
      .send()
      .await
      .context("Open-Meteo request failed")?;

    if !resp.status().is_success() {
      let status = resp.status();
      let body = resp.text().await.unwrap_or_default();
      anyhow::bail!("Open-Meteo returned {}: {}", status, body);
    }

    let data: WeatherResponse = resp
      .json()
      .await
      .context("Failed to parse Open-Meteo response")?;

    let days = data.daily.time.iter().enumerate().map(|(i, date)| {
      let temp_max  = data.daily.temperature_2m_max.get(i).and_then(|v| *v).unwrap_or(25.0);
      let temp_min  = data.daily.temperature_2m_min.get(i).and_then(|v| *v).unwrap_or(18.0);
      let rain_mm   = data.daily.precipitation_sum.get(i).and_then(|v| *v).unwrap_or(0.0);
      let wmo_code  = data.daily.weathercode.get(i).and_then(|v| *v).unwrap_or(0);
      let condition = wmo_to_condition(wmo_code);

      let warning = if condition.is_adverse() {
        Some(format!(
          "{} Rain/storm forecast ({:.1}mm) — check outdoor activities",
          condition.as_emoji(), rain_mm
        ))
      } else if temp_max > 35.0 {
        Some(format!("🌡️ Extreme heat ({:.0}°C) — limit midday outdoor exposure", temp_max))
      } else if temp_min < 0.0 {
        Some(format!("🥶 Below freezing ({:.0}°C) — pack warm layers", temp_min))
      } else {
        None
      };

      WeatherDay {
        date: date.clone(),
        temp_max_c: temp_max,
        temp_min_c: temp_min,
        rain_mm,
        condition,
        warning,
      }
    }).collect();

    Ok(days)
  }

  /// Convenience: fetch weather for a named city using a hardcoded
  /// coordinate lookup for common travel destinations.
  /// Falls back to Tokyo coordinates for unknown cities.
  pub async fn forecast_for_city(
    &self,
    city: &str,
    start_date: &str,
    end_date: &str,
  ) -> Result<Vec<WeatherDay>> {
    let (lat, lon) = city_coords(city);
    self.forecast(lat, lon, start_date, end_date).await
  }
}

impl Default for WeatherClient {
  fn default() -> Self { Self::new() }
}

// ─── City coordinate table ────────────────────────────────────────────────────

fn city_coords(city: &str) -> (f64, f64) {
  let lower = city.to_lowercase();
  // IATA codes and city names for common travel hubs
  match lower.as_str() {
    "tokyo" | "tyo" | "nrt" | "hnd"        => (35.6762, 139.6503),
    "osaka" | "kix"                         => (34.6937, 135.5023),
    "kyoto"                                 => (35.0116, 135.7681),
    "sapporo" | "cts"                       => (43.0642, 141.3469),
    "fukuoka" | "fuk"                       => (33.5904, 130.4017),
    "bangkok" | "bkk" | "dmk"              => (13.7563, 100.5018),
    "singapore" | "sin"                     => (1.3521,  103.8198),
    "hong kong" | "hkg"                     => (22.3193, 114.1694),
    "seoul" | "icn" | "gmp"                 => (37.5665, 126.9780),
    "taipei" | "tpe" | "tsa"               => (25.0330, 121.5654),
    "shanghai" | "pvg" | "sha"             => (31.2304, 121.4737),
    "beijing" | "pek"                       => (39.9042, 116.4074),
    "london" | "lhr" | "lgw"               => (51.5074, -0.1278),
    "paris" | "cdg" | "ory"                => (48.8566, 2.3522),
    "new york" | "nyc" | "jfk" | "lga"    => (40.7128, -74.0060),
    "los angeles" | "lax"                  => (34.0522, -118.2437),
    "sydney" | "syd"                        => (-33.8688, 151.2093),
    "dubai" | "dxb"                         => (25.2048, 55.2708),
    "kuala lumpur" | "kl" | "kul"          => (3.1390, 101.6869),
    "bali" | "denpasar" | "dps"            => (-8.3405, 115.0920),
    _                                       => (35.6762, 139.6503), // default: Tokyo
  }
}

// ─── Builder ──────────────────────────────────────────────────────────────────

/// Build WeatherClient — no env vars needed, Open-Meteo is free.
pub fn build_weather() -> WeatherClient {
  WeatherClient::new()
}
