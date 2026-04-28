//! # Sudo Timelock Module
//!
//! Implements a decentralized administrator timelock that enforces a mandatory
//! 48-hour execution delay on all state-critical administrative actions.
//!
//! ## Security Model
//! - **Admin**: Must propose actions and wait 48h before execution.
//! - **Security Council**: Can veto any staged action during the delay window.
//! - **Circuit Breaker**: Can immediately pause the protocol without timelock.
//!
//! ## Storage
//! All staged actions are stored in **Persistent** storage to survive
//! contract instance TTL extensions and ensure durability.

use soroban_sdk::{
    contracttype, Address, Env, Symbol,
};

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Mandatory 48-hour delay in seconds (48 × 60 × 60 = 172_800).
pub const TIMELOCK_DELAY_SECONDS: u64 = 172_800;

/// Maximum TTL for staged actions in persistent storage (~30 days in ledgers).
pub const STAGED_ACTION_TTL: u32 = 518_400;

// ─────────────────────────────────────────────────────────────────────────────
// Storage Keys  (all ≤ 9 chars so `Symbol::short` works)
// ─────────────────────────────────────────────────────────────────────────────

/// Persistent storage key prefix for staged actions (keyed by action_id).
pub const STAGED_ACTION: Symbol = Symbol::short("STG_ACT");

/// Instance storage key for the next action ID counter (monotonic u64).
pub const ACTION_COUNTER: Symbol = Symbol::short("ACT_CTR");

/// Instance storage key for the Security Council address.
pub const SECURITY_COUNCIL: Symbol = Symbol::short("SEC_CNC");

/// Instance storage key for the Circuit Breaker address.
pub const CIRCUIT_BREAKER: Symbol = Symbol::short("CIR_BRK");

/// Instance storage key for the global pause flag.
pub const PAUSED: Symbol = Symbol::short("PAUSED");

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// Enumerates all administrative operations that are subject to the timelock.
///
/// Each variant encodes the full payload needed to replay the action at
/// execution time, so no external state is required.
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum AdminActionType {
    /// Pause the entire protocol (when invoked through the timelock path).
    Pause,
    /// Unpause the protocol — always timelocked to prevent instant recovery
    /// after a malicious pause.
    Unpause,
    /// Create a vault with full initialization.
    /// Payload: (owner, amount, start_time, end_time)
    CreateVaultFull(Address, i128, u64, u64),
    /// Create a vault with lazy initialization.
    /// Payload: (owner, amount, start_time, end_time)
    CreateVaultLazy(Address, i128, u64, u64),
    /// Revoke all unreleased tokens from a vault.
    /// Payload: (vault_id)
    RevokeTokens(u64),
    /// Revoke a specific amount from a vault.
    /// Payload: (vault_id, amount)
    RevokePartial(u64, i128),
    /// Transfer vault beneficiary.
    /// Payload: (vault_id, new_address)
    TransferBeneficiary(u64, Address),
    /// Mark a vault as irrevocable.
    /// Payload: (vault_id)
    MarkIrrevocable(u64),
    /// Propose a new admin (first step of two-step admin transfer).
    /// Payload: (new_admin)
    ProposeNewAdmin(Address),
}

/// The status of a staged action in the timelock pipeline.
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum ActionStatus {
    /// Action is staged and waiting for the timelock to expire.
    Pending,
    /// Action was successfully executed after the timelock expired.
    Executed,
    /// Action was vetoed by the Security Council before execution.
    Vetoed,
}

/// A staged administrative action stored in persistent storage.
///
/// Once proposed, the action cannot be modified — only executed (after the
/// delay) or vetoed (by the Security Council).
#[contracttype]
#[derive(Clone)]
pub struct StagedAction {
    /// Unique monotonic identifier.
    pub action_id: u64,
    /// The fully-encoded action payload.
    pub action: AdminActionType,
    /// Address of the admin who proposed this action.
    pub proposer: Address,
    /// Ledger timestamp at which this action was proposed.
    pub proposed_at: u64,
    /// Ledger timestamp after which this action may be executed.
    /// Computed as `proposed_at + TIMELOCK_DELAY_SECONDS`.
    pub execute_after: u64,
    /// Current lifecycle status.
    pub status: ActionStatus,
}

// ─────────────────────────────────────────────────────────────────────────────
// Event Types
// ─────────────────────────────────────────────────────────────────────────────

/// Event data emitted when an admin action is proposed.
#[contracttype]
#[derive(Clone)]
pub struct AdminActionProposed {
    pub action_id: u64,
    pub proposer: Address,
    pub execute_after: u64,
    pub proposed_at: u64,
}

/// Event data emitted when a staged action is executed.
#[contracttype]
#[derive(Clone)]
pub struct AdminActionExecuted {
    pub action_id: u64,
    pub executor: Address,
    pub executed_at: u64,
}

/// Event data emitted when a staged action is vetoed.
#[contracttype]
#[derive(Clone)]
pub struct AdminActionVetoed {
    pub action_id: u64,
    pub vetoed_by: Address,
    pub vetoed_at: u64,
}

/// Event data emitted on emergency pause.
#[contracttype]
#[derive(Clone)]
pub struct EmergencyPauseActivated {
    pub activated_by: Address,
    pub activated_at: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Storage Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Get the next action ID and increment the counter.
pub fn next_action_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&ACTION_COUNTER)
        .unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&ACTION_COUNTER, &next);
    next
}

/// Store a staged action in persistent storage.
pub fn store_staged_action(env: &Env, action: &StagedAction) {
    env.storage()
        .persistent()
        .set(&(STAGED_ACTION, action.action_id), action);
    // Extend TTL to ensure the action survives the full timelock window
    env.storage()
        .persistent()
        .extend_ttl(
            &(STAGED_ACTION, action.action_id),
            STAGED_ACTION_TTL,
            STAGED_ACTION_TTL,
        );
}

/// Load a staged action from persistent storage.
pub fn load_staged_action(env: &Env, action_id: u64) -> StagedAction {
    env.storage()
        .persistent()
        .get(&(STAGED_ACTION, action_id))
        .unwrap_or_else(|| panic!("Staged action not found"))
}

/// Check if the contract is currently paused.
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&PAUSED)
        .unwrap_or(false)
}

/// Set the paused flag.
pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&PAUSED, &paused);
}

/// Get the Security Council address, if set.
pub fn get_security_council(env: &Env) -> Option<Address> {
    env.storage().instance().get(&SECURITY_COUNCIL)
}

/// Set the Security Council address.
pub fn set_security_council_addr(env: &Env, addr: &Address) {
    env.storage().instance().set(&SECURITY_COUNCIL, addr);
}

/// Get the Circuit Breaker address, if set.
pub fn get_circuit_breaker(env: &Env) -> Option<Address> {
    env.storage().instance().get(&CIRCUIT_BREAKER)
}

/// Set the Circuit Breaker address.
pub fn set_circuit_breaker_addr(env: &Env, addr: &Address) {
    env.storage().instance().set(&CIRCUIT_BREAKER, addr);
}

/// Require that the caller is the Security Council.
pub fn require_security_council(env: &Env, caller: &Address) {
    let council: Address = get_security_council(env)
        .unwrap_or_else(|| panic!("Security Council not set"));
    caller.require_auth();
    if *caller != council {
        panic!("Caller is not the Security Council");
    }
}

/// Require that the caller is the Circuit Breaker.
pub fn require_circuit_breaker(env: &Env, caller: &Address) {
    let breaker: Address = get_circuit_breaker(env)
        .unwrap_or_else(|| panic!("Circuit Breaker not set"));
    caller.require_auth();
    if *caller != breaker {
        panic!("Caller is not the Circuit Breaker");
    }
}

/// Require that the contract is not paused. Reverts with a clear message.
pub fn require_not_paused(env: &Env) {
    if is_paused(env) {
        panic!("Contract is paused");
    }
}
