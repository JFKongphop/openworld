/*!
OSS fetch test — uploads a file then reads it back to prove
round-trip read/write works.

  cargo run --bin oss_fetch
*/

use openworld::oss_store::build_oss_store;

#[tokio::main]
async fn main() {
  openworld::load_env();

  println!("── OSS Fetch Test ──────────────────────────────────────");

  let oss = match build_oss_store() {
    Some(o) => o,
    None => {
      eprintln!("✗  OSS_ACCESS_KEY_ID / OSS_ACCESS_KEY_SECRET not set in .env");
      std::process::exit(1);
    }
  };

  // 1. Upload a test payload
  let key = "test/fetch-test.json";
  let payload = serde_json::json!({
      "project":   "OpenWorld",
      "service":   "Alibaba Cloud OSS",
      "region":    "Thailand (Bangkok) — ap-southeast-7",
      "status":    "read/write verified",
      "timestamp": chrono::Local::now().to_rfc3339(),
  });
  let body = serde_json::to_vec_pretty(&payload).unwrap();

  println!("·  Uploading → {}", key);
  match oss.put(key, &body, "application/json").await {
    Ok(url) => println!("✓  Uploaded  → {}", url),
    Err(e) => {
      eprintln!("✗  Upload failed: {}", e);
      std::process::exit(1);
    }
  }

  // 2. Fetch it back
  println!("·  Fetching  → {}", key);
  match oss.get(key).await {
    Ok(bytes) => {
      let text = String::from_utf8_lossy(&bytes);
      println!("✓  Fetched {} bytes\n", bytes.len());
      println!("── Content ─────────────────────────────────────────");
      println!("{}", text);
      println!("────────────────────────────────────────────────────");
      println!("OSS round-trip verified ✓");
    }
    Err(e) => {
      eprintln!("✗  Fetch failed: {}", e);
      std::process::exit(1);
    }
  }
}
