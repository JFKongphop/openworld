/*!
Agent framework — shared types, trait, and execution context for all travel agents.

Agents:
  PlannerAgent      — itinerary generation via 0G Compute
  SearchAgent       — flight/hotel/transport discovery via Firecrawl
  ReservationAgent  — booking execution via OpenClaw browser automation
  RecoveryAgent     — failure recovery and replanning via 0G Compute
  VaultAgent        — budget enforcement and payment authorisation
  ArtifactAgent     — ERC-7857 journey artifact creation + 0G Storage persistence
*/

pub mod artifact;
pub mod planner;
pub mod recovery;
pub mod reservation;
pub mod search;
pub mod vault;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Local;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::travel_spec::TravelPolicy;

// ─── Activity Log ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogType {
  Info,
  Success,
  Warning,
  Error,
  Action,
}

/// One line of agent activity — streamed live to the frontend terminal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityLog {
  pub timestamp: String,
  pub agent: String,
  pub message: String,
  pub log_type: LogType,
}

impl ActivityLog {
  pub fn new(agent: &str, message: &str, log_type: LogType) -> Self {
    Self {
      timestamp: Local::now().format("%H:%M:%S").to_string(),
      agent: agent.to_string(),
      message: message.to_string(),
      log_type,
    }
  }

  pub fn info(agent: &str, message: &str) -> Self {
    Self::new(agent, message, LogType::Info)
  }

  pub fn success(agent: &str, message: &str) -> Self {
    Self::new(agent, message, LogType::Success)
  }

  pub fn warn(agent: &str, message: &str) -> Self {
    Self::new(agent, message, LogType::Warning)
  }

  pub fn error(agent: &str, message: &str) -> Self {
    Self::new(agent, message, LogType::Error)
  }

  pub fn action(agent: &str, message: &str) -> Self {
    Self::new(agent, message, LogType::Action)
  }
}

// ─── Itinerary ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
  Flight,
  Hotel,
  Train,
  Bus,
  Transfer,
}

/// One leg / stay in a planned itinerary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TravelSegment {
  pub id: String,
  pub kind: SegmentKind,
  pub from: String,
  pub to: Option<String>,
  pub date: String,
  pub duration: Option<String>,
  pub provider_hints: Vec<String>,
  pub estimated_price_usd: f64,
}

/// One activity within a planned day
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyActivity {
  /// Time slot — e.g. "09:00", "Morning", "Evening"
  pub time: String,
  /// Short description — e.g. "Visit Senso-ji Temple"
  pub activity: String,
  /// Area / location name
  pub location: String,
  /// Rough cost in USD (0 if free)
  pub est_cost_usd: f64,
  /// Optional tips or notes
  pub notes: Option<String>,
}

/// A single day's schedule within the itinerary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayPlan {
  pub day: u32,
  pub date: String,
  /// Theme / headline for the day — e.g. "Explore Shinjuku & Shibuya"
  pub title: String,
  pub activities: Vec<DailyActivity>,
}

/// Full planned itinerary produced by PlannerAgent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Itinerary {
  pub destination: String,
  pub duration_days: u32,
  pub segments: Vec<TravelSegment>,
  pub estimated_total_usd: f64,
  pub reasoning: String,
  /// Day-by-day activity schedule (filled when LLM supports it)
  #[serde(default)]
  pub daily_plan: Vec<DayPlan>,
}

// ─── Search Results ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightOption {
  pub airline: String,
  pub route: String,
  pub departure: String,
  pub arrival: String,
  pub stops: u32,
  pub duration: String,
  pub price_usd: f64,
  pub booking_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotelOption {
  pub name: String,
  pub location: String,
  pub price_per_night_usd: f64,
  pub rating: f64,
  pub near_station: bool,
  pub booking_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportOption {
  pub provider: String,
  pub route: String,
  pub kind: String,
  pub departure: String,
  pub price_usd: f64,
  pub booking_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchResults {
  pub flights: Vec<FlightOption>,
  pub hotels: Vec<HotelOption>,
  pub transport: Vec<TransportOption>,
}

// ─── Booking ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BookingStatus {
  Confirmed,
  Pending,
  Failed,
  Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingResult {
  pub segment_id: String,
  pub booking_type: String,
  pub provider: String,
  pub reference: String,
  pub price_usd: f64,
  pub status: BookingStatus,
  pub confirmation_url: Option<String>,
}

// ─── Journey Artifact (ERC-7857) ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingHash {
  pub segment_id: String,
  pub booking_type: String,
  pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyArtifact {
  pub artifact_id: String,
  pub session_id: String,
  pub trip_summary: String,
  pub destination: String,
  pub duration_days: u32,
  pub total_spent_usd: f64,
  pub bookings: Vec<BookingHash>,
  pub execution_logs_hash: String,
  pub storage_root_hash: Option<String>,
  pub report_root_hash: Option<String>,
  pub on_chain_tx: Option<String>,
  pub created_at: String,
  /// Path to the generated Markdown travel report on disk
  #[serde(default)]
  pub report_path: Option<String>,
  /// EVM wallet address that owns this journey NFT (from trip.md `owner:` field).
  /// Falls back to the operator wallet if not specified.
  #[serde(default)]
  pub owner_address: Option<String>,
}

// ─── Execution Context ────────────────────────────────────────────────────────

/// Shared state passed into every agent during orchestration
#[derive(Clone)]
pub struct ExecutionContext {
  pub session_id: Uuid,
  pub policy: TravelPolicy,
  /// Broadcast channel — send logs to all WebSocket subscribers
  pub log_tx: broadcast::Sender<ActivityLog>,
}

impl ExecutionContext {
  pub fn new(policy: TravelPolicy) -> (Self, broadcast::Receiver<ActivityLog>) {
    let (tx, rx) = broadcast::channel(256);
    let ctx = Self {
      session_id: Uuid::new_v4(),
      policy,
      log_tx: tx,
    };
    (ctx, rx)
  }

  /// Emit a log entry to all subscribers
  pub fn log(&self, entry: ActivityLog) {
    let _ = self.log_tx.send(entry);
  }
}

// ─── Agent Trait ──────────────────────────────────────────────────────────────

#[async_trait]
pub trait Agent: Send + Sync {
  /// Agent display name shown in activity terminal
  fn name(&self) -> &str;

  /// Execute the agent's work within the given context
  async fn run(&self, ctx: &ExecutionContext) -> Result<()>;
}
