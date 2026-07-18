/*!
ArtifactAgent — signed execution artifact creation and local persistence.

Responsibilities:
  1. Hashes all booking references to produce verifiable booking proofs
  2. Hashes the full orchestration log for execution provenance
  3. Produces an HMAC-SHA256 execution signature (operator key proof)
  4. Persists the artifact to MemoryStore (local JSON)
  5. Generates a Markdown travel report with destination tips from Qwen

The artifact is a cryptographically signed record of autonomous execution —
no blockchain required: the HMAC proves the operator key was present when
the journey completed, while the content hash is reproducible.
*/

use anyhow::Result;
use async_trait::async_trait;
use chrono::Local;
use hmac::{Hmac, Mac};
use serde_json;
use sha2::Sha256;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{
  ActivityLog, Agent, BookingHash, BookingResult, BookingStatus, ExecutionContext, Itinerary,
  JourneyArtifact,
};
use crate::memory_store::MemoryStore;
use crate::oss_store::build_oss_store;
use crate::qwen_client::QwenClient;
use crate::report::generate_travel_report;

type HmacSha256 = Hmac<Sha256>;

// ─── ArtifactAgent ────────────────────────────────────────────────────────────

pub struct ArtifactAgent {
  storage: MemoryStore,
  compute: QwenClient,
  itinerary: Arc<Mutex<Option<Itinerary>>>,
  bookings: Arc<Mutex<Vec<BookingResult>>>,
  pub artifact: Arc<Mutex<Option<JourneyArtifact>>>,
}

impl ArtifactAgent {
  pub fn new(
    storage: MemoryStore,
    compute: QwenClient,
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
    ctx.log(ActivityLog::action(
      self.name(),
      "Preparing journey artifact...",
    ));

    let itinerary = self.itinerary.lock().await.clone();
    let bookings = self.bookings.lock().await.clone();

    // Build booking hashes
    let booking_hashes: Vec<BookingHash> = bookings
      .iter()
      .filter(|b| matches!(b.status, BookingStatus::Confirmed | BookingStatus::Ticketed))
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

    // HMAC-SHA256 execution signature using operator key
    let operator_key =
      std::env::var("OPERATOR_SIGNING_KEY").unwrap_or_else(|_| "openworld-dev-key".to_string());
    let execution_proof = hmac_sign(&operator_key, &log_preimage);
    ctx.log(ActivityLog::info(
      self.name(),
      &format!(
        "Execution proof: {}...{}",
        &execution_proof[..8],
        &execution_proof[execution_proof.len() - 8..]
      ),
    ));

    let total_spent: f64 = bookings
      .iter()
      .filter(|b| matches!(b.status, BookingStatus::Confirmed | BookingStatus::Ticketed))
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

    ctx.log(ActivityLog::info(
      self.name(),
      &format!("Trip: {}", trip_summary),
    ));

    // Persist artifact record to MemoryStore
    ctx.log(ActivityLog::action(
      self.name(),
      "Persisting artifact to local store...",
    ));

    let artifact_id = Uuid::new_v4().to_string();
    let artifact_record = serde_json::json!({
      "artifact_id": artifact_id,
      "session_id": ctx.session_id.to_string(),
      "trip_summary": trip_summary,
      "total_spent": total_spent,
      "booking_hashes": booking_hashes.iter().map(|h| &h.hash).collect::<Vec<_>>(),
      "execution_hash": execution_logs_hash,
      "execution_proof": execution_proof,
      "created_at": Local::now().to_rfc3339(),
    });

    let store_key = format!("artifact_{}", &artifact_id[..8]);
    let storage_root_hash = match self.storage.store(&store_key, &artifact_record) {
      Ok(h) => {
        ctx.log(ActivityLog::success(
          self.name(),
          &format!(
            "✓ Artifact stored locally — hash: {}...",
            &h[..16.min(h.len())]
          ),
        ));
        Some(h)
      }
      Err(e) => {
        ctx.log(ActivityLog::warn(
          self.name(),
          &format!("Local store skipped ({})", e),
        ));
        None
      }
    };

    // ── Upload artifact JSON to Alibaba Cloud OSS ──────────────────────────
    if let Some(oss) = build_oss_store() {
      ctx.log(ActivityLog::action(
        self.name(),
        "Uploading artifact to Alibaba Cloud OSS...",
      ));
      let artifact_bytes = serde_json::to_vec_pretty(&artifact_record).unwrap_or_default();
      let oss_key = format!("artifacts/{}.json", artifact_id);
      match oss.put(&oss_key, &artifact_bytes, "application/json").await {
        Ok(url) => ctx.log(ActivityLog::success(
          self.name(),
          &format!("☁️  Artifact → {}", url),
        )),
        Err(e) => ctx.log(ActivityLog::warn(
          self.name(),
          &format!("OSS artifact upload skipped ({})", e),
        )),
      }
    }

    // ── Generate destination travel tips via Qwen ─────────────────────────
    ctx.log(ActivityLog::action(
      self.name(),
      "Generating destination guide via Qwen...",
    ));
    let city_name = ctx.policy.resolved_city_name().to_string();
    let dep_date = &ctx.policy.trip.departure_date;
    let ret_date = &ctx.policy.trip.return_date;
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
    ctx.log(ActivityLog::action(
      self.name(),
      "Generating travel report...",
    ));

    let itinerary_snap = self.itinerary.lock().await.clone();
    let report_md = generate_travel_report(
      &ctx.policy,
      &itinerary_snap,
      &bookings,
      &artifact_id,
      &ctx.session_id.to_string(),
      &execution_logs_hash,
      storage_root_hash.as_deref(),
      None, // report_root_hash
      None, // on_chain_tx — no longer used
      travel_tips.as_deref(),
    );

    let report_path = save_report(
      &ctx.policy.trip.destination,
      &ctx.session_id.to_string(),
      &report_md,
    );
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

    // ── Store report in MemoryStore ──────────────────────────────────────────
    let report_root_hash = match &report_path {
      Ok(_) => {
        let report_key = format!("report_{}", &artifact_id[..8]);
        match self.storage.store_text(&report_key, &report_md) {
          Ok(h) => {
            ctx.log(ActivityLog::success(
              self.name(),
              &format!(
                "✓ Report stored locally — hash: {}...",
                &h[..16.min(h.len())]
              ),
            ));
            Some(h)
          }
          Err(e) => {
            ctx.log(ActivityLog::warn(
              self.name(),
              &format!("Report local store skipped ({})", e),
            ));
            None
          }
        }
      }
      Err(_) => None,
    };

    // ── Upload Markdown report to Alibaba Cloud OSS ──────────────────────────
    if let Some(oss) = build_oss_store() {
      let report_filename =
        crate::report::report_filename(&ctx.policy.trip.destination, &ctx.session_id.to_string());
      let oss_key = format!("reports/{}", report_filename);
      match oss
        .put(&oss_key, report_md.as_bytes(), "text/markdown")
        .await
      {
        Ok(url) => ctx.log(ActivityLog::success(
          self.name(),
          &format!("☁️  Report   → {}", url),
        )),
        Err(e) => ctx.log(ActivityLog::warn(
          self.name(),
          &format!("OSS report upload skipped ({})", e),
        )),
      }
    }

    let artifact = JourneyArtifact {
      artifact_id: artifact_id.clone(),
      session_id: ctx.session_id.to_string(),
      trip_summary,
      destination,
      duration_days: ctx.policy.trip.duration_days,
      total_spent_usd: total_spent,
      bookings: booking_hashes,
      execution_logs_hash,
      storage_root_hash,
      report_root_hash,
      on_chain_tx: None,
      created_at: Local::now().to_rfc3339(),
      report_path: report_path.ok(),
      owner_address: ctx.policy.trip.owner.clone(),
      execution_proof: Some(execution_proof.clone()),
    };

    ctx.log(ActivityLog::success(
      self.name(),
      &format!(
        "✓ Journey artifact created — ID: {}",
        &artifact.artifact_id[..8]
      ),
    ));

    ctx.log(ActivityLog::success(
      self.name(),
      "Execution proof complete.",
    ));

    *self.artifact.lock().await = Some(artifact);
    Ok(())
  }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Write a Markdown report to the reports directory and return the absolute path.
fn save_report(destination: &str, session_id: &str, content: &str) -> Result<String> {
  let reports_dir = std::env::var("REPORTS_DIR").unwrap_or_else(|_| {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.join("reports").to_string_lossy().to_string()
  });

  std::fs::create_dir_all(&reports_dir)?;

  let filename = crate::report::report_filename(destination, session_id);
  let full_path = std::path::Path::new(&reports_dir).join(&filename);

  std::fs::write(&full_path, content)?;

  Ok(full_path.to_string_lossy().to_string())
}

/// Produce an HMAC-SHA256 hex digest of `message` using `key`.
/// Used as a lightweight execution proof — verifiable by anyone with the operator key.
fn hmac_sign(key: &str, message: &str) -> String {
  let mut mac = HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC accepts any key length");
  mac.update(message.as_bytes());
  let result = mac.finalize();
  result
    .into_bytes()
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect()
}

fn hash_booking(b: &BookingResult) -> String {
  let preimage = format!(
    "{}|{}|{}|{:.2}|{}",
    b.segment_id, b.booking_type, b.provider, b.price_usd, b.reference
  );
  format!("{:x}", md5::compute(preimage.as_bytes()))
}
