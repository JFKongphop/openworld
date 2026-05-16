/*!
ERC-7857 Journey Artifact — on-chain minting of verifiable travel execution proofs.

Mints a journey artifact on the 0G Mainnet as proof of autonomous execution.
The artifact metadata (itinerary hash, booking refs, execution logs) is first
persisted to 0G Storage; the on-chain record then references that root hash.

Env vars:
  JOURNEY_CONTRACT_ADDRESS — deployed OpenWorldJourney contract (0x...)
  OPERATOR_PRIVATE_KEY     — agent wallet (0x...)
  OG_RPC_URL               — 0G Mainnet RPC (default: https://evmrpc.0g.ai)
*/

use anyhow::{Context, Result};
use ethers::{
  abi::{self, Token},
  middleware::SignerMiddleware,
  providers::{Http, Middleware, Provider},
  signers::{LocalWallet, Signer},
  types::{Address, Bytes, TransactionRequest},
};
use std::sync::Arc;

use crate::agents::JourneyArtifact;

// md5 is used for the report_hash fallback when 0G Storage upload hasn't happened
#[allow(unused_imports)]
use md5;

// ─── Utility ──────────────────────────────────────────────────────────────────

/// Decode a 0x-prefixed hex string into a fixed 32-byte array (zero-left-padded)
pub fn hex_to_bytes32(s: &str) -> Result<[u8; 32]> {
  let hex = s.trim_start_matches("0x");
  let padded = format!("{:0>64}", hex);
  let bytes = hex::decode(&padded[..64])
    .map_err(|e| anyhow::anyhow!("hex decode failed for '{}': {}", s, e))?;
  let mut arr = [0u8; 32];
  arr.copy_from_slice(&bytes);
  Ok(arr)
}

// ─── Client builder ───────────────────────────────────────────────────────────

type SignedProvider = SignerMiddleware<Provider<Http>, LocalWallet>;

fn build_client() -> Result<(Arc<SignedProvider>, Address)> {
  let contract_addr: Address = std::env::var("JOURNEY_CONTRACT_ADDRESS")
    .context("JOURNEY_CONTRACT_ADDRESS not set")?
    .parse()
    .context("Invalid JOURNEY_CONTRACT_ADDRESS")?;

  let private_key =
    std::env::var("OPERATOR_PRIVATE_KEY").context("OPERATOR_PRIVATE_KEY not set")?;

  let rpc_url =
    std::env::var("OG_RPC_URL").unwrap_or_else(|_| "https://evmrpc.0g.ai".to_string());

  let provider = Provider::<Http>::try_from(rpc_url.as_str())
    .context("Failed to create 0G provider")?;

  let wallet: LocalWallet = private_key
    .trim_start_matches("0x")
    .parse::<LocalWallet>()
    .context("Invalid OPERATOR_PRIVATE_KEY")?
    .with_chain_id(16602u64); // 0G Testnet chain ID

  let client = Arc::new(SignerMiddleware::new(provider, wallet));
  Ok((client, contract_addr))
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Mint a journey artifact on-chain.
///
/// Calls `mintJourneyArtifact(string sessionId, bytes32 executionHash, string storageRootHash)`
/// Returns the transaction hash.
pub async fn mint_journey_artifact(artifact: &JourneyArtifact) -> Result<String> {
  let (client, contract_addr) = build_client()?;

  println!(
    "\x1b[35m[ERC-7857]\x1b[0m  Minting journey artifact on 0G Testnet (OpenWorldJourney)..."
  );
  println!("\x1b[2m  Session:    {}\x1b[0m", &artifact.session_id[..8]);
  println!("\x1b[2m  Contract:   {}\x1b[0m", contract_addr);

  // mintAndRecord(address,string,string,bytes32,bytes32)
  // owner is the trip owner wallet (from trip.md); falls back to operator wallet
  let selector = &ethers::utils::keccak256(
    b"mintAndRecord(address,string,string,bytes32,bytes32)",
  )[..4];

  let owner_address: Address = artifact
    .owner_address
    .as_deref()
    .and_then(|s| s.parse().ok())
    .unwrap_or_else(|| {
      // fallback: derive from operator private key
      std::env::var("OPERATOR_PRIVATE_KEY")
        .ok()
        .and_then(|k| k.trim_start_matches("0x").parse::<LocalWallet>().ok())
        .map(|w| w.address())
        .unwrap_or(Address::zero())
    });

  // Collection 1 — memoryHash: 0G Storage root of the uploaded artifact JSON
  // If the 0G upload succeeded, storage_root_hash has the real root.
  // Fallback: MD5 of the execution context (always available).
  let memory_hash_src = artifact
    .storage_root_hash
    .clone()
    .unwrap_or_else(|| artifact.execution_logs_hash.clone());
  let memory_hash = hex_to_bytes32(&memory_hash_src).unwrap_or_else(|_| {
    let digest = md5::compute(memory_hash_src.as_bytes());
    let mut arr = [0u8; 32];
    arr[16..].copy_from_slice(&digest.0);
    arr
  });

  // Collection 2 — reportHash: 0G Storage root of the uploaded Markdown report.
  // Falls back to MD5 of execution_logs_hash if the report upload didn't happen.
  let report_hash_src = artifact
    .report_root_hash
    .clone()
    .unwrap_or_else(|| artifact.execution_logs_hash.clone());
  let report_hash = hex_to_bytes32(&report_hash_src).unwrap_or_else(|_| {
    let digest = md5::compute(report_hash_src.as_bytes());
    let mut arr = [0u8; 32];
    arr[16..].copy_from_slice(&digest.0);
    arr
  });

  let params = abi::encode(&[
    Token::Address(owner_address),
    Token::String(artifact.session_id.clone()),
    Token::String(artifact.trip_summary.clone()),
    Token::FixedBytes(memory_hash.to_vec()),
    Token::FixedBytes(report_hash.to_vec()),
  ]);

  let mut calldata = selector.to_vec();
  calldata.extend_from_slice(&params);

  let tx = TransactionRequest::new()
    .to(contract_addr)
    .data(Bytes::from(calldata))
    .gas(700_000u64)
    .gas_price(3_000_000_000u64)
    .chain_id(16602u64);

  let pending = client
    .send_transaction(tx, None)
    .await
    .context("mintAndRecord tx failed")?;

  let receipt = pending
    .await
    .context("Failed to await tx confirmation")?
    .ok_or_else(|| anyhow::anyhow!("No receipt — tx may have been dropped"))?;

  let tx_hash = format!("{:#x}", receipt.transaction_hash);
  let mem_hex = format!("0x{}", hex::encode(memory_hash));
  let rep_hex = format!("0x{}", hex::encode(report_hash));

  println!("\x1b[32m[ERC-7857]\x1b[0m  ✓ Journey minted on 0G Testnet");
  println!("\x1b[2m  Contract:         {}\x1b[0m", contract_addr);
  println!("\x1b[32m  Tx Hash:          {}\x1b[0m", tx_hash);
  println!("\x1b[36m  Collection 1 (0G Artifact root):        {}\x1b[0m", mem_hex);
  println!("\x1b[36m  Collection 2 (0G Report root):          {}\x1b[0m", rep_hex);
  println!("\x1b[2m  Explorer:         https://chainscan-galileo.0g.ai/tx/{}\x1b[0m", tx_hash);

  Ok(tx_hash)
}
