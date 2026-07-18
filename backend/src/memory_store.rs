/*!
MemoryStore — lightweight JSON file storage for OpenWorld session data.

Simple local-disk store for artifacts and reports.
Each artifact is written as a JSON file under `MEMORY_DIR` (default: ./memory).
A content hash (MD5) is returned as the record identifier, allowing
deterministic retrieval even across process restarts.

Alibaba Cloud OSS integration can be layered on top in Phase 1.7+ by
calling `oss_client.put_object()` after the local write.

Env vars:
  MEMORY_DIR — directory for stored JSON files (default: ./memory)
*/

use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::{
  fs,
  path::{Path, PathBuf},
};

// ─── Client ───────────────────────────────────────────────────────────────────

/// Simple JSON-file-backed store for session artifacts and execution logs.
#[derive(Clone, Debug)]
pub struct MemoryStore {
  base_dir: PathBuf,
}

impl MemoryStore {
  /// Create store, ensuring `base_dir` exists on disk.
  pub fn new(base_dir: impl AsRef<Path>) -> Result<Self> {
    let base_dir = base_dir.as_ref().to_path_buf();
    fs::create_dir_all(&base_dir)
      .with_context(|| format!("Failed to create MEMORY_DIR at {:?}", base_dir))?;
    Ok(Self { base_dir })
  }

  /// Serialize `value` to JSON, write to `<base_dir>/<key>.json`, and return
  /// the MD5 hex digest of the JSON bytes as a content hash.
  pub fn store<T: Serialize>(&self, key: &str, value: &T) -> Result<String> {
    let json =
      serde_json::to_string_pretty(value).context("Failed to serialise value for MemoryStore")?;
    self.store_text(key, &json)
  }

  /// Write raw `text` to `<base_dir>/<key>.md` (Markdown) or `<key>.json` and return its MD5 content hash.
  pub fn store_text(&self, key: &str, text: &str) -> Result<String> {
    let safe_key = key.replace(['/', ':', ' ', '\\'], "_");
    // Use .md extension for report keys, .json for everything else
    let ext = if safe_key.starts_with("report_") {
      "md"
    } else {
      "json"
    };
    let path = self.base_dir.join(format!("{safe_key}.{ext}"));
    fs::write(&path, text.as_bytes())
      .with_context(|| format!("Failed to write MemoryStore entry at {:?}", path))?;
    let hash = format!("{:x}", md5::compute(text.as_bytes()));
    Ok(hash)
  }

  /// Deserialize a stored value by its content hash.
  /// Scans all `.json` files under `base_dir` and returns the first whose
  /// MD5 matches `hash`.
  pub fn load<T: DeserializeOwned>(&self, hash: &str) -> Result<T> {
    for entry in fs::read_dir(&self.base_dir)
      .with_context(|| format!("Cannot read MEMORY_DIR {:?}", self.base_dir))?
    {
      let entry = entry?;
      let path = entry.path();
      if !matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("json") | Some("md")
      ) {
        continue;
      }
      let bytes = fs::read(&path)?;
      if format!("{:x}", md5::compute(&bytes)) == hash {
        return serde_json::from_slice(&bytes)
          .with_context(|| format!("Failed to deserialise {:?}", path));
      }
    }
    anyhow::bail!("No MemoryStore entry found for hash {hash}")
  }

  /// Load a stored value by key name (without hash lookup).
  pub fn load_by_key<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
    let safe_key = key.replace(['/', ':', ' ', '\\'], "_");
    let path = self.base_dir.join(format!("{safe_key}.json"));
    let bytes = fs::read(&path)
      .with_context(|| format!("MemoryStore key '{key}' not found at {:?}", path))?;
    serde_json::from_slice(&bytes)
      .with_context(|| format!("Failed to deserialise MemoryStore key '{key}'"))
  }

  /// List all stored keys (filenames without extension).
  pub fn list_keys(&self) -> Result<Vec<String>> {
    let mut keys = Vec::new();
    for entry in fs::read_dir(&self.base_dir)? {
      let entry = entry?;
      let path = entry.path();
      if path.extension().and_then(|e| e.to_str()) == Some("json") {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
          keys.push(stem.to_string());
        }
      }
    }
    Ok(keys)
  }
}

// ─── Builder ──────────────────────────────────────────────────────────────────

/// Build MemoryStore from environment variable MEMORY_DIR (default: ./memory).
pub fn build_memory_store() -> Result<MemoryStore> {
  let dir = std::env::var("MEMORY_DIR").unwrap_or_else(|_| "./memory".to_string());
  MemoryStore::new(dir)
}
