/*!
0G Storage Client — persistent memory for the travel orchestration system.

Responsibilities:
  1. Upload travel.md profiles to 0G Storage (reusable across sessions)
  2. Persist orchestration logs and booking artifacts
  3. Store ERC-7857 journey artifact metadata
  4. Download and reconstruct historical execution records

Uses 0g-cli for all upload / download operations.

Env vars:
  OG_INDEXER_RPC      — e.g. https://indexer-storage-turbo.0g.ai
  OG_STORAGE_STREAM_ID — e.g. openworld_travel
  OG_CLI_PATH         — path to 0g-cli binary (default: ./0g-cli)
  OG_MANIFEST_PATH    — path to manifest.json (default: ./manifest.json)
*/

use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;

// ─── Stored Record Types ───────────────────────────────────────────────────────

/// A stored travel.md profile with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredProfile {
  pub profile_id: String,
  pub name: String,
  pub created_at: String,
  pub policy_yaml: String,
  pub root_hash: Option<String>,
}

/// A stored execution log bundle for one session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredExecutionLog {
  pub session_id: String,
  pub destination: String,
  pub started_at: String,
  pub completed_at: Option<String>,
  pub log_entries: Vec<String>,
  pub root_hash: Option<String>,
}

/// A stored journey artifact (ERC-7857 metadata)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredArtifact {
  pub artifact_id: String,
  pub session_id: String,
  pub trip_summary: String,
  pub total_spent: f64,
  pub booking_hashes: Vec<String>,
  pub execution_hash: String,
  pub on_chain_tx: Option<String>,
  pub created_at: String,
  pub root_hash: Option<String>,
}

// ─── Client ───────────────────────────────────────────────────────────────────

/// 0G Storage client — wraps 0g-cli for upload/download
#[derive(Clone)]
pub struct OgStorageClient {
  indexer_rpc: String,
  #[allow(dead_code)]
  stream_id: String,
  cli_path: String,
  #[allow(dead_code)]
  manifest_path: String,
}

impl OgStorageClient {
  pub fn new(
    indexer_rpc: String,
    stream_id: String,
    cli_path: String,
    manifest_path: String,
  ) -> Self {
    Self {
      indexer_rpc,
      stream_id,
      cli_path,
      manifest_path,
    }
  }

  /// Upload a serialisable value to 0G Storage.
  /// Streams 0g-cli stderr line-by-line so progress is visible, then extracts
  /// the Merkle root hash from the output.  Even on a non-zero exit (e.g. the
  /// node confirms the upload but times out waiting for finality) the root is
  /// recovered from the log, so the caller always gets a usable hash.
  pub async fn upload<T: Serialize>(&self, key: &str, value: &T) -> Result<String> {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};

    let json = serde_json::to_string(value).context("Failed to serialise data for 0G Storage")?;
    let encoded = general_purpose::STANDARD.encode(json.as_bytes());

    let safe_key = key.replace('/', "_").replace(':', "_");
    let temp_path = format!(
      "/tmp/ow_upload_{}_{}.dat",
      safe_key,
      chrono::Local::now().timestamp_millis()
    );
    fs::write(&temp_path, encoded.as_bytes())
      .context("Failed to write temp upload file")?;

    let private_key = std::env::var("OPERATOR_PRIVATE_KEY")
      .unwrap_or_default()
      .trim_start_matches("0x")
      .to_string();
    let rpc_url = std::env::var("OG_RPC_URL")
      .unwrap_or_else(|_| "https://evmrpc.0g.ai".to_string());

    if private_key.is_empty() {
      let _ = fs::remove_file(&temp_path);
      anyhow::bail!("OPERATOR_PRIVATE_KEY not set — cannot upload to 0G Storage");
    }

    let mut child = tokio::process::Command::new(&self.cli_path)
      .args([
        "upload",
        "--url",      &rpc_url,
        "--indexer",  &self.indexer_rpc,
        "--key",      &private_key,
        "--file",     &temp_path,
        "--expected-replica", "1",
      ])
      .stderr(Stdio::piped())
      .stdout(Stdio::piped())
      .spawn()
      .context("Failed to spawn 0g-cli upload")?;

    // Stream stderr in real time so upload progress is visible
    let stderr_pipe = child.stderr.take().unwrap();
    let mut reader = BufReader::new(stderr_pipe).lines();
    let mut all_output = String::new();
    while let Ok(Some(line)) = reader.next_line().await {
      println!("    [0G] {}", line);
      all_output.push_str(&line);
      all_output.push('\n');
    }

    let status = child.wait().await.context("0g-cli process error")?;
    let _ = fs::remove_file(&temp_path);

    // Root hash extraction — same strategy as backend-style:
    // 1. Prefer lines that mention "merkle", "root=" or "root ="
    // 2. Fall back to any 64-char hex in the full output
    let hex_re = Regex::new(r"0x([0-9a-fA-F]{64})").unwrap();
    let extract_root = |text: &str| -> Option<String> {
      // First pass: lines explicitly about the Merkle root
      text.lines()
        .find(|l| {
          let lo = l.to_lowercase();
          lo.contains("merkle") || lo.contains("root=") || lo.contains("root =")
        })
        .and_then(|l| hex_re.captures(l).map(|c| format!("0x{}", &c[1])))
        // Second pass: any 64-hex token anywhere in output
        .or_else(|| {
          hex_re.captures_iter(text)
            .map(|c| format!("0x{}", &c[1]))
            .last()
        })
    };

    if status.success() {
      match extract_root(&all_output) {
        Some(root) => {
          println!("    [0G] Root hash: {}", root);
          Ok(root)
        }
        None => {
          anyhow::bail!("0g-cli upload succeeded but no root hash found in output")
        }
      }
    } else {
      // Even on failure the CLI may have computed + logged the Merkle root
      // before the error (e.g. finality timeout after the data was already sent)
      match extract_root(&all_output) {
        Some(root) => {
          println!("    [0G] Upload finished with warnings — root hash recovered: {}", root);
          Ok(root)
        }
        None => {
          anyhow::bail!("0g-cli upload failed and no root hash recovered from output")
        }
      }
    }
  }

  /// Upload raw text (e.g. a Markdown report) to 0G Storage.
  /// The text is base64-encoded and stored as-is (no JSON wrapper).
  pub async fn upload_text(&self, key: &str, text: &str) -> Result<String> {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};

    let encoded = general_purpose::STANDARD.encode(text.as_bytes());
    let safe_key = key.replace('/', "_").replace(':', "_");
    let temp_path = format!(
      "/tmp/ow_upload_{}_{}.dat",
      safe_key,
      chrono::Local::now().timestamp_millis()
    );
    fs::write(&temp_path, encoded.as_bytes())
      .context("Failed to write temp upload file")?;

    let private_key = std::env::var("OPERATOR_PRIVATE_KEY")
      .unwrap_or_default()
      .trim_start_matches("0x")
      .to_string();
    let rpc_url = std::env::var("OG_RPC_URL")
      .unwrap_or_else(|_| "https://evmrpc.0g.ai".to_string());

    if private_key.is_empty() {
      let _ = fs::remove_file(&temp_path);
      anyhow::bail!("OPERATOR_PRIVATE_KEY not set — cannot upload to 0G Storage");
    }

    let mut child = tokio::process::Command::new(&self.cli_path)
      .args([
        "upload",
        "--url",      &rpc_url,
        "--indexer",  &self.indexer_rpc,
        "--key",      &private_key,
        "--file",     &temp_path,
        "--expected-replica", "1",
      ])
      .stderr(Stdio::piped())
      .stdout(Stdio::piped())
      .spawn()
      .context("Failed to spawn 0g-cli upload")?;

    let stderr_pipe = child.stderr.take().unwrap();
    let mut reader = BufReader::new(stderr_pipe).lines();
    let mut all_output = String::new();
    while let Ok(Some(line)) = reader.next_line().await {
      println!("    [0G] {}", line);
      all_output.push_str(&line);
      all_output.push('\n');
    }

    let status = child.wait().await.context("0g-cli process error")?;
    let _ = fs::remove_file(&temp_path);

    let hex_re = Regex::new(r"0x([0-9a-fA-F]{64})").unwrap();
    let extract_root = |text: &str| -> Option<String> {
      text.lines()
        .find(|l| {
          let lo = l.to_lowercase();
          lo.contains("merkle") || lo.contains("root=") || lo.contains("root =")
        })
        .and_then(|l| hex_re.captures(l).map(|c| format!("0x{}", &c[1])))
        .or_else(|| {
          hex_re.captures_iter(text)
            .map(|c| format!("0x{}", &c[1]))
            .last()
        })
    };

    if status.success() {
      match extract_root(&all_output) {
        Some(root) => { println!("    [0G] Root hash: {}", root); Ok(root) }
        None => anyhow::bail!("0g-cli upload succeeded but no root hash found"),
      }
    } else {
      match extract_root(&all_output) {
        Some(root) => { println!("    [0G] Upload finished with warnings — root hash recovered: {}", root); Ok(root) }
        None => anyhow::bail!("0g-cli upload failed and no root hash recovered"),
      }
    }
  }

  /// Download a stored JSON record by root hash and deserialise it.
  pub async fn download<T: for<'de> Deserialize<'de>>(
    &self,
    root_hash: &str,
  ) -> Result<T> {
    let temp_path = format!(
      "/tmp/ow_download_{}_{}.bin",
      root_hash.trim_start_matches("0x").chars().take(8).collect::<String>(),
      chrono::Local::now().timestamp_millis()
    );

    let output = tokio::process::Command::new(&self.cli_path)
      .args([
        "download",
        "--indexer",
        &self.indexer_rpc,
        "--root",
        root_hash,
        "--file",
        &temp_path,
      ])
      .output()
      .await
      .context("Failed to execute 0g-cli download")?;

    if !output.status.success() {
      let err = String::from_utf8_lossy(&output.stderr);
      anyhow::bail!("0g-cli download failed: {}", err.trim());
    }

    let raw = fs::read(&temp_path).context("Failed to read downloaded file")?;
    let _ = fs::remove_file(&temp_path);

    let decoded = general_purpose::STANDARD
      .decode(&raw)
      .context("Base64 decode failed")?;
    let json = String::from_utf8(decoded).context("UTF-8 decode failed")?;
    serde_json::from_str(&json).context("JSON deserialise failed")
  }
}

// ─── Builder ──────────────────────────────────────────────────────────────────

/// Build OgStorageClient from environment variables
pub fn build_og_storage() -> OgStorageClient {
  OgStorageClient::new(
    std::env::var("OG_INDEXER_RPC")
      .unwrap_or_else(|_| "https://indexer-storage-turbo.0g.ai".to_string()),
    std::env::var("OG_STORAGE_STREAM_ID")
      .unwrap_or_else(|_| "openworld_travel".to_string()),
    std::env::var("OG_CLI_PATH").unwrap_or_else(|_| "./0g-cli".to_string()),
    std::env::var("OG_MANIFEST_PATH").unwrap_or_else(|_| "./manifest.json".to_string()),
  )
}
