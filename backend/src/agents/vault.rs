/*!
VaultAgent — budget enforcement and payment authorisation.

Responsibilities:
  - Enforces travel.md vault rules before any booking is executed
  - Tracks cumulative spend against budget_max
  - Rejects transactions that exceed max_single_transaction
  - Logs every approval and rejection with remaining balance
*/

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{ActivityLog, Agent, BookingResult, BookingStatus, ExecutionContext};

// ─── VaultState ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VaultState {
  pub budget_total: f64,
  pub spent: f64,
  pub transactions: Vec<VaultTransaction>,
}

impl VaultState {
  pub fn remaining(&self) -> f64 {
    self.budget_total - self.spent
  }
}

#[derive(Debug, Clone)]
pub struct VaultTransaction {
  pub segment_id: String,
  pub provider: String,
  pub amount: f64,
  pub approved: bool,
  pub reason: String,
}

// ─── VaultAgent ───────────────────────────────────────────────────────────────

pub struct VaultAgent {
  bookings: Arc<Mutex<Vec<BookingResult>>>,
  pub state: Arc<Mutex<VaultState>>,
}

impl VaultAgent {
  pub fn new(bookings: Arc<Mutex<Vec<BookingResult>>>, budget: f64) -> Self {
    Self {
      bookings,
      state: Arc::new(Mutex::new(VaultState {
        budget_total: budget,
        spent: 0.0,
        transactions: Vec::new(),
      })),
    }
  }
}

#[async_trait]
impl Agent for VaultAgent {
  fn name(&self) -> &str {
    "VaultAgent"
  }

  async fn run(&self, ctx: &ExecutionContext) -> Result<()> {
    if !ctx.policy.vault.auto_payment {
      ctx.log(ActivityLog::info(
        self.name(),
        "auto_payment = false — vault approval skipped",
      ));
      return Ok(());
    }

    ctx.log(ActivityLog::action(
      self.name(),
      &format!(
        "Checking budget — total: {} USD",
        ctx.policy.trip.budget_max as u64
      ),
    ));

    let bookings = self.bookings.lock().await.clone();
    let mut vault = self.state.lock().await;

    vault.budget_total = ctx.policy.trip.budget_max;
    vault.spent = 0.0;
    vault.transactions.clear();

    for booking in &bookings {
      if booking.status != BookingStatus::Confirmed {
        continue;
      }

      let amount = booking.price_usd;

      // Rule 1: single transaction cap
      if amount > ctx.policy.vault.max_single_transaction {
        ctx.log(ActivityLog::warn(
          self.name(),
          &format!(
            "⚠ {} ({:.0} USD) exceeds single-transaction limit ({:.0} USD)",
            booking.provider, amount, ctx.policy.vault.max_single_transaction
          ),
        ));
        vault.transactions.push(VaultTransaction {
          segment_id: booking.segment_id.clone(),
          provider: booking.provider.clone(),
          amount,
          approved: false,
          reason: format!(
            "Exceeds max_single_transaction ({:.0} USD)",
            ctx.policy.vault.max_single_transaction
          ),
        });
        continue;
      }

      // Rule 2: total budget cap
      if vault.spent + amount > vault.budget_total {
        ctx.log(ActivityLog::warn(
          self.name(),
          &format!(
            "⚠ {} ({:.0} USD) would exceed budget — remaining: {:.0} USD",
            booking.provider,
            amount,
            vault.remaining()
          ),
        ));
        vault.transactions.push(VaultTransaction {
          segment_id: booking.segment_id.clone(),
          provider: booking.provider.clone(),
          amount,
          approved: false,
          reason: "Exceeds remaining budget".to_string(),
        });
        continue;
      }

      // Approved
      vault.spent += amount;
      vault.transactions.push(VaultTransaction {
        segment_id: booking.segment_id.clone(),
        provider: booking.provider.clone(),
        amount,
        approved: true,
        reason: "Within budget and transaction limits".to_string(),
      });

      ctx.log(ActivityLog::success(
        self.name(),
        &format!(
          "✓ Payment approved — {} {:.0} USD  |  Remaining: {:.0} USD",
          booking.provider,
          amount,
          vault.remaining()
        ),
      ));
    }

    let approved_count = vault.transactions.iter().filter(|t| t.approved).count();
    let rejected_count = vault.transactions.iter().filter(|t| !t.approved).count();

    ctx.log(ActivityLog::info(
      self.name(),
      &format!(
        "Vault summary — approved: {}, rejected: {}, spent: {:.0} / {:.0} USD",
        approved_count, rejected_count, vault.spent, vault.budget_total
      ),
    ));

    Ok(())
  }
}
