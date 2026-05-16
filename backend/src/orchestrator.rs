/*!
Orchestrator — the brain of the OpenWorld travel system.

Execution flow:
  1. Parse travel.md → TravelPolicy
  2. PlannerAgent    → Itinerary (0G Compute)
  3. SearchAgent     → SearchResults (Firecrawl)
  4. VaultAgent      → pre-booking budget check
  5. ReservationAgent → BookingResults (OpenClaw)
  6. RecoveryAgent   → repair failed bookings (0G Compute)
  7. VaultAgent      → post-booking spend verification
  8. ArtifactAgent   → ERC-7857 artifact + 0G Storage

All agent activity is broadcast on the ExecutionContext log channel,
which the API layer forwards to WebSocket subscribers in real time.
*/

use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};
use uuid::Uuid;

use crate::agents::{
  artifact::ArtifactAgent,
  planner::PlannerAgent,
  recovery::RecoveryAgent,
  reservation::ReservationAgent,
  search::SearchAgent,
  vault::VaultAgent,
  ActivityLog, Agent, BookingResult, ExecutionContext, Itinerary, JourneyArtifact, SearchResults,
};
use crate::og_compute::{build_og_compute, OgComputeClient};
use crate::og_storage::build_og_storage;
use crate::travel_spec::{parse_travel_md, TravelPolicy};

// ─── Session State ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
  Created,
  Planning,
  Searching,
  VerifyingBudget,
  Reserving,
  Recovering,
  Finalising,
  Complete,
  Failed,
}

/// Full runtime state of one orchestration session
#[derive(Clone)]
pub struct Session {
  pub session_id: Uuid,
  pub policy: TravelPolicy,
  pub state: Arc<RwLock<SessionState>>,
  pub logs: Arc<Mutex<Vec<ActivityLog>>>,
  pub itinerary: Arc<Mutex<Option<Itinerary>>>,
  pub search_results: Arc<Mutex<SearchResults>>,
  pub bookings: Arc<Mutex<Vec<BookingResult>>>,
  pub artifact: Arc<Mutex<Option<JourneyArtifact>>>,
  /// Broadcast channel for live log streaming to WebSocket subscribers
  pub log_tx: broadcast::Sender<ActivityLog>,
  pub created_at: String,
}

impl Session {
  pub fn new(policy: TravelPolicy) -> Self {
    let (tx, _) = broadcast::channel(512);
    Self {
      session_id: Uuid::new_v4(),
      policy,
      state: Arc::new(RwLock::new(SessionState::Created)),
      logs: Arc::new(Mutex::new(Vec::new())),
      itinerary: Arc::new(Mutex::new(None)),
      search_results: Arc::new(Mutex::new(SearchResults::default())),
      bookings: Arc::new(Mutex::new(Vec::new())),
      artifact: Arc::new(Mutex::new(None)),
      log_tx: tx,
      created_at: Local::now().to_rfc3339(),
    }
  }

  /// Subscribe to live log events from this session
  pub fn subscribe(&self) -> broadcast::Receiver<ActivityLog> {
    self.log_tx.subscribe()
  }

  pub async fn current_state(&self) -> SessionState {
    self.state.read().await.clone()
  }
}

// ─── Orchestrator ─────────────────────────────────────────────────────────────

/// Session registry — holds all active and completed sessions
pub type SessionRegistry = Arc<RwLock<HashMap<Uuid, Arc<Session>>>>;

pub fn new_registry() -> SessionRegistry {
  Arc::new(RwLock::new(HashMap::new()))
}

/// Create a new session from travel.md YAML content
pub fn create_session(yaml: &str) -> Result<Arc<Session>> {
  let policy = parse_travel_md(yaml)?;
  let errors = policy.validate();
  if !errors.is_empty() {
    anyhow::bail!("Invalid travel.md: {}", errors.join("; "));
  }
  Ok(Arc::new(Session::new(policy)))
}

/// Spawn the orchestration pipeline for a session in a background task
pub fn run_session(session: Arc<Session>) {
  tokio::spawn(async move {
    if let Err(e) = orchestrate(session.clone()).await {
      let err_msg = format!("Orchestration failed: {}", e);
      let _ = session.log_tx.send(ActivityLog::error("Orchestrator", &err_msg));
      *session.state.write().await = SessionState::Failed;
    }
  });
}

// ─── Pipeline ─────────────────────────────────────────────────────────────────

async fn orchestrate(session: Arc<Session>) -> Result<()> {
  let compute = build_og_compute().unwrap_or_else(|_| {
    // Log warning but continue — agents have fallbacks
    OgComputeClient::new("http://localhost:11434/v1/chat/completions".to_string(), "llama3".to_string())
  });
  let storage = build_og_storage();

  // Build ExecutionContext wired to this session's broadcast channel
  let ctx = ExecutionContext {
    session_id: session.session_id,
    policy: session.policy.clone(),
    log_tx: session.log_tx.clone(),
  };

  // Wire log fan-out → session.logs (persistent) AND broadcast
  let logs_store = session.logs.clone();
  let mut log_rx = session.subscribe();
  tokio::spawn(async move {
    while let Ok(entry) = log_rx.recv().await {
      logs_store.lock().await.push(entry);
    }
  });

  emit(&ctx, "Orchestrator", &format!(
    "Session {} — orchestration starting",
    &session.session_id.to_string()[..8]
  ));
  emit(&ctx, "Orchestrator", &format!(
    "{} → {} | Budget: {} USD | Duration: {} days",
    ctx.policy.trip.origin,
    ctx.policy.trip.destination,
    ctx.policy.trip.budget_max as u64,
    ctx.policy.trip.duration_days
  ));

  // ── Step 1: Planning ──────────────────────────────────────────────────────
  *session.state.write().await = SessionState::Planning;
  let planner = PlannerAgent::new(compute.clone(), session.itinerary.clone());
  planner.run(&ctx).await?;

  // ── Step 2: Searching ──────────────────────────────────────────────────────
  *session.state.write().await = SessionState::Searching;
  let searcher = SearchAgent::new(session.itinerary.clone(), session.search_results.clone(), compute.clone());
  searcher.run(&ctx).await?;

  // ── Step 3: Pre-booking budget check ──────────────────────────────────────
  *session.state.write().await = SessionState::VerifyingBudget;
  // VaultAgent first pass — just logs constraints, no bookings to verify yet
  emit(&ctx, "VaultAgent", &format!(
    "Pre-check: budget {:.0} USD, max single tx {:.0} USD",
    ctx.policy.trip.budget_max,
    ctx.policy.vault.max_single_transaction
  ));

  // ── Step 4: Reservations ──────────────────────────────────────────────────
  *session.state.write().await = SessionState::Reserving;
  let reservations = ReservationAgent::new(
    session.itinerary.clone(),
    session.search_results.clone(),
    session.bookings.clone(),
  );
  reservations.run(&ctx).await?;

  // ── Step 5: Recovery (if needed) ─────────────────────────────────────────
  *session.state.write().await = SessionState::Recovering;
  let recovery = RecoveryAgent::new(
    compute.clone(),
    session.itinerary.clone(),
    session.bookings.clone(),
  );
  recovery.run(&ctx).await?;

  // ── Step 6: Vault approval ────────────────────────────────────────────────
  *session.state.write().await = SessionState::VerifyingBudget;
  let vault = VaultAgent::new(session.bookings.clone(), ctx.policy.trip.budget_max);
  vault.run(&ctx).await?;

  // ── Step 7: Artifact creation ─────────────────────────────────────────────
  *session.state.write().await = SessionState::Finalising;
  let artifact_agent = ArtifactAgent::new(
    storage,
    compute.clone(),
    session.itinerary.clone(),
    session.bookings.clone(),
    session.artifact.clone(),
  );
  artifact_agent.run(&ctx).await?;

  // ── Done ──────────────────────────────────────────────────────────────────
  *session.state.write().await = SessionState::Complete;
  emit(&ctx, "Orchestrator", "✓ All agents complete — journey execution finished");

  Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn emit(ctx: &ExecutionContext, agent: &str, message: &str) {
  ctx.log(ActivityLog::info(agent, message));
}
