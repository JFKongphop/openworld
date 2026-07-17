/*!
Mapbox Directions Client — route feasibility validation for PlannerAgent.

Calls the Mapbox Directions API to verify that daily activity plans
are physically achievable — no 4-hour drives crammed into a morning.

Env vars:
  MAPBOX_ACCESS_TOKEN — Mapbox public token (pk.eyJ1...)
*/

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

const MAPBOX_DIRECTIONS_BASE: &str = "https://api.mapbox.com/directions/v5/mapbox";

// ─── Response types ───────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
struct DirectionsResponse {
  #[serde(default)]
  routes: Vec<Route>,
  code: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Route {
  duration: f64,
  #[allow(dead_code)]
  distance: f64,
}

// ─── Client ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct MapboxClient {
  access_token: String,
  http: Client,
}

impl MapboxClient {
  pub fn new(access_token: String) -> Self {
    Self { access_token, http: Client::new() }
  }

  /// Get driving duration in minutes between two coordinate pairs.
  /// `from` and `to` are (longitude, latitude) tuples.
  pub async fn driving_minutes(
    &self,
    from: (f64, f64),
    to: (f64, f64),
  ) -> Result<u32> {
    self.route_minutes("driving", from, to).await
  }

  /// Get walking duration in minutes between two coordinate pairs.
  pub async fn walking_minutes(
    &self,
    from: (f64, f64),
    to: (f64, f64),
  ) -> Result<u32> {
    self.route_minutes("walking", from, to).await
  }

  /// Geocode a place name to (longitude, latitude) using Mapbox Geocoding API.
  pub async fn geocode(&self, place: &str) -> Result<(f64, f64)> {
    let url = format!(
      "https://api.mapbox.com/geocoding/v5/mapbox.places/{}.json",
      urlencoding::encode(place)
    );

    #[derive(Deserialize)]
    struct GeoResponse { features: Vec<GeoFeature> }
    #[derive(Deserialize)]
    struct GeoFeature { center: Vec<f64> }

    let resp: GeoResponse = self
      .http
      .get(&url)
      .query(&[("access_token", &self.access_token), ("limit", &"1".to_string())])
      .send()
      .await
      .context("Mapbox geocoding request failed")?
      .json()
      .await
      .context("Failed to parse Mapbox geocoding response")?;

    let center = resp.features
      .into_iter()
      .next()
      .context("Mapbox geocoding returned no results")?
      .center;

    Ok((center[0], center[1]))
  }

  /// Validate that a list of locations can be visited in order within a day.
  /// Returns (is_feasible, total_transit_minutes, warning_message).
  pub async fn validate_day_plan(
    &self,
    locations: &[String],
  ) -> (bool, u32, Option<String>) {
    if locations.len() < 2 {
      return (true, 0, None);
    }

    let mut total_minutes = 0u32;
    let mut failed_geocodes = 0usize;

    for window in locations.windows(2) {
      let from_name = &window[0];
      let to_name   = &window[1];

      let coords = tokio::join!(
        self.geocode(from_name),
        self.geocode(to_name),
      );

      match (coords.0, coords.1) {
        (Ok(from), Ok(to)) => {
          match self.driving_minutes(from, to).await {
            Ok(mins) => total_minutes += mins,
            Err(_) => failed_geocodes += 1,
          }
        }
        _ => failed_geocodes += 1,
      }
    }

    // Allow up to 180 min transit in a day (3 hrs driving between activities)
    let feasible = total_minutes <= 180;
    let warning = if !feasible {
      Some(format!(
        "Day plan requires ~{}min transit — consider removing an activity or choosing closer venues",
        total_minutes
      ))
    } else if total_minutes > 90 {
      Some(format!("Day plan has ~{}min transit — tight but doable", total_minutes))
    } else {
      None
    };

    if failed_geocodes > 0 && total_minutes == 0 {
      // All geocodes failed — assume feasible, skip validation
      return (true, 0, None);
    }

    (feasible, total_minutes, warning)
  }

  async fn route_minutes(
    &self,
    profile: &str,
    from: (f64, f64),
    to: (f64, f64),
  ) -> Result<u32> {
    let coords = format!("{},{};{},{}", from.0, from.1, to.0, to.1);
    let url    = format!("{}/{}/{}", MAPBOX_DIRECTIONS_BASE, profile, coords);

    let resp = self
      .http
      .get(&url)
      .query(&[
        ("access_token", self.access_token.as_str()),
        ("overview",     "false"),
        ("geometries",   "geojson"),
      ])
      .send()
      .await
      .context("Mapbox directions request failed")?;

    if !resp.status().is_success() {
      let status = resp.status();
      let body = resp.text().await.unwrap_or_default();
      anyhow::bail!("Mapbox directions returned {}: {}", status, body);
    }

    let data: DirectionsResponse = resp
      .json()
      .await
      .context("Failed to parse Mapbox directions response")?;

    if data.code.as_deref() != Some("Ok") && !data.routes.is_empty() == false {
      anyhow::bail!("Mapbox directions code: {:?}", data.code);
    }

    let route = data.routes.into_iter().next()
      .context("Mapbox directions returned no routes")?;

    Ok((route.duration / 60.0).ceil() as u32)
  }
}

// ─── Builder ──────────────────────────────────────────────────────────────────

pub fn build_mapbox() -> anyhow::Result<MapboxClient> {
  let token = std::env::var("MAPBOX_ACCESS_TOKEN")
    .context("MAPBOX_ACCESS_TOKEN not set in .env")?;
  Ok(MapboxClient::new(token))
}
