/*!
ArtifactAgent — ERC-7857 journey artifact creation and 0G Storage persistence.

Responsibilities:
  1. Hashes all booking references to produce verifiable booking proofs
  2. Hashes the full orchestration log for execution provenance
  3. Persists the artifact metadata to 0G Storage
  4. Optionally mints the artifact on-chain via the ERC-7857 contract

The artifact transforms the session from "temporary chat output"
into a persistent, verifiable record of autonomous execution.
*/

use anyhow::Result;
use async_trait::async_trait;
use chrono::Local;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{
  ActivityLog, Agent, BookingHash, BookingResult, BookingStatus, ExecutionContext,
  Itinerary, JourneyArtifact,
};
use crate::erc7857;
use crate::og_compute::OgComputeClient;
use crate::og_storage::{OgStorageClient, StoredArtifact};
use crate::report::generate_travel_report;

// ─── ArtifactAgent ────────────────────────────────────────────────────────────

pub struct ArtifactAgent {
  storage: OgStorageClient,
  compute: OgComputeClient,
  itinerary: Arc<Mutex<Option<Itinerary>>>,
  bookings: Arc<Mutex<Vec<BookingResult>>>,
  pub artifact: Arc<Mutex<Option<JourneyArtifact>>>,
}

impl ArtifactAgent {
  pub fn new(
    storage: OgStorageClient,
    compute: OgComputeClient,
    itinerary: Arc<Mutex<Option<Itinerary>>>,
    bookings: Arc<Mutex<Vec<BookingResult>>>,
    artifact: Arc<Mutex<Option<JourneyArtifact>>>,
  ) -> Self {
    Self {
      storage,
      compute,
      itinerary,
      bookings,
      artifact,
    }
  }
}

#[async_trait]
impl Agent for ArtifactAgent {
  fn name(&self) -> &str {
    "ArtifactAgent"
  }

  async fn run(&self, ctx: &ExecutionContext) -> Result<()> {
    ctx.log(ActivityLog::action(self.name(), "Preparing journey artifact..."));

    let itinerary = self.itinerary.lock().await.clone();
    let bookings = self.bookings.lock().await.clone();

    // Build booking hashes
    let booking_hashes: Vec<BookingHash> = bookings
      .iter()
      .filter(|b| b.status == BookingStatus::Confirmed)
      .map(|b| BookingHash {
        segment_id: b.segment_id.clone(),
        booking_type: b.booking_type.clone(),
        hash: hash_booking(b),
      })
      .collect();

    ctx.log(ActivityLog::info(
      self.name(),
      &format!("Hashed {} confirmed bookings", booking_hashes.len()),
    ));

    // Compute execution logs hash (hash of session_id + policy + booking refs)
    let log_preimage = format!(
      "{}|{}|{}",
      ctx.session_id,
      ctx.policy.to_constraint_json(),
      bookings
        .iter()
        .map(|b| b.reference.as_str())
        .collect::<Vec<_>>()
        .join(",")
    );
    let execution_logs_hash = format!("{:x}", md5::compute(log_preimage.as_bytes()));

    let total_spent: f64 = bookings
      .iter()
      .filter(|b| b.status == BookingStatus::Confirmed)
      .map(|b| b.price_usd)
      .sum();

    let destination = itinerary
      .as_ref()
      .map(|i| i.destination.as_str())
      .unwrap_or(&ctx.policy.trip.destination)
      .to_string();

    let trip_summary = format!(
      "{} → {} ({}d) — {:.0} USD spent on {} bookings",
      ctx.policy.trip.origin,
      destination,
      ctx.policy.trip.duration_days,
      total_spent,
      booking_hashes.len()
    );

    ctx.log(ActivityLog::info(self.name(), &format!("Trip: {}", trip_summary)));

    // Persist to 0G Storage
    ctx.log(ActivityLog::action(self.name(), "Persisting artifact to 0G Storage..."));

    let stored = StoredArtifact {
      artifact_id: Uuid::new_v4().to_string(),
      session_id: ctx.session_id.to_string(),
      trip_summary: trip_summary.clone(),
      total_spent,
      booking_hashes: booking_hashes.iter().map(|h| h.hash.clone()).collect(),
      execution_hash: execution_logs_hash.clone(),
      on_chain_tx: None,
      created_at: Local::now().to_rfc3339(),
      root_hash: None,
    };

    let root_hash = match self
      .storage
      .upload(&format!("artifact_{}", &stored.artifact_id[..8]), &stored)
      .await
    {
      Ok(h) => {
        ctx.log(ActivityLog::success(
          self.name(),
          &format!("✓ Artifact stored on 0G Storage — root: {}", &h[..std::cmp::min(h.len(), 16)]),
        ));
        Some(h)
      }
      Err(e) => {
        ctx.log(ActivityLog::warn(
          self.name(),
          &format!("0G Storage upload skipped ({})", e),
        ));
        None
      }
    };

    // ── Generate destination travel tips via 0G Compute ────────────────────
    ctx.log(ActivityLog::action(self.name(), "Generating destination guide via 0G Compute..."));
    let city_name = ctx.policy.resolved_city_name().to_string();
    let dep_date  = &ctx.policy.trip.departure_date;
    let ret_date  = &ctx.policy.trip.return_date;
    let tips_prompt = format!(
      r#"You are a travel expert writing a destination guide for a traveller visiting {city_name} from {dep_date} to {ret_date}.

CRITICAL RULES — YOU MUST FOLLOW THESE EXACTLY:
- Output ONLY plain Markdown text. NO JSON. NO code blocks. NO backticks.
- Use bullet points (- ) under each section header.
- Do NOT wrap the output in ``` or any code fence.
- Start your response directly with the first ### header.

Write exactly these six sections:

### 💴 Currency & Money
- Local currency name and symbol
- Approximate exchange rate to USD
- Cash vs card advice (Japan is cash-heavy)
- Best ATMs to use (e.g. 7-Eleven, Japan Post)

### 🚇 Local Transport
- Main transport options (subway, JR, bus)
- How to get an IC card (Suica / Pasmo) and load money
- Key metro lines relevant to {city_name}
- Estimated fare per ride in local currency

### 🌤️ Weather & Packing
- Expected temperatures and conditions during {dep_date} to {ret_date}
- What clothing to pack
- Any seasonal events or warnings (rain, heat, festivals)

### 🙏 Cultural Etiquette
- 4-5 essential customs to respect (shoes, tipping, queuing, etc.)

### 📱 Useful Apps
- 5 recommended apps with one-line descriptions (maps, transit, translation, food, payments)

### 🆘 Emergency Contacts
- Police number
- Ambulance / fire number
- Thai Embassy in {city_name} phone number

Keep each section to 4-6 bullet points. Be specific and practical. Do NOT output JSON."#
    );
    let travel_tips = self.compute.infer(&tips_prompt).await.ok();

    // ── Generate Markdown travel report ──────────────────────────────────────
    ctx.log(ActivityLog::action(self.name(), "Generating travel report..."));

    let itinerary_snap = self.itinerary.lock().await.clone();
    let report_md = generate_travel_report(
      &ctx.policy,
      &itinerary_snap,
      &bookings,
      &stored.artifact_id,
      &ctx.session_id.to_string(),
      &stored.execution_hash,
      root_hash.as_deref(),
      None, // report_root_hash — filled after upload
      None, // on_chain_tx — filled later if minted
      travel_tips.as_deref(),
    );

    let report_path = save_report(&ctx.policy.trip.destination, &ctx.session_id.to_string(), &report_md);
    match &report_path {
      Ok(p) => ctx.log(ActivityLog::success(
        self.name(),
        &format!("✓ Travel report saved → {}", p),
      )),
      Err(e) => ctx.log(ActivityLog::warn(
        self.name(),
        &format!("Report write failed: {}", e),
      )),
    }

    // ── Upload Markdown report to 0G Storage ────────────────────────────────
    let report_root_hash = match &report_path {
      Ok(path) => {
        ctx.log(ActivityLog::action(self.name(), "Uploading report to 0G Storage..."));
        let report_key = format!("report_{}", &stored.artifact_id[..8]);
        match self.storage.upload_text(&report_key, &report_md).await {
          Ok(h) => {
            ctx.log(ActivityLog::success(
              self.name(),
              &format!("✓ Report stored on 0G Storage — root: {}", &h[..std::cmp::min(h.len(), 16)]),
            ));
            let _ = path; // already used above
            Some(h)
          }
          Err(e) => {
            ctx.log(ActivityLog::warn(
              self.name(),
              &format!("Report 0G upload skipped ({})", e),
            ));
            None
          }
        }
      }
      Err(_) => None,
    };

    let artifact = JourneyArtifact {
      artifact_id: stored.artifact_id.clone(),
      session_id: ctx.session_id.to_string(),
      trip_summary,
      destination,
      duration_days: ctx.policy.trip.duration_days,
      total_spent_usd: total_spent,
      bookings: booking_hashes,
      execution_logs_hash,
      storage_root_hash: root_hash,
      report_root_hash,
      on_chain_tx: None,
      created_at: stored.created_at,
      report_path: report_path.ok(),
      owner_address: ctx.policy.trip.owner.clone(),
    };

    ctx.log(ActivityLog::success(
      self.name(),
      &format!("✓ Journey artifact created — ID: {}", &artifact.artifact_id[..8]),
    ));

    // ── Mint on-chain via ERC-7857 (OpenWorldJourney contract) ───────────────
    ctx.log(ActivityLog::action(self.name(), "Minting ERC-7857 journey artifact on 0G Testnet..."));
    let on_chain_tx = match erc7857::mint_journey_artifact(&artifact).await {
      Ok(tx) => {
        ctx.log(ActivityLog::success(
          self.name(),
          &format!("✓ ERC-7857 minted on 0G Testnet — tx: {}", tx),
        ));
        Some(tx)
      }
      Err(e) => {
        ctx.log(ActivityLog::warn(
          self.name(),
          &format!("ERC-7857 mint skipped ({})", e),
        ));
        None
      }
    };

    let artifact = JourneyArtifact { on_chain_tx: on_chain_tx.clone(), ..artifact };

    // Re-save the report now that we have the on-chain tx hash
    if on_chain_tx.is_some() {
      let itinerary_snap = self.itinerary.lock().await.clone();
      let bookings_snap = self.bookings.lock().await.clone();
      let updated_report = generate_travel_report(
        &ctx.policy,
        &itinerary_snap,
        &bookings_snap,
        &artifact.artifact_id,
        &ctx.session_id.to_string(),
        &artifact.execution_logs_hash,
        artifact.storage_root_hash.as_deref(),
        artifact.report_root_hash.as_deref(),
        artifact.on_chain_tx.as_deref(),
        travel_tips.as_deref(),
      );
      if let Some(path) = &artifact.report_path {
        let _ = std::fs::write(path, &updated_report);
        ctx.log(ActivityLog::info(self.name(), "Report updated with on-chain proof."));
      }
    }

    ctx.log(ActivityLog::success(self.name(), "Execution proof complete."));

    *self.artifact.lock().await = Some(artifact);
    Ok(())
  }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Write a Markdown report to the reports directory and return the absolute path.
fn save_report(destination: &str, session_id: &str, content: &str) -> Result<String> {
  let reports_dir = std::env::var("REPORTS_DIR")
    .unwrap_or_else(|_| {
      // Default: reports/ next to the binary's working directory
      let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
      manifest.join("reports").to_string_lossy().to_string()
    });

  std::fs::create_dir_all(&reports_dir)?;

  let filename = crate::report::report_filename(destination, session_id);
  let full_path = std::path::Path::new(&reports_dir).join(&filename);

  std::fs::write(&full_path, content)?;

  Ok(full_path.to_string_lossy().to_string())
}

fn hash_booking(b: &BookingResult) -> String {
  let preimage = format!(
    "{}|{}|{}|{:.2}|{}",
    b.segment_id, b.booking_type, b.provider, b.price_usd, b.reference
  );
  format!("{:x}", md5::compute(preimage.as_bytes()))
}
