// Issue #145 / #92 — KPI Vesting Gate
// Plugs into VestingContract::claim_tokens as an additional guard.
// All token math stays in lib.rs — this module only enforces the KPI gate.

use soroban_sdk::{contractevent, Address, Env, Symbol};
use crate::kpi_engine::{self, KpiOracleConfig, KpiVerificationRecord};
use crate::oracle::ComparisonOperator;

// ── DataKey variants needed (added to lib.rs DataKey enum separately) ────
// DataKey::KpiConfig(u64)  — stored by kpi_engine via tuple key
// DataKey::KpiMet(u64)     — stored by kpi_engine via tuple key
// DataKey::KpiLog(u64)     — stored by kpi_engine via tuple key

// ── Public API called from lib.rs ─────────────────────────────────────────

/// Attach a KPI gate to an existing vault.
/// Admin-only — enforced by the caller in lib.rs before this is called.
///
/// Example for Twitter 50 k followers:
///   oracle_contract = <your oracle adapter address>
///   metric_id       = symbol_short!("TW_FOLL")
///   threshold       = 50_000
///   operator        = ComparisonOperator::GreaterThanOrEqual
pub fn attach_kpi_gate(
    env: &Env,
    vault_id: u64,
    oracle_contract: Address,
    metric_id: Symbol,
    threshold: i128,
    operator: ComparisonOperator,
) {
    if threshold <= 0 {
        panic!("KPI threshold must be positive");
    }

    // Prevent overwriting a gate that is already met — that would be
    // nonsensical and could confuse indexers.
    if kpi_engine::is_kpi_met(env, vault_id) {
        panic!("Cannot reconfigure a KPI gate that has already been verified");
    }

    let config = KpiOracleConfig {
        oracle_contract,
        metric_id,
        threshold,
        operator,
    };

    kpi_engine::set_kpi_config(env, vault_id, &config);

    KpiSetEvent {
        vault_id,
        threshold,
        timestamp: env.ledger().timestamp(),
    }.publish(env);
}

#[event]
pub struct KpiSetEvent {
    #[topic]
    pub vault_id: u64,
    pub threshold: i128,
    pub timestamp: u64,
}

/// Gate check inserted at the top of claim_tokens / claim_tokens_diversified.
///
/// Returns immediately if no KPI gate is configured (opt-in feature).
/// Panics with a clear message if a gate exists but has not been verified yet.
pub fn require_kpi_gate_passed(env: &Env, vault_id: u64) {
    // No config means no gate — vesting proceeds normally.
    if kpi_engine::get_kpi_config(env, vault_id).is_none() {
        return;
    }

    if !kpi_engine::is_kpi_met(env, vault_id) {
        panic!("KPI gate not yet verified: project has not hit the required growth target");
    }
}

/// Anyone can call this to attempt oracle verification.
/// If the KPI is already met it is a cheap no-op (idempotent fast-path).
/// Returns true if KPI is now met (either just verified or was already met).
pub fn try_verify_kpi(env: &Env, vault_id: u64, caller: &Address) -> bool {
    kpi_engine::verify_kpi(env, vault_id, caller)
}

/// Read-only: is the KPI gate met for this vault?
pub fn kpi_status(env: &Env, vault_id: u64) -> bool {
    kpi_engine::is_kpi_met(env, vault_id)
}

/// Read-only: full verification log for a vault.
pub fn kpi_verification_log(env: &Env, vault_id: u64) -> soroban_sdk::Vec<KpiVerificationRecord> {
    kpi_engine::get_kpi_log(env, vault_id)
}

/// Read-only: returns the configured threshold for a vault, or 0 if none.
pub fn kpi_threshold(env: &Env, vault_id: u64) -> i128 {
    kpi_engine::get_kpi_config(env, vault_id)
        .map(|c| c.threshold)
        .unwrap_or(0)
}
