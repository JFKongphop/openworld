/*!
SearchAgent — real flight + hotel prices via SerpAPI, transport via Qwen.

Flight and hotel search uses SerpAPI (Google Flights / Google Hotels engines)
for live real-world prices. Ground transport (trains, buses) stays as Qwen
because Google doesn't index local rail fares reliably.

Fallback: if SerpAPI is unavailable, falls back to Qwen chain-of-thought.
*/

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{
  ActivityLog, Agent, ExecutionContext, FlightOption, HotelOption, Itinerary, SearchResults,
  SegmentKind, TransportOption,
};
use crate::qwen_client::QwenClient;
use crate::serpapi::{build_serpapi, SerpApiClient};

// ─── IATA city code lookup ─────────────────────────────────────────────────────
/// Map common city/country names to their primary IATA airport codes.
/// SerpAPI Google Flights requires IATA codes, not city names.
fn city_to_iata(city: &str) -> String {
  match city.to_lowercase().trim() {
    "tokyo" | "tyo" => "NRT".into(),
    "osaka" => "KIX".into(),
    "kyoto" => "ITM".into(),
    "bangkok" | "bkk" => "BKK".into(),
    "singapore" | "sin" => "SIN".into(),
    "hong kong" | "hkg" => "HKG".into(),
    "seoul" | "icn" => "ICN".into(),
    "busan" => "PUS".into(),
    "taipei" | "tpe" => "TPE".into(),
    "london" | "lon" => "LHR".into(),
    "paris" | "par" => "CDG".into(),
    "amsterdam" => "AMS".into(),
    "dubai" | "dxb" => "DXB".into(),
    "kuala lumpur" | "kul" => "KUL".into(),
    "jakarta" => "CGK".into(),
    "bali" | "denpasar" => "DPS".into(),
    "ho chi minh" | "saigon" | "sgn" => "SGN".into(),
    "hanoi" | "han" => "HAN".into(),
    "new york" | "nyc" => "JFK".into(),
    "los angeles" | "la" | "lax" => "LAX".into(),
    "sydney" | "syd" => "SYD".into(),
    "melbourne" => "MEL".into(),
    "narita" | "narita airport" => "NRT".into(),
    "haneda" => "HND".into(),
    "suvarnabhumi" => "BKK".into(),
    other => other.to_uppercase(),
  }
}

// ─── SearchAgent ──────────────────────────────────────────────────────────────

pub struct SearchAgent {
  itinerary: Arc<Mutex<Option<Itinerary>>>,
  pub results: Arc<Mutex<SearchResults>>,
  compute: QwenClient,
  serpapi: Option<SerpApiClient>,
}

impl SearchAgent {
  pub fn new(
    itinerary: Arc<Mutex<Option<Itinerary>>>,
    results: Arc<Mutex<SearchResults>>,
    compute: QwenClient,
  ) -> Self {
    let serpapi = build_serpapi().ok();
    Self {
      itinerary,
      results,
      compute,
      serpapi,
    }
  }
}

#[async_trait]
impl Agent for SearchAgent {
  fn name(&self) -> &str {
    "SearchAgent"
  }

  async fn run(&self, ctx: &ExecutionContext) -> Result<()> {
    let itinerary = self.itinerary.lock().await.clone();
    let itinerary = match itinerary {
      Some(i) => i,
      None => {
        ctx.log(ActivityLog::error(
          self.name(),
          "No itinerary — skipping search",
        ));
        return Ok(());
      }
    };

    // Collect owned task data per category so async blocks can capture by reference
    let flight_tasks: Vec<(String, String, String)> = {
      let mut seen = std::collections::HashSet::new();
      itinerary
        .segments
        .iter()
        .filter(|s| matches!(s.kind, SegmentKind::Flight))
        .filter_map(|s| {
          let from = s.from.clone();
          let to = s
            .to
            .clone()
            .unwrap_or_else(|| itinerary.destination.clone());
          // Deduplicate by route — same BKK→NRT doesn't need two SerpAPI calls
          let key = format!("{from}-{to}");
          if seen.insert(key) {
            Some((from, to, s.date.clone()))
          } else {
            None
          }
        })
        .collect()
    };

    let hotel_tasks: Vec<String> = {
      let mut seen = std::collections::HashSet::new();
      itinerary
        .segments
        .iter()
        .filter(|s| matches!(s.kind, SegmentKind::Hotel))
        .filter_map(|s| {
          let city = s.from.clone();
          // Deduplicate by city — no need to call SerpAPI 5× for 5 hotel nights in Tokyo
          if seen.insert(city.clone()) {
            Some(city)
          } else {
            None
          }
        })
        .collect()
    };

    let transport_tasks: Vec<(String, String, String)> = itinerary
      .segments
      .iter()
      .filter(|s| matches!(s.kind, SegmentKind::Train | SegmentKind::Bus))
      .map(|s| {
        (
          s.from.clone(),
          s.to
            .clone()
            .unwrap_or_else(|| itinerary.destination.clone()),
          format!("{:?}", s.kind).to_lowercase(),
        )
      })
      .collect();

    ctx.log(ActivityLog::action(
      self.name(),
      &format!(
        "Searching {} flight(s), {} hotel(s), {} route(s) — flights/hotels via SerpAPI, transport via Qwen...",
        flight_tasks.len(),
        hotel_tasks.len(),
        transport_tasks.len(),
      ),
    ));

    if self.serpapi.is_some() {
      ctx.log(ActivityLog::info(
        self.name(),
        "✓ SerpAPI connected — using real Google Flights + Hotels prices",
      ));
    } else {
      ctx.log(ActivityLog::warn(
        self.name(),
        "SerpAPI unavailable — falling back to Qwen estimates for flights/hotels",
      ));
    }

    // ── Parallel search — all three categories run concurrently ─────────────
    let (flights, hotels, transport) = tokio::join!(
      async {
        let mut out = Vec::new();
        for (from, to, date) in &flight_tasks {
          ctx.log(ActivityLog::action(
            self.name(),
            &format!("Pricing flights: {from} → {to}"),
          ));
          let f = self.search_flights(from, to, date, ctx).await;
          ctx.log(ActivityLog::info(
            self.name(),
            &format!("Found {} flight option(s)", f.len()),
          ));
          out.extend(f);
        }
        out
      },
      async {
        let mut out = Vec::new();
        for city in &hotel_tasks {
          ctx.log(ActivityLog::action(
            self.name(),
            &format!("Pricing hotels in {city}..."),
          ));
          let h = self.search_hotels(city, ctx).await;
          ctx.log(ActivityLog::info(
            self.name(),
            &format!("Found {} hotel option(s)", h.len()),
          ));
          out.extend(h);
        }
        out
      },
      async {
        let mut out = Vec::new();
        for (from, to, kind) in &transport_tasks {
          ctx.log(ActivityLog::action(
            self.name(),
            &format!("Pricing {kind} routes: {from} → {to}"),
          ));
          let t = self.search_transport(from, to, kind, ctx).await;
          ctx.log(ActivityLog::info(
            self.name(),
            &format!("Found {} route option(s)", t.len()),
          ));
          out.extend(t);
        }
        out
      },
    );

    let mut all_results = SearchResults {
      flights,
      hotels,
      transport,
    };

    // ── PriceReflectionTool — catch hallucinated prices ──────────────────────
    if !all_results.flights.is_empty()
      || !all_results.hotels.is_empty()
      || !all_results.transport.is_empty()
    {
      ctx.log(ActivityLog::action(
        self.name(),
        "PriceReflectionTool — validating prices against real-world ranges...",
      ));
      self
        .reflect_and_filter(&mut all_results, ctx, &itinerary)
        .await;
    }

    ctx.log(ActivityLog::success(
      self.name(),
      &format!(
        "Research complete — {} flights, {} hotels, {} transport options",
        all_results.flights.len(),
        all_results.hotels.len(),
        all_results.transport.len()
      ),
    ));

    *self.results.lock().await = all_results;
    Ok(())
  }
}

// ─── PriceReflectionTool ──────────────────────────────────────────────────────

impl SearchAgent {
  /// Validate all prices against real-world ranges via Qwen AI.
  /// Removes items the model flags as clearly unrealistic (too cheap or too expensive).
  async fn reflect_and_filter(
    &self,
    results: &mut SearchResults,
    ctx: &ExecutionContext,
    itinerary: &Itinerary,
  ) {
    let origin = &ctx.policy.trip.origin;
    let dest = &itinerary.destination;

    // Build compact price summary
    let mut summary = String::new();
    if !results.flights.is_empty() {
      summary.push_str(&format!("FLIGHTS ({origin} → {dest}):\n"));
      for (i, f) in results.flights.iter().enumerate() {
        summary.push_str(&format!(
          "  [{i}] {}: ${:.0}/{}-stop\n",
          f.airline, f.price_usd, f.stops
        ));
      }
    }
    if !results.hotels.is_empty() {
      summary.push_str(&format!("HOTELS in {dest}:\n"));
      for (i, h) in results.hotels.iter().enumerate() {
        summary.push_str(&format!(
          "  [{i}] {}: ${:.0}/night, rating {:.1}\n",
          h.name, h.price_per_night_usd, h.rating
        ));
      }
    }
    if !results.transport.is_empty() {
      summary.push_str("TRANSPORT (within destination):\n");
      for (i, t) in results.transport.iter().enumerate() {
        summary.push_str(&format!(
          "  [{i}] {} ({}): ${:.0}\n",
          t.provider, t.route, t.price_usd
        ));
      }
    }

    let prompt = format!(
      r#"Review these travel prices for a trip from {origin} to {dest}.

{summary}
Flag items with CLEARLY unrealistic prices:
- International economy flights one-way: suspicious if <$150 or >$1,200
- Hotels per night in Japan: suspicious if <$20 or >$400
- Shinkansen/express train within Japan: suspicious if <$30 or >$300
- Highway buses within Japan: suspicious if <$8 or >$120

Output ONLY valid JSON — no explanation, no markdown:
{{"remove_flights":[],"remove_hotels":[],"remove_transport":[]}}"#
    );

    let raw = match self.compute.infer_with_system(
      "You are a travel price validator. Identify only clearly unrealistic prices. Return only a JSON object with index arrays.",
      &prompt,
      Some(200),
    ).await {
      Ok(r) => r,
      Err(_) => return,
    };

    ctx.log(ActivityLog::info(
      self.name(),
      &format!(
        "PriceReflection: {}",
        raw.trim().chars().take(120).collect::<String>()
      ),
    ));

    // Extract JSON object from response
    let json_str = match (raw.find('{'), raw.rfind('}')) {
      (Some(s), Some(e)) if e >= s => &raw[s..=e],
      _ => return,
    };
    let v: serde_json::Value = match serde_json::from_str(json_str) {
      Ok(v) => v,
      Err(_) => return,
    };

    // Parse indices descending so removal doesn't shift remaining indices
    let parse_indices = |key: &str| -> Vec<usize> {
      let mut idxs: Vec<usize> = v
        .get(key)
        .and_then(|a| a.as_array())
        .map(|arr| {
          arr
            .iter()
            .filter_map(|x| x.as_u64().map(|n| n as usize))
            .collect()
        })
        .unwrap_or_default();
      idxs.sort_unstable_by(|a, b| b.cmp(a));
      idxs
    };

    for idx in parse_indices("remove_flights") {
      if idx < results.flights.len() {
        let r = results.flights.remove(idx);
        ctx.log(ActivityLog::warn(
          self.name(),
          &format!(
            "Removed unrealistic flight: {} ${:.0}",
            r.airline, r.price_usd
          ),
        ));
      }
    }
    for idx in parse_indices("remove_hotels") {
      if idx < results.hotels.len() {
        let r = results.hotels.remove(idx);
        ctx.log(ActivityLog::warn(
          self.name(),
          &format!(
            "Removed unrealistic hotel: {} ${:.0}/night",
            r.name, r.price_per_night_usd
          ),
        ));
      }
    }
    for idx in parse_indices("remove_transport") {
      if idx < results.transport.len() {
        let r = results.transport.remove(idx);
        ctx.log(ActivityLog::warn(
          self.name(),
          &format!(
            "Removed unrealistic route: {} ${:.0}",
            r.provider, r.price_usd
          ),
        ));
      }
    }
  }
}

// ─── SerpAPI + Qwen search methods ───────────────────────────────────────────

impl SearchAgent {
  async fn search_flights(
    &self,
    from: &str,
    to: &str,
    date: &str,
    ctx: &ExecutionContext,
  ) -> Vec<FlightOption> {
    // Resolve city names → IATA codes for SerpAPI compatibility
    let from_iata = city_to_iata(from);
    let to_iata = city_to_iata(to);

    // ── Try SerpAPI first (real Google Flights prices) ──────────────────────
    if let Some(ref api) = self.serpapi {
      match api.search_flights(&from_iata, &to_iata, date).await {
        Ok(results) if !results.is_empty() => {
          ctx.log(ActivityLog::success(
            self.name(),
            &format!(
              "SerpAPI: {} real flight prices for {} → {}",
              results.len(),
              from_iata,
              to_iata
            ),
          ));
          return results;
        }
        Ok(_) => {
          ctx.log(ActivityLog::warn(
            self.name(),
            &format!(
              "SerpAPI returned no flights for {} → {} — falling back to Qwen",
              from_iata, to_iata
            ),
          ));
        }
        Err(e) => {
          ctx.log(ActivityLog::warn(
            self.name(),
            &format!("SerpAPI flights error ({}), falling back to Qwen", e),
          ));
        }
      }
    }

    // ── Qwen fallback ───────────────────────────────────────────────────────
    let budget = ctx.policy.trip.budget_max as u32;
    let max_flight = (ctx.policy.trip.budget_max * 0.40) as u32;
    let stop_pref = if ctx.policy.flight.max_stops == 0 {
      "non-stop only"
    } else {
      "up to 1 stop"
    };
    let airlines = ctx.policy.flight.preferred_airlines.join(", ");
    let airlines = if airlines.is_empty() {
      "any airline".to_string()
    } else {
      airlines
    };

    let think = format!(
      r#"A traveler needs economy class flights from {from} to {to} departing around {date}.
Total trip budget: ${budget} USD | Max per flight: ${max_flight} USD | Stops: {stop_pref} | Airlines: {airlines}

Reason step by step:
1. Which full-service and low-cost airlines operate the {from} → {to} route?
2. What are typical economy fares in USD for this route in this season?
3. How do prices vary across budget carriers vs full-service?
Write your analysis now."#
    );
    let answer = format!(
      r#"Based on your analysis above, output ONLY a valid JSON array of exactly 5 flight options. No markdown, no explanation — just the JSON array.
Schema: [{{"airline":"ANA","price_usd":420,"stops":0,"duration":"6h30m","departure":"09:00"}}]
All prices must be positive USD values."#
    );

    let raw = match self
      .compute
      .think_then_answer(
        "You are a senior flight pricing expert with deep knowledge of Asian airline routes.",
        &think,
        &answer,
        Some(1024),
      )
      .await
    {
      Ok(r) => r,
      Err(_) => return demo_flights(from, to),
    };
    parse_flights(&raw, from, to).unwrap_or_else(|| demo_flights(from, to))
  }

  async fn search_hotels(&self, city: &str, ctx: &ExecutionContext) -> Vec<HotelOption> {
    let max_night = ctx.policy.hotel.max_price_per_night;
    let min_rating = ctx.policy.hotel.min_rating;
    let dep_date = &ctx.policy.trip.departure_date;
    let ret_date = &ctx.policy.trip.return_date;

    // ── Try SerpAPI first (real Google Hotels prices) ───────────────────────
    if let Some(ref api) = self.serpapi {
      match api
        .search_hotels(city, dep_date, ret_date, min_rating, max_night)
        .await
      {
        Ok(results) if !results.is_empty() => {
          ctx.log(ActivityLog::success(
            self.name(),
            &format!("SerpAPI: {} real hotel prices in {}", results.len(), city),
          ));
          return results;
        }
        Ok(_) => {
          ctx.log(ActivityLog::warn(
            self.name(),
            &format!(
              "SerpAPI returned no hotels in {} — falling back to Qwen",
              city
            ),
          ));
        }
        Err(e) => {
          ctx.log(ActivityLog::warn(
            self.name(),
            &format!("SerpAPI hotels error ({}), falling back to Qwen", e),
          ));
        }
      }
    }

    // ── Qwen fallback ───────────────────────────────────────────────────────
    let max_night_u = max_night as u32;
    let location = if ctx.policy.hotel.near_station {
      "near a train or metro station"
    } else {
      "city centre"
    };

    let think = format!(
      r#"A traveler needs a hotel in {city}. Max ${max_night_u} USD/night, min rating {min_rating}/5, location: {location}.

Reason step by step:
1. Which neighbourhoods in {city} are best for a mid-range traveler near transport?
2. Which recognizable hotel brands have properties in {city}?
3. What do rooms in these hotels realistically cost per night in USD?
Write your analysis now."#
    );
    let answer = format!(
      r#"Based on your analysis above, output ONLY a valid JSON array of exactly 5 hotel options. No markdown, no explanation — just the JSON array.
Schema: [{{"name":"Dormy Inn Premium Shinjuku","price_per_night":88,"rating":4.3,"location":"Shinjuku, near JR station"}}]
All options must be under ${max_night_u}/night, use real hotel names, positive prices only."#
    );

    let raw = match self
      .compute
      .think_then_answer(
        "You are a senior hotel pricing expert specialising in Japanese accommodation markets.",
        &think,
        &answer,
        Some(1024),
      )
      .await
    {
      Ok(r) => r,
      Err(_) => return demo_hotels(city),
    };
    parse_hotels(&raw, city).unwrap_or_else(|| demo_hotels(city))
  }

  async fn search_transport(
    &self,
    from: &str,
    to: &str,
    kind: &str,
    ctx: &ExecutionContext,
  ) -> Vec<TransportOption> {
    let dest = &ctx.policy.trip.destination;

    // Turn 1 — free reasoning about transport options
    let think = format!(
      r#"A traveler needs {kind} transport from {from} to {to} in {dest}.

Reason step by step:
1. What train or bus operators connect {from} and {to}? (JR Shinkansen, private railways, highway buses, etc.)
2. What are the actual fares in USD for each operator on this route?
3. Are there rail passes (JR Pass, Seishun 18, IC cards, Klook deals) that are cheaper than buying individual tickets?
4. How long does each option take?
Write your analysis now."#
    );

    // Turn 2 — structured output from reasoning
    let answer = format!(
      r#"Based on your analysis above, output ONLY a valid JSON array of exactly 5 transport options. No markdown, no explanation — just the JSON array.
Schema: [{{"provider":"JR Shinkansen Nozomi","route":"{from} → {to}","kind":"train","price_usd":55,"duration":"2h30m"}}]
Use real operator names. All prices in USD. Prices must be realistic and positive."#
    );

    let raw = match self.compute.think_then_answer(
      "You are a senior Japan transport pricing expert with deep knowledge of JR, private railways, and highway buses.",
      &think,
      &answer,
      Some(1024),
    ).await {
      Ok(r) => r,
      Err(_) => return demo_transport(from, to),
    };

    parse_transport(&raw, from, to).unwrap_or_else(|| demo_transport(from, to))
  }
}

// ─── JSON parsers ─────────────────────────────────────────────────────────────

fn parse_flights(raw: &str, from: &str, to: &str) -> Option<Vec<FlightOption>> {
  let start = raw.find('[')?;
  let end = raw.rfind(']')?;
  let arr: Vec<serde_json::Value> = serde_json::from_str(&raw[start..=end]).ok()?;
  let opts: Vec<FlightOption> = arr
    .iter()
    .filter_map(|v| {
      let airline = v["airline"].as_str()?.to_string();
      let price = v["price_usd"].as_f64().or_else(|| v["price"].as_f64())?;
      let stops = v["stops"].as_u64().unwrap_or(0) as u32;
      let duration = v["duration"].as_str().unwrap_or("~6h").to_string();
      let departure = v["departure"].as_str().unwrap_or("09:00").to_string();
      if price <= 0.0 {
        return None;
      }
      Some(FlightOption {
        airline,
        route: format!("{from} → {to}"),
        departure,
        arrival: "Check provider".to_string(),
        stops,
        duration,
        price_usd: price,
        booking_url: None,
        seats_remaining: None,
      })
    })
    .collect();
  if opts.is_empty() {
    None
  } else {
    Some(opts)
  }
}

fn parse_hotels(raw: &str, city: &str) -> Option<Vec<HotelOption>> {
  let start = raw.find('[')?;
  let end = raw.rfind(']')?;
  let arr: Vec<serde_json::Value> = serde_json::from_str(&raw[start..=end]).ok()?;
  let opts: Vec<HotelOption> = arr
    .iter()
    .filter_map(|v| {
      let name = v["name"].as_str()?.to_string();
      let price = v["price_per_night"]
        .as_f64()
        .or_else(|| v["price_usd"].as_f64())
        .or_else(|| v["price"].as_f64())?;
      let rating = v["rating"].as_f64().unwrap_or(4.0);
      let location = v["location"].as_str().unwrap_or(city).to_string();
      if price <= 0.0 {
        return None;
      }
      Some(HotelOption {
        name,
        location,
        price_per_night_usd: price,
        rating,
        near_station: true,
        booking_url: None,
      })
    })
    .collect();
  if opts.is_empty() {
    None
  } else {
    Some(opts)
  }
}

fn parse_transport(raw: &str, from: &str, to: &str) -> Option<Vec<TransportOption>> {
  let start = raw.find('[')?;
  let end = raw.rfind(']')?;
  let arr: Vec<serde_json::Value> = serde_json::from_str(&raw[start..=end]).ok()?;
  let opts: Vec<TransportOption> = arr
    .iter()
    .filter_map(|v| {
      let provider = v["provider"].as_str()?.to_string();
      let route = v["route"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{from} → {to}"));
      let kind = v["kind"].as_str().unwrap_or("train").to_string();
      let price = v["price_usd"].as_f64().or_else(|| v["price"].as_f64())?;
      if price <= 0.0 {
        return None;
      }
      Some(TransportOption {
        provider,
        route,
        kind,
        departure: "Check provider".to_string(),
        price_usd: price,
        booking_url: None,
      })
    })
    .collect();
  if opts.is_empty() {
    None
  } else {
    Some(opts)
  }
}

// ─── Demo fallbacks (used only when both SerpAPI and Qwen are unavailable) ────

fn demo_flights(from: &str, to: &str) -> Vec<FlightOption> {
  let route = format!("{from} → {to}");
  vec![
    FlightOption {
      airline: "ANA".into(),
      route: route.clone(),
      departure: "09:00".into(),
      arrival: "15:30".into(),
      stops: 0,
      duration: "6h30m".into(),
      price_usd: 420.0,
      booking_url: None,
      seats_remaining: None,
    },
    FlightOption {
      airline: "JAL".into(),
      route: route.clone(),
      departure: "11:00".into(),
      arrival: "17:30".into(),
      stops: 0,
      duration: "6h30m".into(),
      price_usd: 395.0,
      booking_url: None,
      seats_remaining: None,
    },
    FlightOption {
      airline: "Thai Airways".into(),
      route: route.clone(),
      departure: "08:30".into(),
      arrival: "17:00".into(),
      stops: 1,
      duration: "8h30m".into(),
      price_usd: 340.0,
      booking_url: None,
      seats_remaining: None,
    },
    FlightOption {
      airline: "Singapore Airlines".into(),
      route: route.clone(),
      departure: "14:00".into(),
      arrival: "23:00".into(),
      stops: 1,
      duration: "9h00m".into(),
      price_usd: 380.0,
      booking_url: None,
      seats_remaining: None,
    },
    FlightOption {
      airline: "Scoot".into(),
      route: route.clone(),
      departure: "07:00".into(),
      arrival: "15:30".into(),
      stops: 1,
      duration: "8h30m".into(),
      price_usd: 280.0,
      booking_url: None,
      seats_remaining: None,
    },
  ]
}

fn demo_hotels(city: &str) -> Vec<HotelOption> {
  vec![
    HotelOption {
      name: "Dormy Inn Premium".into(),
      location: format!("{city}, near station"),
      price_per_night_usd: 88.0,
      rating: 4.3,
      near_station: true,
      booking_url: None,
    },
    HotelOption {
      name: "APA Hotel".into(),
      location: format!("{city}, city centre"),
      price_per_night_usd: 72.0,
      rating: 3.9,
      near_station: true,
      booking_url: None,
    },
    HotelOption {
      name: "Sotetsu Fresa Inn".into(),
      location: format!("{city}, near station"),
      price_per_night_usd: 78.0,
      rating: 4.1,
      near_station: true,
      booking_url: None,
    },
    HotelOption {
      name: "Comfort Hotel".into(),
      location: format!("{city}, central"),
      price_per_night_usd: 65.0,
      rating: 3.8,
      near_station: true,
      booking_url: None,
    },
    HotelOption {
      name: "Cross Hotel".into(),
      location: format!("{city}, downtown"),
      price_per_night_usd: 95.0,
      rating: 4.4,
      near_station: false,
      booking_url: None,
    },
  ]
}

fn demo_transport(from: &str, to: &str) -> Vec<TransportOption> {
  let route = format!("{from} → {to}");
  vec![
    TransportOption {
      provider: "JR Shinkansen Nozomi".into(),
      route: route.clone(),
      kind: "train".into(),
      departure: "09:30".into(),
      price_usd: 130.0,
      booking_url: None,
    },
    TransportOption {
      provider: "JR Shinkansen Hikari".into(),
      route: route.clone(),
      kind: "train".into(),
      departure: "10:00".into(),
      price_usd: 110.0,
      booking_url: None,
    },
    TransportOption {
      provider: "Klook Rail Pass".into(),
      route: route.clone(),
      kind: "train".into(),
      departure: "Flexible".into(),
      price_usd: 85.0,
      booking_url: None,
    },
    TransportOption {
      provider: "Willer Highway Bus".into(),
      route: route.clone(),
      kind: "bus".into(),
      departure: "07:00".into(),
      price_usd: 35.0,
      booking_url: None,
    },
    TransportOption {
      provider: "Local Express Train".into(),
      route: route.clone(),
      kind: "train".into(),
      departure: "Hourly".into(),
      price_usd: 25.0,
      booking_url: None,
    },
  ]
}
