/*!
Alibaba Cloud Log Service (SLS) — real-time structured agent log shipping.

Every ActivityLog entry emitted by the pipeline is forwarded to SLS,
creating a searchable audit trail visible in the SLS console dashboard.

Authentication: SLS v0.6.0 HMAC-SHA1 (base64-encoded).
Body encoding:  minimal inline Protobuf (no external proto deps).
Transport:      fire-and-forget background task — never blocks the pipeline.

Required env vars:
  SLS_ACCESS_KEY_ID     — RAM access key ID (reuse OSS key)
  SLS_ACCESS_KEY_SECRET — RAM access key secret (reuse OSS key)
  SLS_PROJECT           — SLS project name  (e.g. qwenhackkongphop)
  SLS_LOGSTORE          — SLS logstore name (e.g. logkongphop)
Optional:
  SLS_ENDPOINT          — public endpoint hostname
                          (default: ap-southeast-7.log.aliyuncs.com)
                          "-internal" suffix is stripped automatically.
*/

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha1::Sha1;
use std::time::Duration;

type HmacSha1 = Hmac<Sha1>;

// ─── LogService ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct LogService {
  client: Client,
  key_id: String,
  key_secret: String,
  project: String,
  logstore: String,
  /// Bare public hostname, e.g. "ap-southeast-7.log.aliyuncs.com"
  endpoint: String,
}

impl LogService {
  fn new(
    key_id: String,
    key_secret: String,
    project: String,
    logstore: String,
    endpoint: String,
  ) -> Self {
    let client = Client::builder()
      .timeout(Duration::from_secs(10))
      .build()
      .expect("SLS HTTP client");
    Self {
      client,
      key_id,
      key_secret,
      project,
      logstore,
      endpoint,
    }
  }

  /// Ship one log entry to SLS. Non-blocking from the caller's perspective
  /// when called inside a fire-and-forget tokio::spawn.
  pub async fn put_log(
    &self,
    session_id: &str,
    agent: &str,
    level: &str,
    message: &str,
  ) -> Result<()> {
    let now_sec = Utc::now().timestamp() as u32;
    let ts_str = Utc::now().to_rfc3339();

    // Encode as SLS LogGroup protobuf
    let contents: &[(&str, &str)] = &[
      ("session_id", session_id),
      ("agent", agent),
      ("level", level),
      ("message", message),
      ("timestamp", &ts_str),
    ];
    let log_bytes = proto_encode_log(now_sec, contents);
    let group_bytes = proto_encode_log_group(&[log_bytes], "agent-logs", "openworld");

    // SLS v0.6.0 signing
    let body_size = group_bytes.len().to_string();
    let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    let path = format!("/logstores/{}/shards/lb", self.logstore);

    // StringToSign: CanonicalizedSLSHeaders must be sorted alphabetically
    let string_to_sign = format!(
            "POST\n\napplication/x-protobuf\n{date}\nx-log-apiversion:0.6.0\nx-log-bodyrawsize:{body_size}\nx-log-compresstype:none\n{path}",
        );
    let sig = self.hmac_sha1_base64(&string_to_sign)?;
    let auth = format!("LOG {}:{}", self.key_id, sig);

    let url = format!("https://{}.{}{}", self.project, self.endpoint, path);

    let resp = self
      .client
      .post(&url)
      .header("Authorization", &auth)
      .header("Date", &date)
      .header("Content-Type", "application/x-protobuf")
      .header("x-log-apiversion", "0.6.0")
      .header("x-log-bodyrawsize", &body_size)
      .header("x-log-compresstype", "none")
      .body(group_bytes)
      .send()
      .await?;

    let status = resp.status();
    if status.is_success() {
      Ok(())
    } else {
      let text = resp.text().await.unwrap_or_default();
      Err(anyhow!(
        "SLS PutLogs {} — {}",
        status,
        &text[..text.len().min(200)]
      ))
    }
  }

  fn hmac_sha1_base64(&self, message: &str) -> Result<String> {
    let mut mac = HmacSha1::new_from_slice(self.key_secret.as_bytes())
      .map_err(|e| anyhow!("HMAC-SHA1 key error: {}", e))?;
    mac.update(message.as_bytes());
    Ok(general_purpose::STANDARD.encode(mac.finalize().into_bytes()))
  }
}

// ─── Builder ──────────────────────────────────────────────────────────────────

/// Build a LogService from env vars. Returns None if not configured.
/// Pipeline degrades gracefully — local logging still works without SLS.
pub fn build_log_service() -> Option<LogService> {
  let key_id = std::env::var("SLS_ACCESS_KEY_ID").ok()?;
  let key_secret = std::env::var("SLS_ACCESS_KEY_SECRET").ok()?;
  let project = std::env::var("SLS_PROJECT").ok()?;
  let logstore = std::env::var("SLS_LOGSTORE").ok()?;

  if key_id.trim().is_empty() || key_secret.trim().is_empty() {
    return None;
  }

  let endpoint =
    std::env::var("SLS_ENDPOINT").unwrap_or_else(|_| "ap-southeast-7.log.aliyuncs.com".to_string());

  // Strip scheme and "-internal" suffix — internal endpoint only works
  // inside Alibaba Cloud VPC; use public endpoint for local dev.
  let endpoint = endpoint
    .trim_start_matches("https://")
    .trim_start_matches("http://")
    .replace("-internal", "")
    .to_string();

  Some(LogService::new(
    key_id, key_secret, project, logstore, endpoint,
  ))
}

// ─── Minimal inline Protobuf encoder ─────────────────────────────────────────
//
// Encodes Alibaba Cloud SLS LogGroup wire format without external proto crates.
//
// Proto schema (SLS log.proto):
//   message Content  { required string key = 1; required string value = 2; }
//   message Log      { required uint32 time = 1; repeated Content contents = 2; }
//   message LogGroup { repeated Log logs = 1; optional string topic = 4;
//                      optional string source = 5; }

fn varint(mut v: u64) -> Vec<u8> {
  let mut out = Vec::with_capacity(10);
  loop {
    let mut b = (v & 0x7F) as u8;
    v >>= 7;
    if v != 0 {
      b |= 0x80;
    }
    out.push(b);
    if v == 0 {
      break;
    }
  }
  out
}

fn tag(field: u32, wire: u8) -> Vec<u8> {
  varint(((field as u64) << 3) | wire as u64)
}

fn len_delim(field: u32, bytes: &[u8]) -> Vec<u8> {
  let mut out = tag(field, 2);
  out.extend(varint(bytes.len() as u64));
  out.extend_from_slice(bytes);
  out
}

fn string_field(field: u32, s: &str) -> Vec<u8> {
  len_delim(field, s.as_bytes())
}

fn uint32_field(field: u32, v: u32) -> Vec<u8> {
  let mut out = tag(field, 0);
  out.extend(varint(v as u64));
  out
}

fn proto_encode_log(time_sec: u32, contents: &[(&str, &str)]) -> Vec<u8> {
  let mut out = uint32_field(1, time_sec); // Log.time
  for (k, v) in contents {
    // Content sub-message
    let mut content = string_field(1, k); // Content.key
    content.extend(string_field(2, v)); // Content.value
    out.extend(len_delim(2, &content)); // Log.contents
  }
  out
}

fn proto_encode_log_group(logs: &[Vec<u8>], topic: &str, source: &str) -> Vec<u8> {
  let mut out = Vec::new();
  for log in logs {
    out.extend(len_delim(1, log)); // LogGroup.logs
  }
  out.extend(string_field(4, topic)); // LogGroup.topic
  out.extend(string_field(5, source)); // LogGroup.source
  out
}
