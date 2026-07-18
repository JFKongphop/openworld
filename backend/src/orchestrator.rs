/*!
Orchestrator — the brain of the OpenWorld travel system.

Execution flow:
  1. Parse travel.md → TravelPolicy
  2. PlannerAgent    → Itinerary (Qwen AI)
  3. SearchAgent     → SearchResults (Firecrawl)
  4. VaultAgent      → pre-booking budget check
  5. ReservationAgent → BookingResults (OpenClaw)
  6. RecoveryAgent   → repair failed bookings (Qwen AI)
  7. VaultAgent      → post-booking spend verification
  8. ArtifactAgent   → ERC-7857 artifact + local storage

All agent activity is broadcast on the ExecutionContext log channel,
which the API layer forwards to WebSocket subscribers in real time.
*/

use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot, Mutex, RwLock};
use uuid::Uuid;

use crate::agents::{
  artifact::ArtifactAgent, planner::PlannerAgent, recovery::RecoveryAgent,
  reservation::ReservationAgent, search::SearchAgent, vault::VaultAgent, ActivityLog, Agent,
  BookingResult, ExecutionContext, Itinerary, JourneyArtifact, SearchResults,
};
use crate::memory_store::build_memory_store;
use crate::qwen_client::{build_qwen_client, QwenClient};
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
  /// Pipeline paused — waiting for human approval via POST /sessions/:id/approve
  AwaitingApproval,
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
  /// Human-in-the-loop approval gate.
  /// Orchestrator stores a sender here when entering AwaitingApproval.
  /// API handler calls Session::approve(true/false) to unblock the pipeline.
  pub approval_tx: Arc<Mutex<Option<oneshot::Sender<bool>>>>,
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
      approval_tx: Arc::new(Mutex::new(None)),
    }
  }

  /// Subscribe to live log events from this session
  pub fn subscribe(&self) -> broadcast::Receiver<ActivityLog> {
    self.log_tx.subscribe()
  }

  pub async fn current_state(&self) -> SessionState {
    self.state.read().await.clone()
  }

  /// Approve or reject a paused session.
  /// Returns false if the session is not currently awaiting approval.
  pub async fn approve(&self, approved: bool) -> bool {
    let mut slot = self.approval_tx.lock().await;
    if let Some(tx) = slot.take() {
      let _ = tx.send(approved);
      true
    } else {
      false
    }
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
      let _ = session
        .log_tx
        .send(ActivityLog::error("Orchestrator", &err_msg));
      *session.state.write().await = SessionState::Failed;
    }
  });
}

// ─── Pipeline ─────────────────────────────────────────────────────────────────

async fn orchestrate(session: Arc<Session>) -> Result<()> {
  let compute = build_qwen_client().unwrap_or_else(|_| {
    // Log warning but continue — agents have fallbacks
    QwenClient::new(
      "http://localhost:11434/v1/chat/completions".to_string(),
      "qwen-max".to_string(),
    )
  });
  let storage = build_memory_store().expect("Failed to create MemoryStore");

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

  // Wire log fan-out → Alibaba Cloud Log Service (fire-and-forget)
  if let Some(sls) = crate::log_service::build_log_service() {
    let session_id_str = session.session_id.to_string();
    let mut sls_rx = session.subscribe();
    tokio::spawn(async move {
      while let Ok(entry) = sls_rx.recv().await {
        let level = match entry.log_type {
          crate::agents::LogType::Success => "success",
          crate::agents::LogType::Warning => "warn",
          crate::agents::LogType::Error => "error",
          crate::agents::LogType::Action => "action",
          crate::agents::LogType::Info => "info",
        };
        // Best-effort — SLS errors are silently dropped to avoid blocking
        let _ = sls
          .put_log(&session_id_str, &entry.agent, level, &entry.message)
          .await;
      }
    });
  }

  emit(
    &ctx,
    "Orchestrator",
    &format!(
      "Session {} — orchestration starting",
      &session.session_id.to_string()[..8]
    ),
  );
  emit(
    &ctx,
    "Orchestrator",
    &format!(
      "{} → {} | Budget: {} USD | Duration: {} days",
      ctx.policy.trip.origin,
      ctx.policy.trip.destination,
      ctx.policy.trip.budget_max as u64,
      ctx.policy.trip.duration_days
    ),
  );

  // ── Step 1: Planning ──────────────────────────────────────────────────────
  *session.state.write().await = SessionState::Planning;
  let planner = PlannerAgent::new(compute.clone(), session.itinerary.clone());
  tokio::time::timeout(Duration::from_secs(240), planner.run(&ctx))
    .await
    .map_err(|_| anyhow::anyhow!("PlannerAgent timed out after 240s"))??;

  // ── Step 2: Searching ──────────────────────────────────────────────────────
  *session.state.write().await = SessionState::Searching;
  let searcher = SearchAgent::new(
    session.itinerary.clone(),
    session.search_results.clone(),
    compute.clone(),
  );
  tokio::time::timeout(Duration::from_secs(120), searcher.run(&ctx))
    .await
    .map_err(|_| anyhow::anyhow!("SearchAgent timed out after 120s"))??;

  // ── Step 3: Pre-booking budget check ──────────────────────────────────────
  *session.state.write().await = SessionState::VerifyingBudget;
  // VaultAgent first pass — just logs constraints, no bookings to verify yet
  emit(
    &ctx,
    "VaultAgent",
    &format!(
      "Pre-check: budget {:.0} USD, max single tx {:.0} USD",
      ctx.policy.trip.budget_max, ctx.policy.vault.max_single_transaction
    ),
  );

  // ── Step 4: Reservations ──────────────────────────────────────────────────
  *session.state.write().await = SessionState::Reserving;
  let reservations = ReservationAgent::new(
    session.itinerary.clone(),
    session.search_results.clone(),
    session.bookings.clone(),
  );
  tokio::time::timeout(Duration::from_secs(120), reservations.run(&ctx))
    .await
    .map_err(|_| anyhow::anyhow!("ReservationAgent timed out after 120s"))??;

  // ── Step 5: Recovery (if needed) ─────────────────────────────────────────
  *session.state.write().await = SessionState::Recovering;
  let recovery = RecoveryAgent::new(
    compute.clone(),
    session.itinerary.clone(),
    session.bookings.clone(),
  );
  tokio::time::timeout(Duration::from_secs(60), recovery.run(&ctx))
    .await
    .map_err(|_| anyhow::anyhow!("RecoveryAgent timed out after 60s"))??;

  // ── Step 6: Vault approval ────────────────────────────────────────────────
  *session.state.write().await = SessionState::VerifyingBudget;
  let vault = VaultAgent::new(session.bookings.clone(), ctx.policy.trip.budget_max);
  tokio::time::timeout(Duration::from_secs(30), vault.run(&ctx))
    .await
    .map_err(|_| anyhow::anyhow!("VaultAgent timed out after 30s"))??;
  // ── Step 6b: Human-in-the-loop gate ────────────────────────────────────
  {
    let vault_state = vault.state.lock().await;
    if vault_state.needs_approval {
      let reason = vault_state.approval_reason.clone().unwrap_or_default();
      drop(vault_state); // release lock before await

      let (tx, rx) = oneshot::channel::<bool>();
      *session.approval_tx.lock().await = Some(tx);
      *session.state.write().await = SessionState::AwaitingApproval;

      emit(
        &ctx,
        "VaultAgent",
        &format!("⏸ Pipeline paused — {}", reason),
      );
      emit(
        &ctx,
        "VaultAgent",
        "Waiting for POST /sessions/{id}/approve or /reject ...",
      );

      let approved = rx.await.unwrap_or(false);

      if approved {
        emit(
          &ctx,
          "VaultAgent",
          "✅ Trip approved by operator — continuing pipeline",
        );
      } else {
        emit(
          &ctx,
          "VaultAgent",
          "❌ Trip rejected by operator — aborting",
        );
        *session.state.write().await = SessionState::Failed;
        anyhow::bail!("Trip rejected at human-in-the-loop approval gate");
      }
    }
  }
  // ── Step 7: Artifact creation ─────────────────────────────────────────────
  *session.state.write().await = SessionState::Finalising;
  let artifact_agent = ArtifactAgent::new(
    storage,
    compute.clone(),
    session.itinerary.clone(),
    session.bookings.clone(),
    session.artifact.clone(),
  );
  tokio::time::timeout(Duration::from_secs(120), artifact_agent.run(&ctx))
    .await
    .map_err(|_| anyhow::anyhow!("ArtifactAgent timed out after 120s"))??;

  // ── Done ──────────────────────────────────────────────────────────────────
  *session.state.write().await = SessionState::Complete;
  emit(
    &ctx,
    "Orchestrator",
    "✓ All agents complete — journey execution finished",
  );

  Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn emit(ctx: &ExecutionContext, agent: &str, message: &str) {
  ctx.log(ActivityLog::info(agent, message));
}
