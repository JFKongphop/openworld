/*!
OSS upload smoke test — run before the full pipeline to verify
credentials and bucket access.

  cargo run --bin oss_test
*/

use openworld::oss_store::build_oss_store;

#[tokio::main]
async fn main() {
  openworld::load_env();

  println!("── OSS Upload Test ─────────────────────────────────────");

  let oss = match build_oss_store() {
    Some(o) => {
      println!("✓  Credentials loaded");
      o
    }
    None => {
      eprintln!("✗  OSS_ACCESS_KEY_ID / OSS_ACCESS_KEY_SECRET not set in .env");
      std::process::exit(1);
    }
  };

  // Upload a small JSON test object
  let payload = serde_json::json!({
      "test": true,
      "message": "OpenWorld OSS connection verified",
      "timestamp": chrono::Local::now().to_rfc3339(),
  });
  let body = serde_json::to_vec_pretty(&payload).unwrap();
  let key = format!("test/smoke-test-{}.json", uuid::Uuid::new_v4());

  println!("·  Uploading → {}", key);
  match oss.put(&key, &body, "application/json").await {
    Ok(url) => {
      println!("✓  Upload succeeded");
      println!("   URL: {}", url);
      println!("────────────────────────────────────────────────────");
      println!("OSS is working. You can now run the full pipeline.");
    }
    Err(e) => {
      eprintln!("✗  Upload failed: {}", e);
      eprintln!("   Check bucket name, region, and key permissions.");
      std::process::exit(1);
    }
  }
}
