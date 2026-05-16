/*!
OpenWorld — Autonomous agentic travel planning and reservation system.

Architecture:
  travel.md → Orchestrator → [PlannerAgent, SearchAgent, ReservationAgent,
                               RecoveryAgent, VaultAgent, ArtifactAgent]
                           → ERC-7857 Journey Artifact on 0G Storage

Powered by 0G Compute (LLM reasoning) + 0G Storage (persistent memory).
*/

pub mod agents;
pub mod erc7857;
pub mod og_compute;
pub mod og_storage;
pub mod orchestrator;
pub mod report;
pub mod travel_spec;

pub use og_compute::{build_og_compute, OgComputeClient};
pub use og_storage::{build_og_storage, OgStorageClient};
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
