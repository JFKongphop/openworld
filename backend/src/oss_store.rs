/*!
Alibaba Cloud OSS Store — uploads artifact JSON and Markdown reports
to the qwenhackkongphop OSS bucket (Bangkok, ap-southeast-7).

Authentication: OSS V1 HMAC-SHA1 signing (per OSS REST API spec).

Required env vars:
  OSS_ACCESS_KEY_ID     — RAM access key ID
  OSS_ACCESS_KEY_SECRET — RAM access key secret
Optional env vars:
  OSS_BUCKET   — bucket name (default: qwenhackkongphop)
  OSS_ENDPOINT — region endpoint hostname (default: oss-ap-southeast-7.aliyuncs.com)
*/

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha1::Sha1;
use std::time::Duration;

type HmacSha1 = Hmac<Sha1>;

// ─── OssStore ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct OssStore {
  client: Client,
  access_key_id: String,
  access_key_secret: String,
  bucket: String,
  /// Bare hostname, e.g. "oss-ap-southeast-7.aliyuncs.com"
  endpoint: String,
}

impl OssStore {
  fn new(
    access_key_id: String,
    access_key_secret: String,
    bucket: String,
    endpoint: String,
  ) -> Self {
    let client = Client::builder()
      .timeout(Duration::from_secs(30))
      .build()
      .expect("OSS HTTP client");
    Self {
      client,
      access_key_id,
      access_key_secret,
      bucket,
      endpoint,
    }
  }

  /// Upload `body` to OSS at `key` (e.g. "artifacts/abc123.json").
  /// Returns the virtual-hosted HTTPS URL on success.
  pub async fn put(&self, key: &str, body: &[u8], content_type: &str) -> Result<String> {
    // RFC 7231 date required by OSS V1 signing
    let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();

    // OSS V1 StringToSign:
    //   VERB\nContent-MD5\nContent-Type\nDate\nCanonicalizedOSSHeaders\nCanonicalizedResource
    // Content-MD5 is optional — leave blank (two adjacent newlines)
    let string_to_sign = format!(
      "PUT\n\n{content_type}\n{date}\n/{bucket}/{key}",
      content_type = content_type,
      date = date,
      bucket = self.bucket,
      key = key,
    );
    let signature = self.hmac_sha1_base64(&string_to_sign)?;
    let auth = format!("OSS {}:{}", self.access_key_id, signature);

    // Virtual-hosted style URL (required for most OSS regions)
    let url = format!(
      "https://{bucket}.{endpoint}/{key}",
      bucket = self.bucket,
      endpoint = self.endpoint,
      key = key,
    );

    let resp = self
      .client
      .put(&url)
      .header("Authorization", &auth)
      .header("Date", &date)
      .header("Content-Type", content_type)
      .body(body.to_vec())
      .send()
      .await?;

    let status = resp.status();
    if status.is_success() {
      Ok(url)
    } else {
      let text = resp.text().await.unwrap_or_default();
      let preview = &text[..text.len().min(300)];
      Err(anyhow!("OSS PUT {} — {}", status, preview))
    }
  }

  /// Download an object from OSS by key. Returns the raw bytes.
  pub async fn get(&self, key: &str) -> Result<Vec<u8>> {
    let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();

    let string_to_sign = format!(
      "GET\n\n\n{date}\n/{bucket}/{key}",
      date = date,
      bucket = self.bucket,
      key = key,
    );
    let signature = self.hmac_sha1_base64(&string_to_sign)?;
    let auth = format!("OSS {}:{}", self.access_key_id, signature);

    let url = format!(
      "https://{bucket}.{endpoint}/{key}",
      bucket = self.bucket,
      endpoint = self.endpoint,
      key = key,
    );

    let resp = self
      .client
      .get(&url)
      .header("Authorization", &auth)
      .header("Date", &date)
      .send()
      .await?;

    let status = resp.status();
    if status.is_success() {
      Ok(resp.bytes().await?.to_vec())
    } else {
      let text = resp.text().await.unwrap_or_default();
      Err(anyhow!(
        "OSS GET {} — {}",
        status,
        &text[..text.len().min(300)]
      ))
    }
  }

  fn hmac_sha1_base64(&self, message: &str) -> Result<String> {
    let mut mac = HmacSha1::new_from_slice(self.access_key_secret.as_bytes())
      .map_err(|e| anyhow!("HMAC-SHA1 key error: {}", e))?;
    mac.update(message.as_bytes());
    Ok(general_purpose::STANDARD.encode(mac.finalize().into_bytes()))
  }
}

// ─── Builder ──────────────────────────────────────────────────────────────────

/// Build an OssStore from environment variables.
/// Returns None if OSS_ACCESS_KEY_ID or OSS_ACCESS_KEY_SECRET are not set.
/// The pipeline degrades gracefully — local storage still works without OSS.
pub fn build_oss_store() -> Option<OssStore> {
  let key_id = std::env::var("OSS_ACCESS_KEY_ID").ok()?;
  let key_secret = std::env::var("OSS_ACCESS_KEY_SECRET").ok()?;

  if key_id.trim().is_empty() || key_secret.trim().is_empty() {
    return None;
  }

  let bucket = std::env::var("OSS_BUCKET").unwrap_or_else(|_| "qwenhackkongphop".to_string());

  let endpoint =
    std::env::var("OSS_ENDPOINT").unwrap_or_else(|_| "oss-ap-southeast-7.aliyuncs.com".to_string());

  // Strip any scheme prefix the user may have included
  let endpoint = endpoint
    .trim_start_matches("https://")
    .trim_start_matches("http://")
    .to_string();

  Some(OssStore::new(key_id, key_secret, bucket, endpoint))
}
