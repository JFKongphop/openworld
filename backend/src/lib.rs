/*!
OpenWorld — Autonomous agentic travel planning and reservation system.

Architecture:
  travel.md → Orchestrator → [PlannerAgent, SearchAgent, ReservationAgent,
                               RecoveryAgent, VaultAgent, ArtifactAgent]
                           → Signed execution artifact + Markdown report

Powered by Qwen (Alibaba Cloud Model Studio) + Alibaba Cloud OSS/KMS/Log Service.
*/

pub mod agents;
pub mod qwen_client;
pub mod memory_store;
pub mod orchestrator;
pub mod report;
pub mod travel_spec;

pub use qwen_client::{build_qwen_client, QwenClient};
pub use memory_store::{build_memory_store, MemoryStore};
pub use orchestrator::{create_session, new_registry, run_session, Session, SessionRegistry, SessionState};
pub use travel_spec::{parse_travel_md, TravelPolicy};

pub use agents::{
  ActivityLog, BookingResult, BookingStatus, ExecutionContext, FlightOption, HotelOption,
  Itinerary, JourneyArtifact, LogType, SearchResults, SegmentKind, TransportOption, TravelSegment,
};

/// Load `.env` from the project root
pub fn load_env() {
  dotenv::dotenv().ok();
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
  dotenv::from_path(root.join(".env")).ok();
}
