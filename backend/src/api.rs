/*!
OpenWorld API Server — HTTP interface for agentic travel orchestration.

Usage:
  cargo run --bin api

Endpoints:
  POST   /sessions                  { "travel_md": "..." }
                                    → { "session_id": "uuid", "state": "created" }

  POST   /sessions/{id}/start       {} → starts orchestration pipeline
                                    → { "session_id": "...", "state": "planning" }

  GET    /sessions/{id}             → { "session_id", "state", "policy", "itinerary",
                                        "bookings", "artifact", "logs" }

  GET    /sessions/{id}/logs        → [ { "timestamp", "agent", "message", "log_type" }, ... ]

  GET    /sessions/{id}/artifact    → JourneyArtifact JSON

  GET    /ws/{id}                   WebSocket — streams ActivityLog as JSON lines

  GET    /health                    → { "status": "ok" }
*/

use std::sync::Arc;

use axum::{
  body::Body,
  extract::{
    ws::{Message, WebSocket, WebSocketUpgrade},
    Path, State,
  },
  http::{header, StatusCode},
  response::{IntoResponse, Response},
  routing::{get, post},
  Json, Router,
};
use openworld::{
  create_session, load_env, new_registry, run_session, ActivityLog,
  Session, SessionRegistry, SessionState,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

// ─── App State ────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
  sessions: SessionRegistry,
}

// ─── Request / Response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateSessionRequest {
  /// Raw content of travel.md (YAML)
  travel_md: String,
}

#[derive(Serialize)]
struct SessionSummary {
  session_id: String,
  state: String,
  destination: String,
  budget_max: f64,
  duration_days: u32,
  created_at: String,
}

#[derive(Serialize)]
struct SessionDetail {
  session_id: String,
  state: String,
  policy: serde_json::Value,
  itinerary: serde_json::Value,
  bookings: serde_json::Value,
  artifact: serde_json::Value,
  log_count: usize,
}

// ─── Error type ───────────────────────────────────────────────────────────────

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
  fn into_response(self) -> axum::response::Response {
    (
      StatusCode::INTERNAL_SERVER_ERROR,
      Json(json!({ "error": self.0.to_string() })),
    )
      .into_response()
  }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
  fn from(e: E) -> Self {
    AppError(e.into())
  }
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// POST /sessions — create a new session from travel.md content
#[axum::debug_handler]
async fn create_session_handler(
  State(state): State<AppState>,
  Json(req): Json<CreateSessionRequest>,
) -> Result<Json<SessionSummary>, AppError> {
  let session = create_session(&req.travel_md)?;
  let summary = session_summary(&session).await;

  state
    .sessions
    .write()
    .await
    .insert(session.session_id, session);

  Ok(Json(summary))
}

/// POST /sessions/{id}/start — launch the orchestration pipeline
#[axum::debug_handler]
async fn start_session_handler(
  State(state): State<AppState>,
  Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
  let session = get_session(&state.sessions, id).await?;

  // Guard against double-start
  if session.current_state().await != SessionState::Created {
    return Err(anyhow::anyhow!("Session already started or completed").into());
  }

  run_session(session.clone());

  Ok(Json(json!({
    "session_id": id.to_string(),
    "state": "planning",
    "message": "Orchestration started"
  })))
}

/// GET /sessions/{id} — full session detail
#[axum::debug_handler]
async fn get_session_handler(
  State(state): State<AppState>,
  Path(id): Path<Uuid>,
) -> Result<Json<SessionDetail>, AppError> {
  let session = get_session(&state.sessions, id).await?;

  let state_str = format!("{:?}", session.current_state().await)
    .to_lowercase()
    .replace('"', "");

  let detail = SessionDetail {
    session_id: id.to_string(),
    state: state_str,
    policy: serde_json::to_value(&session.policy).unwrap_or_default(),
    itinerary: serde_json::to_value(&*session.itinerary.lock().await).unwrap_or_default(),
    bookings: serde_json::to_value(&*session.bookings.lock().await).unwrap_or_default(),
    artifact: serde_json::to_value(&*session.artifact.lock().await).unwrap_or_default(),
    log_count: session.logs.lock().await.len(),
  };

  Ok(Json(detail))
}

/// GET /sessions/{id}/logs — full activity log list
#[axum::debug_handler]
async fn get_logs_handler(
  State(state): State<AppState>,
  Path(id): Path<Uuid>,
) -> Result<Json<Vec<ActivityLog>>, AppError> {
  let session = get_session(&state.sessions, id).await?;
  let logs = session.logs.lock().await.clone();
  Ok(Json(logs))
}

/// GET /sessions/{id}/artifact — journey artifact JSON
#[axum::debug_handler]
async fn get_artifact_handler(
  State(state): State<AppState>,
  Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
  let session = get_session(&state.sessions, id).await?;
  let artifact = session.artifact.lock().await.clone();
  match artifact {
    Some(a) => Ok(Json(serde_json::to_value(a)?)),
    None => Err(anyhow::anyhow!("Artifact not yet created for session {}", id).into()),
  }
}

/// GET /ws/{id} — WebSocket for live activity log streaming
async fn ws_handler(
  ws: WebSocketUpgrade,
  Path(id): Path<Uuid>,
  State(state): State<AppState>,
) -> impl IntoResponse {
  ws.on_upgrade(move |socket| handle_ws(socket, id, state))
}

async fn handle_ws(mut socket: WebSocket, id: Uuid, state: AppState) {
  let session = match state.sessions.read().await.get(&id).cloned() {
    Some(s) => s,
    None => {
      let msg = json!({ "error": format!("Session {} not found", id) });
      let _ = socket.send(Message::Text(msg.to_string())).await;
      return;
    }
  };

  // Send existing logs first so the client gets history on connect
  let past_logs = session.logs.lock().await.clone();
  for log in &past_logs {
    if let Ok(text) = serde_json::to_string(log) {
      if socket.send(Message::Text(text)).await.is_err() {
        return;
      }
    }
  }

  // Subscribe to live updates
  let mut rx = session.subscribe();

  loop {
    tokio::select! {
      // New log from orchestration pipeline
      Ok(log) = rx.recv() => {
        match serde_json::to_string(&log) {
          Ok(text) => {
            if socket.send(Message::Text(text)).await.is_err() {
              break; // Client disconnected
            }
          }
          Err(_) => continue,
        }

        // Close connection when session is done
        let current = session.current_state().await;
        if current == SessionState::Complete || current == SessionState::Failed {
          let done_msg = json!({ "event": "session_complete", "state": format!("{:?}", current) });
          let _ = socket.send(Message::Text(done_msg.to_string())).await;
          break;
        }
      }
      // Client closed
      else => break,
    }
  }
}

/// GET /sessions/{id}/report — return the Markdown travel report
#[axum::debug_handler]
async fn get_report_handler(
  State(state): State<AppState>,
  Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
  let session = get_session(&state.sessions, id).await?;
  let artifact = session.artifact.lock().await.clone();

  let artifact = artifact
    .ok_or_else(|| anyhow::anyhow!("Report not yet available for session {}", id))?;

  let path = artifact
    .report_path
    .ok_or_else(|| anyhow::anyhow!("No report path recorded for session {}", id))?;

  let content = std::fs::read_to_string(&path)
    .map_err(|e| anyhow::anyhow!("Cannot read report at {}: {}", path, e))?;

  Ok(
    Response::builder()
      .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
      .header(
        header::CONTENT_DISPOSITION,
        format!("inline; filename=\"{}\"", std::path::Path::new(&path)
          .file_name()
          .and_then(|n| n.to_str())
          .unwrap_or("report.md")),
      )
      .body(Body::from(content))
      .unwrap(),
  )
}

/// GET /health
async fn health() -> impl IntoResponse {
  Json(json!({
    "status": "ok",
    "service": "OpenWorld Agentic Travel API",
    "version": env!("CARGO_PKG_VERSION")
  }))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

async fn get_session(
  registry: &SessionRegistry,
  id: Uuid,
) -> Result<Arc<Session>, AppError> {
  registry
    .read()
    .await
    .get(&id)
    .cloned()
    .ok_or_else(|| anyhow::anyhow!("Session {} not found", id).into())
}

async fn session_summary(session: &Session) -> SessionSummary {
  SessionSummary {
    session_id: session.session_id.to_string(),
    state: format!("{:?}", session.current_state().await).to_lowercase(),
    destination: session.policy.trip.destination.clone(),
    budget_max: session.policy.trip.budget_max,
    duration_days: session.policy.trip.duration_days,
    created_at: session.created_at.clone(),
  }
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
  load_env();

  let port: u16 = std::env::var("PORT")
    .unwrap_or_else(|_| "3000".to_string())
    .parse()
    .unwrap_or(3000);

  let state = AppState {
    sessions: new_registry(),
  };

  let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods(Any)
    .allow_headers(Any);

  let app = Router::new()
    .route("/health", get(health))
    .route("/sessions", post(create_session_handler))
    .route("/sessions/:id", get(get_session_handler))
    .route("/sessions/:id/start", post(start_session_handler))
    .route("/sessions/:id/logs", get(get_logs_handler))
    .route("/sessions/:id/artifact", get(get_artifact_handler))
    .route("/sessions/:id/report", get(get_report_handler))
    .route("/ws/:id", get(ws_handler))
    .layer(cors)
    .with_state(state);

  let addr = format!("0.0.0.0:{}", port);
  println!("\x1b[32m[OpenWorld]\x1b[0m  API server listening on http://{}", addr);
  println!("\x1b[2m  POST /sessions          — create session from travel.md\x1b[0m");
  println!("\x1b[2m  POST /sessions/:id/start — launch orchestration\x1b[0m");
  println!("\x1b[2m  GET  /sessions/:id/report — markdown travel report\x1b[0m");
  println!("\x1b[2m  GET  /ws/:id             — live WebSocket stream\x1b[0m");

  let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
  axum::serve(listener, app).await.unwrap();
}
