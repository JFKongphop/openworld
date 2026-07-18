/*!
OpenWorld MCP Server — JSON-RPC 2.0 over stdio (MCP spec 2024-11-05).

8 tools + 3 resources expose all OpenWorld agents to any MCP client
(VS Code Copilot, Cursor, Claude Desktop, Continue, etc.).

Tools:
  openworld_plan_trip        — parse travel.md YAML, create session, run PlannerAgent
  openworld_search_flights   — search real flights via SerpAPI (or Qwen fallback)
  openworld_search_hotels    — search real hotels via SerpAPI (or Qwen fallback)
  openworld_get_weather      — Open-Meteo 7-day forecast for a destination city
  openworld_run_pipeline     — launch full 7-agent orchestration pipeline (async)
  openworld_get_session      — poll session state, logs, bookings, itinerary
  openworld_approve_session  — approve or reject a paused AwaitingApproval session
  openworld_get_report       — retrieve the generated Markdown travel report

Resources:
  openworld://sessions        — JSON list of all active/completed sessions
  openworld://session/{id}    — full session state JSON
  openworld://report/{id}     — generated Markdown trip report text

Usage:
  cargo build --bin mcp
  ./target/debug/mcp          (stdin/stdout — used by MCP clients)
*/

use anyhow::Result;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

use openworld::{
  agents::{
    planner::PlannerAgent,
    ExecutionContext,
  },
  build_qwen_client,
  create_session, new_registry, run_session,
  SessionRegistry, SessionState,
  weather::build_weather,
  serpapi::build_serpapi,
  report::generate_travel_report,
};

// ─── Shared MCP state ─────────────────────────────────────────────────────────

#[derive(Clone)]
struct McpState {
  registry: SessionRegistry,
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
  dotenv::dotenv().ok();

  let state = McpState { registry: new_registry() };

  let stdin  = tokio::io::stdin();
  let mut stdout = tokio::io::stdout();
  let mut reader = BufReader::new(stdin);
  let mut line   = String::new();

  loop {
    line.clear();
    let n = reader.read_line(&mut line).await?;
    if n == 0 { break; }

    let trimmed = line.trim();
    if trimmed.is_empty() { continue; }

    let request: Value = match serde_json::from_str(trimmed) {
      Ok(v) => v,
      Err(_) => continue,
    };

    // Notifications (no "id") get no response
    if request.get("id").is_none() {
      continue;
    }

    let response = handle_request(request, &state).await;
    let mut out = serde_json::to_string(&response)?;
    out.push('\n');
    stdout.write_all(out.as_bytes()).await?;
    stdout.flush().await?;
  }

  Ok(())
}

// ─── Dispatcher ───────────────────────────────────────────────────────────────

async fn handle_request(req: Value, state: &McpState) -> Value {
  let id     = req["id"].clone();
  let method = req["method"].as_str().unwrap_or("");
  let params = req.get("params").cloned().unwrap_or(json!({}));

  match method {
    "initialize"      => rpc_ok(id, json!({
      "protocolVersion": "2024-11-05",
      "serverInfo": { "name": "openworld", "version": "0.1.0" },
      "capabilities": {
        "tools": {},
        "resources": { "subscribe": false, "listChanged": false }
      }
    })),
    "tools/list"      => rpc_ok(id, json!({ "tools": tools_list() })),
    "tools/call"      => tools_call(id, params, state).await,
    "resources/list"  => rpc_ok(id, json!({ "resources": resources_list() })),
    "resources/read"  => resources_read(id, params, state).await,
    "ping"            => rpc_ok(id, json!({})),
    _                 => rpc_error(id, -32601, "Method not found"),
  }
}

// ─── Tool schemas ─────────────────────────────────────────────────────────────

fn tools_list() -> Value {
  json!([
    {
      "name": "openworld_plan_trip",
      "description": "Parse a travel.md YAML string, create an OpenWorld session, run PlannerAgent, and return a structured itinerary with day-by-day activities and budget allocation.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "travel_md": {
            "type": "string",
            "description": "Full travel.md YAML content (trip, flight, hotel, transport, automation, vault sections)"
          }
        },
        "required": ["travel_md"]
      }
    },
    {
      "name": "openworld_search_flights",
      "description": "Search for real flight options on a given route using SerpAPI Google Flights. Falls back to Qwen AI estimates if SerpAPI is unavailable.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "origin":      { "type": "string", "description": "IATA airport code, e.g. BKK" },
          "destination": { "type": "string", "description": "IATA airport code, e.g. NRT" },
          "date":        { "type": "string", "description": "Departure date YYYY-MM-DD" }
        },
        "required": ["origin", "destination", "date"]
      }
    },
    {
      "name": "openworld_search_hotels",
      "description": "Search for real hotel options in a city using SerpAPI Google Hotels. Falls back to Qwen AI estimates if SerpAPI is unavailable.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "city":           { "type": "string", "description": "City name, e.g. Tokyo" },
          "checkin":        { "type": "string", "description": "Check-in date YYYY-MM-DD" },
          "checkout":       { "type": "string", "description": "Check-out date YYYY-MM-DD" },
          "min_rating":     { "type": "number", "description": "Minimum star rating (0-5). Default 3.5" },
          "max_per_night":  { "type": "number", "description": "Max price per night in USD. Default 200" }
        },
        "required": ["city", "checkin", "checkout"]
      }
    },
    {
      "name": "openworld_get_weather",
      "description": "Get a 7-day weather forecast for a destination city via Open-Meteo (free, no API key required). Returns temperature, rain, and travel advisories per day.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "city":       { "type": "string", "description": "Destination city name, e.g. Tokyo" },
          "start_date": { "type": "string", "description": "Start date YYYY-MM-DD" },
          "end_date":   { "type": "string", "description": "End date YYYY-MM-DD" }
        },
        "required": ["city", "start_date", "end_date"]
      }
    },
    {
      "name": "openworld_run_pipeline",
      "description": "Launch the full 7-agent OpenWorld pipeline: PlannerAgent → SearchAgent → VaultAgent → ReservationAgent → RecoveryAgent → ArtifactAgent. Returns a session_id immediately; poll openworld_get_session for progress.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "travel_md": {
            "type": "string",
            "description": "Full travel.md YAML content"
          }
        },
        "required": ["travel_md"]
      }
    },
    {
      "name": "openworld_get_session",
      "description": "Poll an OpenWorld session for its current state, recent activity logs, booked segments, and itinerary summary.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "session_id": { "type": "string", "description": "UUID returned by plan_trip or run_pipeline" }
        },
        "required": ["session_id"]
      }
    },
    {
      "name": "openworld_approve_session",
      "description": "Approve or reject a session that is paused in AwaitingApproval state (triggered when >80% of budget is committed). Rejection terminates the pipeline.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "session_id": { "type": "string", "description": "UUID of the paused session" },
          "approved":   { "type": "boolean", "description": "true to continue, false to cancel" }
        },
        "required": ["session_id", "approved"]
      }
    },
    {
      "name": "openworld_check_visa",
      "description": "Check visa requirements for a traveler by passport nationality and destination country, using Qwen (Alibaba Cloud Model Studio) reasoning. Returns visa type, max stay, processing time, and key conditions.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "nationality": {
            "type": "string",
            "description": "Passport nationality — country name or 3-letter code, e.g. THA or Thailand"
          },
          "destination": {
            "type": "string",
            "description": "Destination country — country name or 3-letter code, e.g. JPN or Japan"
          },
          "purpose": {
            "type": "string",
            "description": "Travel purpose: tourism, business, or transit. Default: tourism"
          }
        },
        "required": ["nationality", "destination"]
      }
    },
    {
      "name": "openworld_get_report",
      "description": "Retrieve the full Markdown travel report for a completed OpenWorld session, including itinerary, bookings, budget summary, and HMAC-signed execution proof.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "session_id": { "type": "string", "description": "UUID of a completed session" }
        },
        "required": ["session_id"]
      }
    }
  ])
}

// ─── Resource schemas ─────────────────────────────────────────────────────────

fn resources_list() -> Value {
  json!([
    {
      "uri":         "openworld://sessions",
      "name":        "Active Sessions",
      "description": "JSON list of all active and completed OpenWorld sessions with state and created_at",
      "mimeType":    "application/json"
    },
    {
      "uri":         "openworld://session/{id}",
      "name":        "Session State",
      "description": "Full state of a specific session — state, policy, itinerary, bookings, recent logs",
      "mimeType":    "application/json"
    },
    {
      "uri":         "openworld://report/{id}",
      "name":        "Trip Report",
      "description": "Generated Markdown trip report for a completed session",
      "mimeType":    "text/markdown"
    }
  ])
}

// ─── Tool dispatcher ──────────────────────────────────────────────────────────

async fn tools_call(id: Value, params: Value, state: &McpState) -> Value {
  let name = match params["name"].as_str() {
    Some(n) => n,
    None => return rpc_error(id, -32602, "Missing tool name"),
  };
  let args = params.get("arguments").cloned().unwrap_or(json!({}));

  let result = match name {
    "openworld_plan_trip"       => tool_plan_trip(&args, state).await,
    "openworld_search_flights"  => tool_search_flights(&args).await,
    "openworld_search_hotels"   => tool_search_hotels(&args).await,
    "openworld_get_weather"     => tool_get_weather(&args).await,
    "openworld_check_visa"      => tool_check_visa(&args).await,
    "openworld_run_pipeline"    => tool_run_pipeline(&args, state).await,
    "openworld_get_session"     => tool_get_session(&args, state).await,
    "openworld_approve_session" => tool_approve_session(&args, state).await,
    "openworld_get_report"      => tool_get_report(&args, state).await,
    _ => return rpc_error(id, -32601, &format!("Unknown tool: {name}")),
  };

  rpc_ok(id, json!({ "content": [{ "type": "text", "text": result }] }))
}

// ─── Tool: plan_trip ──────────────────────────────────────────────────────────

async fn tool_plan_trip(args: &Value, state: &McpState) -> String {
  let travel_md = match args["travel_md"].as_str() {
    Some(s) => s,
    None => return err_text("Missing travel_md"),
  };

  let session = match create_session(travel_md) {
    Ok(s) => s,
    Err(e) => return err_text(&format!("Invalid travel.md: {e}")),
  };

  let session_id = session.session_id;
  state.registry.write().await.insert(session_id, session.clone());

  let compute = match build_qwen_client() {
    Ok(c) => c,
    Err(e) => return err_text(&format!("Qwen unavailable: {e}")),
  };

  let ctx = ExecutionContext { session_id, policy: session.policy.clone(), log_tx: session.log_tx.clone() };

  let planner = PlannerAgent::new(compute, session.itinerary.clone());
  if let Err(e) = openworld::agents::Agent::run(&planner, &ctx).await {
    return err_text(&format!("PlannerAgent failed: {e}"));
  }

  let itinerary = session.itinerary.lock().await.clone();
  match itinerary {
    Some(i) => serde_json::to_string_pretty(&json!({
      "session_id": session_id.to_string(),
      "status": "planned",
      "itinerary": i
    })).unwrap_or_default(),
    None => err_text("PlannerAgent returned no itinerary"),
  }
}

// ─── Tool: search_flights ─────────────────────────────────────────────────────

async fn tool_search_flights(args: &Value) -> String {
  let origin = args["origin"].as_str().unwrap_or("");
  let dest   = args["destination"].as_str().unwrap_or("");
  let date   = args["date"].as_str().unwrap_or("");

  if origin.is_empty() || dest.is_empty() || date.is_empty() {
    return err_text("origin, destination, date are all required");
  }

  if let Some(api) = build_serpapi().ok() {
    match api.search_flights(origin, dest, date).await {
      Ok(flights) if !flights.is_empty() => {
        return serde_json::to_string_pretty(&json!({
          "source": "SerpAPI (Google Flights)",
          "route": format!("{origin} → {dest}"),
          "date": date,
          "flights": flights
        })).unwrap_or_default();
      }
      _ => {}
    }
  }

  err_text("SerpAPI unavailable and no Qwen fallback in standalone tool — run openworld_plan_trip for full search")
}

// ─── Tool: search_hotels ──────────────────────────────────────────────────────

async fn tool_search_hotels(args: &Value) -> String {
  let city         = args["city"].as_str().unwrap_or("");
  let checkin      = args["checkin"].as_str().unwrap_or("");
  let checkout     = args["checkout"].as_str().unwrap_or("");
  let min_rating   = args["min_rating"].as_f64().unwrap_or(3.5);
  let max_per_night = args["max_per_night"].as_f64().unwrap_or(200.0);

  if city.is_empty() || checkin.is_empty() || checkout.is_empty() {
    return err_text("city, checkin, checkout are all required");
  }

  if let Some(api) = build_serpapi().ok() {
    match api.search_hotels(city, checkin, checkout, min_rating, max_per_night).await {
      Ok(hotels) if !hotels.is_empty() => {
        return serde_json::to_string_pretty(&json!({
          "source": "SerpAPI (Google Hotels)",
          "city": city,
          "checkin": checkin,
          "checkout": checkout,
          "hotels": hotels
        })).unwrap_or_default();
      }
      _ => {}
    }
  }

  err_text("SerpAPI unavailable — run openworld_plan_trip for full search with Qwen fallback")
}

// ─── Tool: get_weather ────────────────────────────────────────────────────────

async fn tool_get_weather(args: &Value) -> String {
  let city       = args["city"].as_str().unwrap_or("");
  let start_date = args["start_date"].as_str().unwrap_or("");
  let end_date   = args["end_date"].as_str().unwrap_or("");

  if city.is_empty() || start_date.is_empty() || end_date.is_empty() {
    return err_text("city, start_date, end_date are all required");
  }

  let client = build_weather();
  match client.forecast_for_city(city, start_date, end_date).await {
    Ok(forecast) => serde_json::to_string_pretty(&json!({
      "city": city,
      "period": format!("{start_date} to {end_date}"),
      "forecast": forecast
    })).unwrap_or_default(),
    Err(e) => err_text(&format!("Weather forecast failed: {e}")),
  }
}

// ─── Tool: check_visa ────────────────────────────────────────────────────────

async fn tool_check_visa(args: &Value) -> String {
  let nationality = args["nationality"].as_str().unwrap_or("");
  let destination = args["destination"].as_str().unwrap_or("");
  let purpose     = args["purpose"].as_str().unwrap_or("tourism");

  if nationality.is_empty() || destination.is_empty() {
    return err_text("nationality and destination are required");
  }

  let compute = match build_qwen_client() {
    Ok(c) => c,
    Err(e) => return err_text(&format!("Qwen unavailable: {e}")),
  };

  let prompt = format!(
    r#"What are the visa requirements for a {nationality} passport holder entering {destination} for {purpose}?

Reply with ONLY a valid JSON object — no markdown, no prose:
{{
  "visa_required": true or false,
  "visa_type": "tourist visa" or null if visa-free,
  "entry_type": "visa_free" | "visa_on_arrival" | "e_visa" | "embassy_visa",
  "max_stay_days": number,
  "conditions": "key conditions such as passport validity requirement",
  "processing_time": "e.g. 3-5 business days" or null if visa-free,
  "approx_fee_usd": number or null if visa-free,
  "notes": "any important warnings or special cases",
  "disclaimer": "Verify with the official embassy before travel — this is AI-generated guidance"
}}"#
  );

  let raw = match compute.infer_with_system(
    "You are an expert immigration consultant with up-to-date knowledge of international visa policies. \
     Return only structured JSON. Be concise and accurate.",
    &prompt,
    Some(512),
  ).await {
    Ok(r) => r,
    Err(e) => return err_text(&format!("Qwen visa check failed: {e}")),
  };

  // Extract JSON object from response
  let json_str = match (raw.find('{'), raw.rfind('}')) {
    (Some(s), Some(e)) if e >= s => &raw[s..=e],
    _ => return err_text("Qwen returned a non-JSON response"),
  };

  // Re-serialize pretty with added provenance fields
  match serde_json::from_str::<Value>(json_str) {
    Ok(mut v) => {
      v["nationality"] = json!(nationality);
      v["destination"] = json!(destination);
      v["purpose"]     = json!(purpose);
      v["powered_by"]  = json!("Qwen (Alibaba Cloud Model Studio)");
      serde_json::to_string_pretty(&v).unwrap_or(raw)
    }
    Err(_) => raw,
  }
}

// ─── Tool: run_pipeline ───────────────────────────────────────────────────────

async fn tool_run_pipeline(args: &Value, state: &McpState) -> String {
  let travel_md = match args["travel_md"].as_str() {
    Some(s) => s,
    None => return err_text("Missing travel_md"),
  };

  let session = match create_session(travel_md) {
    Ok(s) => s,
    Err(e) => return err_text(&format!("Invalid travel.md: {e}")),
  };

  let session_id = session.session_id;
  state.registry.write().await.insert(session_id, session.clone());

  // Spawn full pipeline — runs in background, caller polls with get_session
  run_session(session);

  serde_json::to_string_pretty(&json!({
    "session_id": session_id.to_string(),
    "status": "running",
    "message": "Pipeline launched. Poll openworld_get_session for progress.",
    "hint": "Pipeline: Planning → Searching → VaultCheck → Reserving → Recovery → Finalising → Complete"
  })).unwrap_or_default()
}

// ─── Tool: get_session ────────────────────────────────────────────────────────

async fn tool_get_session(args: &Value, state: &McpState) -> String {
  let id_str = match args["session_id"].as_str() {
    Some(s) => s,
    None => return err_text("Missing session_id"),
  };
  let id = match Uuid::parse_str(id_str) {
    Ok(u) => u,
    Err(_) => return err_text("Invalid UUID"),
  };

  let registry = state.registry.read().await;
  let session  = match registry.get(&id) {
    Some(s) => s.clone(),
    None => return err_text("Session not found"),
  };
  drop(registry);

  let state_val  = session.current_state().await;
  let logs       = session.logs.lock().await.clone();
  let itinerary  = session.itinerary.lock().await.clone();
  let bookings   = session.bookings.lock().await.clone();

  // Return last 20 log lines to keep response compact
  let recent_logs: Vec<_> = logs.iter().rev().take(20).collect();

  serde_json::to_string_pretty(&json!({
    "session_id":   id_str,
    "state":        state_val,
    "created_at":   session.created_at,
    "destination":  session.policy.trip.destination,
    "budget_max":   session.policy.trip.budget_max,
    "itinerary":    itinerary,
    "bookings":     bookings,
    "recent_logs":  recent_logs
  })).unwrap_or_default()
}

// ─── Tool: approve_session ────────────────────────────────────────────────────

async fn tool_approve_session(args: &Value, state: &McpState) -> String {
  let id_str = match args["session_id"].as_str() {
    Some(s) => s,
    None => return err_text("Missing session_id"),
  };
  let approved = args["approved"].as_bool().unwrap_or(false);

  let id = match Uuid::parse_str(id_str) {
    Ok(u) => u,
    Err(_) => return err_text("Invalid UUID"),
  };

  let registry = state.registry.read().await;
  let session  = match registry.get(&id) {
    Some(s) => s.clone(),
    None => return err_text("Session not found"),
  };
  drop(registry);

  let current_state = session.current_state().await;
  if current_state != SessionState::AwaitingApproval {
    return err_text(&format!("Session is in state {current_state:?}, not AwaitingApproval"));
  }

  let sent = session.approve(approved).await;
  if !sent {
    return err_text("No approval gate active — session may have already been resolved");
  }

  serde_json::to_string_pretty(&json!({
    "session_id": id_str,
    "approved":   approved,
    "message": if approved {
      "Pipeline will continue with reservations"
    } else {
      "Pipeline cancelled — session will transition to Failed"
    }
  })).unwrap_or_default()
}

// ─── Tool: get_report ────────────────────────────────────────────────────────

async fn tool_get_report(args: &Value, state: &McpState) -> String {
  let id_str = match args["session_id"].as_str() {
    Some(s) => s,
    None => return err_text("Missing session_id"),
  };
  let id = match Uuid::parse_str(id_str) {
    Ok(u) => u,
    Err(_) => return err_text("Invalid UUID"),
  };

  let registry = state.registry.read().await;
  let session  = match registry.get(&id) {
    Some(s) => s.clone(),
    None => return err_text("Session not found"),
  };
  drop(registry);

  let current_state = session.current_state().await;
  if current_state != SessionState::Complete {
    return err_text(&format!("Session is {current_state:?} — report only available when Complete"));
  }

  let itinerary = session.itinerary.lock().await.clone();
  let bookings  = session.bookings.lock().await.clone();
  let _artifact = session.artifact.lock().await.clone();

  match itinerary {
    Some(itin) => generate_travel_report(
      &session.policy,
      &Some(itin),
      &bookings,
      "",    // artifact_id
      &session.session_id.to_string(),
      "",    // logs hash
      None,  // storage_root_hash
      None,  // report_root_hash
      None,  // on_chain_tx
      None,  // travel_tips
    ),
    None => err_text("No itinerary available — cannot generate report"),
  }
}

// ─── Resource handlers ────────────────────────────────────────────────────────

async fn resources_read(id: Value, params: Value, state: &McpState) -> Value {
  let uri = match params["uri"].as_str() {
    Some(u) => u.to_string(),
    None => return rpc_error(id, -32602, "Missing uri"),
  };

  let content = if uri == "openworld://sessions" {
    resource_sessions(state).await
  } else if let Some(sid) = uri.strip_prefix("openworld://session/") {
    resource_session(sid, state).await
  } else if let Some(sid) = uri.strip_prefix("openworld://report/") {
    resource_report(sid, state).await
  } else {
    return rpc_error(id, -32602, &format!("Unknown resource URI: {uri}"));
  };

  rpc_ok(id, json!({
    "contents": [{ "uri": uri, "text": content }]
  }))
}

async fn resource_sessions(state: &McpState) -> String {
  let registry = state.registry.read().await;
  let sessions: Vec<Value> = {
    let mut list = Vec::new();
    for (id, sess) in registry.iter() {
      let state = sess.current_state().await;
      list.push(json!({
        "session_id":  id.to_string(),
        "state":       state,
        "destination": sess.policy.trip.destination,
        "created_at":  sess.created_at,
      }));
    }
    list
  };
  serde_json::to_string_pretty(&sessions).unwrap_or_default()
}

async fn resource_session(id_str: &str, state: &McpState) -> String {
  tool_get_session(&json!({ "session_id": id_str }), state).await
}

async fn resource_report(id_str: &str, state: &McpState) -> String {
  tool_get_report(&json!({ "session_id": id_str }), state).await
}

// ─── JSON-RPC helpers ─────────────────────────────────────────────────────────

fn rpc_ok(id: Value, result: Value) -> Value {
  json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i32, message: &str) -> Value {
  json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn err_text(msg: &str) -> String {
  format!("ERROR: {msg}")
}
