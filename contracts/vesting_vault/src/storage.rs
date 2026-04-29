use soroban_sdk::{Env, Vec, Address, Map, BytesN};
use crate::types::{ClaimEvent, AuthorizedPayoutAddress, AddressWhitelistRequest, Nullifier, Commitment, PathPaymentConfig, PathPaymentClaimEvent, LockupConfig, BeneficiaryReassignment, VetoVote, TokenSupplyInfo, LSTConfig, TvlCapConfig, RateLimitConfig, RelayerConfig, ConfidentialGrant, MasterViewingKey, StreamPause};

pub const CLAIM_HISTORY: &str = "CLAIM_HISTORY";
pub const AUTHORIZED_PAYOUT_ADDRESS: &str = "AUTHORIZED_PAYOUT_ADDRESS";
pub const PENDING_ADDRESS_REQUEST: &str = "PENDING_ADDRESS_REQUEST";

// Stream pause storage keys for suspicious activity detection
pub const STREAM_PAUSES: &str = "STREAM_PAUSES";
pub const STREAM_PAUSE_HISTORY: &str = "STREAM_PAUSE_HISTORY";

// Emergency pause storage keys
pub const AUDITORS: &str = "AUDITORS";
pub const AUDITOR_PAUSE_REQUESTS: &str = "AUDITOR_PAUSE_REQUESTS";
pub const EMERGENCY_PAUSE: &str = "EMERGENCY_PAUSE";

// Cross-project reputation storage keys
pub const REPUTATION_BRIDGE_CONTRACT: &str = "REPUTATION_BRIDGE_CONTRACT";
pub const REPUTATION_BONUS_APPLIED: &str = "REPUTATION_BONUS_APPLIED";

// Milestone vesting storage keys
pub const MILESTONE_CONFIGS: &str = "MILESTONE_CONFIGS";
pub const MILESTONE_STATUS: &str = "MILESTONE_STATUS";

// Zero-Knowledge Privacy Claims storage keys
pub const NULLIFIER_MAP: &str = "NULLIFIER_MAP";
pub const COMMITMENT_STORAGE: &str = "COMMITMENT_STORAGE";
pub const PRIVACY_CLAIM_HISTORY: &str = "PRIVACY_CLAIM_HISTORY";
pub const MERKLE_ROOTS: &str = "MERKLE_ROOTS";

// Stellar Horizon Path Payment Claim storage keys
pub const PATH_PAYMENT_CONFIG: &str = "PATH_PAYMENT_CONFIG";
pub const PATH_PAYMENT_CLAIM_HISTORY: &str = "PATH_PAYMENT_CLAIM_HISTORY";

// Lock-up period storage keys
pub const LOCKUP_CONFIGS: &str = "LOCKUP_CONFIGS";

// Beneficiary reassignment and governance veto storage keys
pub const BENEFICIARY_REASSIGNMENTS: &str = "BENEFICIARY_REASSIGNMENTS";
pub const VETO_VOTES: &str = "VETO_VOTES";
pub const TOKEN_SUPPLY_INFO: &str = "TOKEN_SUPPLY_INFO";
pub const REASSIGNMENT_COUNTER: &str = "REASSIGNMENT_COUNTER";
pub const GOVERNANCE_VETO_THRESHOLD: &str = "GOVERNANCE_VETO_THRESHOLD"; // Percentage (e.g., 5 for 5%)
pub const LST_CONFIGS: &str = "LST_CONFIGS";

// LST Auto-Compounding storage keys (Issue #154)
pub const LST_POOL_SHARES: &str = "LST_POOL_SHARES";
pub const USER_LST_SHARES: &str = "USER_LST_SHARES";
pub const UNBONDING_REQUESTS: &str = "UNBONDING_REQUESTS";
pub const UNBONDING_QUEUE: &str = "UNBONDING_QUEUE";

// Protocol Sunset and Migration storage keys (Issue #280)
pub const PROTOCOL_SUNSET: &str = "PROTOCOL_SUNSET";
pub const MIGRATION_PAYLOADS: &str = "MIGRATION_PAYLOADS";
pub const RELAYER_MIGRATIONS: &str = "RELAYER_MIGRATIONS";

// 48 hours in seconds
const TIMELOCK_DURATION: u64 = 172_800;

// 7 days in seconds for emergency pause
const EMERGENCY_PAUSE_DURATION: u64 = 604_800;

pub fn get_claim_history(e: &Env) -> Vec<ClaimEvent> {
    e.storage()
        .instance()
        .get(&CLAIM_HISTORY)
        .unwrap_or(Vec::new(e))
}

pub fn set_claim_history(e: &Env, history: &Vec<ClaimEvent>) {
    e.storage().instance().set(&CLAIM_HISTORY, history);
}

pub fn get_authorized_payout_address(e: &Env, beneficiary: &Address) -> Option<AuthorizedPayoutAddress> {
    e.storage()
        .instance()
        .get(&(AUTHORIZED_PAYOUT_ADDRESS, beneficiary))
}

pub fn set_authorized_payout_address(e: &Env, beneficiary: &Address, auth_address: &AuthorizedPayoutAddress) {
    e.storage().instance().set(&(AUTHORIZED_PAYOUT_ADDRESS, beneficiary), auth_address);
}

pub fn get_pending_address_request(e: &Env, beneficiary: &Address) -> Option<AddressWhitelistRequest> {
    e.storage()
        .instance()
        .get(& (PENDING_ADDRESS_REQUEST, beneficiary))
}

pub fn set_pending_address_request(e: &Env, beneficiary: &Address, request: &AddressWhitelistRequest) {
    e.storage().instance().set(&(PENDING_ADDRESS_REQUEST, beneficiary), request);
}

pub fn remove_pending_address_request(e: &Env, beneficiary: &Address) {
    e.storage().instance().remove(&(PENDING_ADDRESS_REQUEST, beneficiary));
}

pub fn get_timelock_duration() -> u64 {
    TIMELOCK_DURATION
}

// Emergency pause functions
pub fn get_auditors(e: &Env) -> Vec<Address> {
    e.storage()
        .instance()
        .get(&AUDITORS)
        .unwrap_or(Vec::new(e))
}

pub fn set_auditors(e: &Env, auditors: &Vec<Address>) {
    e.storage().instance().set(&AUDITORS, auditors);
}

pub fn get_auditor_pause_requests(e: &Env) -> Map<Address, crate::emergency::AuditorPauseRequest> {
    e.storage()
        .instance()
        .get(&AUDITOR_PAUSE_REQUESTS)
        .unwrap_or(Map::new(e))
}

pub fn set_auditor_pause_requests(e: &Env, requests: &Map<Address, crate::emergency::AuditorPauseRequest>) {
    e.storage().instance().set(&AUDITOR_PAUSE_REQUESTS, requests);
}

pub fn get_emergency_pause(e: &Env) -> Option<crate::emergency::EmergencyPause> {
    e.storage().instance().get(&EMERGENCY_PAUSE)
}

pub fn set_emergency_pause(e: &Env, pause: &crate::emergency::EmergencyPause) {
    e.storage().instance().set(&EMERGENCY_PAUSE, pause);
}

pub fn remove_emergency_pause(e: &Env) {
    e.storage().instance().remove(&EMERGENCY_PAUSE);
}

// Cross-project reputation functions
pub fn get_reputation_bridge_contract(e: &Env) -> Option<Address> {
    e.storage().instance().get(&REPUTATION_BRIDGE_CONTRACT)
}

pub fn set_reputation_bridge_contract(e: &Env, contract_address: &Address) {
    e.storage().instance().set(&REPUTATION_BRIDGE_CONTRACT, contract_address);
}

pub fn has_reputation_bonus_applied(e: &Env, beneficiary: &Address) -> bool {
    e.storage()
        .instance()
        .get(&(REPUTATION_BONUS_APPLIED, beneficiary))
        .unwrap_or(false)
}

pub fn set_reputation_bonus_applied(e: &Env, beneficiary: &Address) {
    e.storage().instance().set(&(REPUTATION_BONUS_APPLIED, beneficiary), &true);
}

// Milestone vesting functions
pub fn get_milestone_configs(e: &Env, vesting_id: u32) -> Option<Vec<u32>> {
    e.storage().instance().get(&(MILESTONE_CONFIGS, vesting_id))
}

pub fn set_milestone_configs(e: &Env, vesting_id: u32, milestones: &Vec<u32>) {
    e.storage().instance().set(&(MILESTONE_CONFIGS, vesting_id), milestones);
}

pub fn get_milestone_status(e: &Env, vesting_id: u32) -> Map<u32, bool> {
    e.storage()
        .instance()
        .get(&(MILESTONE_STATUS, vesting_id))
        .unwrap_or(Map::new(e))
}

pub fn set_milestone_status(e: &Env, vesting_id: u32, status: &Map<u32, bool>) {
    e.storage().instance().set(&(MILESTONE_STATUS, vesting_id), status);
}

pub fn get_emergency_pause_duration() -> u64 {
    EMERGENCY_PAUSE_DURATION
}

// Zero-Knowledge Privacy Claims functions

// Nullifier map functions - prevent double-spending in private claims
pub fn is_nullifier_used(e: &Env, nullifier: &Nullifier) -> bool {
    e.storage()
        .instance()
        .get(&(NULLIFIER_MAP, nullifier.clone()))
        .unwrap_or(false)
}

pub fn set_nullifier_used(e: &Env, nullifier: &Nullifier) {
    e.storage().instance().set(&(NULLIFIER_MAP, nullifier.clone()), &true);
}

// Commitment storage functions
pub fn get_commitment(e: &Env, commitment_hash: &BytesN<32>) -> Option<Commitment> {
    e.storage().instance().get(&(COMMITMENT_STORAGE, commitment_hash.clone()))
}

pub fn set_commitment(e: &Env, commitment_hash: &BytesN<32>, commitment: &Commitment) {
    e.storage().instance().set(&(COMMITMENT_STORAGE, commitment_hash.clone()), commitment);
}

pub fn mark_commitment_used(e: &Env, commitment_hash: &BytesN<32>) {
    if let Some(mut commitment) = get_commitment(e, commitment_hash) {
        commitment.is_used = true;
        set_commitment(e, commitment_hash, &commitment);
    }
}

// Privacy claim history functions
pub fn get_privacy_claim_history(e: &Env) -> Vec<crate::types::PrivacyClaimEvent> {
    e.storage()
        .instance()
        .get(&PRIVACY_CLAIM_HISTORY)
        .unwrap_or(Vec::new(e))
}

pub fn add_privacy_claim_event(e: &Env, event: &crate::types::PrivacyClaimEvent) {
    let mut history = get_privacy_claim_history(e);
    history.push_back(event.clone());
    e.storage().instance().set(&PRIVACY_CLAIM_HISTORY, &history);
}

// Merkle root management for ZK proofs
pub fn add_merkle_root(e: &Env, merkle_root: &BytesN<32>) {
    let mut roots = get_merkle_roots(e);
    roots.push_back(merkle_root.clone());
    e.storage().instance().set(&MERKLE_ROOTS, &roots);
}

pub fn get_merkle_roots(e: &Env) -> Vec<BytesN<32>> {
    e.storage()
        .instance()
        .get(&MERKLE_ROOTS)
        .unwrap_or(Vec::new(e))
}

pub fn is_valid_merkle_root(e: &Env, merkle_root: &BytesN<32>) -> bool {
    let roots = get_merkle_roots(e);
    roots.contains(merkle_root.clone())
}

// Stellar Horizon Path Payment Claim storage functions
pub fn get_path_payment_config(e: &Env) -> Option<PathPaymentConfig> {
    e.storage().instance().get(&PATH_PAYMENT_CONFIG)
}

pub fn set_path_payment_config(e: &Env, config: &PathPaymentConfig) {
    e.storage().instance().set(&PATH_PAYMENT_CONFIG, config);
}

pub fn get_path_payment_claim_history(e: &Env) -> Vec<PathPaymentClaimEvent> {
    e.storage()
        .instance()
        .get(&PATH_PAYMENT_CLAIM_HISTORY)
        .unwrap_or(Vec::new(e))
}

pub fn add_path_payment_claim_event(e: &Env, event: &PathPaymentClaimEvent) {
    let mut history = get_path_payment_claim_history(e);
    history.push_back(event.clone());
    e.storage().instance().set(&PATH_PAYMENT_CLAIM_HISTORY, &history);
}

// Lock-up period storage functions
pub fn get_lockup_config(e: &Env, vesting_id: u32) -> Option<LockupConfig> {
    e.storage().instance().get(&(LOCKUP_CONFIGS, vesting_id))
}

pub fn set_lockup_config(e: &Env, vesting_id: u32, config: &LockupConfig) {
    e.storage().instance().set(&(LOCKUP_CONFIGS, vesting_id), config);
}

pub fn remove_lockup_config(e: &Env, vesting_id: u32) {
    e.storage().instance().remove(&(LOCKUP_CONFIGS, vesting_id));
}

// ========== TAX CONFIGURATION STORAGE ==========
pub const TAX_CONFIGS: &str = "TAX_CONFIGS"; // keyed by vesting_id
pub const CUMULATIVE_TAXES: &str = "CUMULATIVE_TAXES"; // keyed by (vesting_id, authority)

pub fn get_tax_config(e: &Env, vesting_id: u32) -> Option<crate::types::TaxConfig> {
    e.storage().instance().get(&(TAX_CONFIGS, vesting_id))
}

pub fn set_tax_config(e: &Env, vesting_id: u32, cfg: &crate::types::TaxConfig) {
    e.storage().instance().set(&(TAX_CONFIGS, vesting_id), cfg);
}

pub fn get_cumulative_taxes(e: &Env, vesting_id: u32, authority: &Address) -> i128 {
    e.storage()
        .instance()
        .get(&(CUMULATIVE_TAXES, vesting_id, authority.clone()))
        .unwrap_or(0i128)
}

pub fn add_cumulative_taxes(e: &Env, vesting_id: u32, authority: &Address, amount: i128) {
    let prev = get_cumulative_taxes(e, vesting_id, authority);
    e.storage().instance().set(&(CUMULATIVE_TAXES, vesting_id, authority.clone()), &(prev + amount));
}

// Beneficiary reassignment and governance veto storage functions
pub fn get_reassignment_counter(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&REASSIGNMENT_COUNTER)
        .unwrap_or(0)
}

pub fn set_reassignment_counter(e: &Env, counter: u32) {
    e.storage().instance().set(&REASSIGNMENT_COUNTER, &counter);
}

pub fn get_beneficiary_reassignment(e: &Env, reassignment_id: u32) -> Option<BeneficiaryReassignment> {
    e.storage().instance().get(&(BENEFICIARY_REASSIGNMENTS, reassignment_id))
}

pub fn set_beneficiary_reassignment(e: &Env, reassignment_id: u32, reassignment: &BeneficiaryReassignment) {
    e.storage().instance().set(&(BENEFICIARY_REASSIGNMENTS, reassignment_id), reassignment);
}

pub fn remove_beneficiary_reassignment(e: &Env, reassignment_id: u32) {
    e.storage().instance().remove(&(BENEFICIARY_REASSIGNMENTS, reassignment_id));
}

pub fn get_veto_votes(e: &Env, reassignment_id: u32) -> Vec<VetoVote> {
    e.storage()
        .instance()
        .get(&(VETO_VOTES, reassignment_id))
        .unwrap_or(Vec::new(e))
}

pub fn set_veto_votes(e: &Env, reassignment_id: u32, votes: &Vec<VetoVote>) {
    e.storage().instance().set(&(VETO_VOTES, reassignment_id), votes);
}

pub fn add_veto_vote(e: &Env, reassignment_id: u32, vote: &VetoVote) {
    let mut votes = get_veto_votes(e, reassignment_id);
    votes.push_back(vote.clone());
    set_veto_votes(e, reassignment_id, &votes);
}

pub fn get_token_supply_info(e: &Env) -> TokenSupplyInfo {
    e.storage()
        .instance()
        .get(&TOKEN_SUPPLY_INFO)
        .unwrap_or(TokenSupplyInfo {
            total_supply: 0,
            last_updated: 0,
        })
}

pub fn set_token_supply_info(e: &Env, supply_info: &TokenSupplyInfo) {
    e.storage().instance().set(&TOKEN_SUPPLY_INFO, supply_info);
}

pub fn get_governance_veto_threshold(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&GOVERNANCE_VETO_THRESHOLD)
        .unwrap_or(5) // Default 5% threshold
}

pub fn set_governance_veto_threshold(e: &Env, threshold: u32) {
    e.storage().instance().set(&GOVERNANCE_VETO_THRESHOLD, &threshold);
}

// 7 days in seconds for governance veto period
const GOVERNANCE_VETO_PERIOD: u64 = 604_800;

pub fn get_governance_veto_period() -> u64 {
    GOVERNANCE_VETO_PERIOD
}

// LST Deposit support
pub fn get_lst_config(e: &Env, vesting_id: u32) -> Option<LSTConfig> {
    e.storage().instance().get(&(LST_CONFIGS, vesting_id))
}

pub fn set_lst_config(e: &Env, vesting_id: u32, config: &LSTConfig) {
    e.storage().instance().set(&(LST_CONFIGS, vesting_id), config);
}

// --- Tax withholding backwards-compatible wrappers ---
pub const TAX_WITHHOLDING_CONFIG: &str = "TAX_WITHHOLDING_CONFIG";

pub fn set_tax_withholding_config(e: &Env, cfg: &crate::types::TaxWithholdingConfig) {
    e.storage().instance().set(&TAX_WITHHOLDING_CONFIG, cfg);
}

pub fn get_tax_withholding_config(e: &Env) -> Option<crate::types::TaxWithholdingConfig> {
    e.storage().instance().get(&TAX_WITHHOLDING_CONFIG)
}

// --- SEP-12 oracle config storage ---
pub const SEP12_ORACLE: &str = "SEP12_ORACLE";

pub fn set_sep12_identity_oracle(e: &Env, oracle: &crate::types::SEP12IdentityOracle) {
    e.storage().instance().set(&SEP12_ORACLE, oracle);
}

pub fn get_sep12_identity_oracle(e: &Env) -> Option<crate::types::SEP12IdentityOracle> {
    e.storage().instance().get(&SEP12_ORACLE)
}

// --- Token metadata storage ---
pub const TOKEN_METADATA: &str = "TOKEN_METADATA";

pub fn set_token_metadata(e: &Env, asset: &Address, metadata: &crate::types::TokenMetadata) {
    e.storage().instance().set(&(TOKEN_METADATA, asset.clone()), metadata);
}

pub fn get_token_metadata(e: &Env, asset: &Address) -> Option<crate::types::TokenMetadata> {
    e.storage().instance().get(&(TOKEN_METADATA, asset.clone()))
}

// --- Vesting grant storage ---
pub const VESTING_GRANT: &str = "VESTING_GRANT"; // keyed by vesting_id

pub fn set_vesting_grant(e: &Env, vesting_id: u32, grant: &crate::types::VestingGrant) {
    e.storage().instance().set(&(VESTING_GRANT, vesting_id), grant);
}

pub fn get_vesting_grant(e: &Env, vesting_id: u32) -> Option<crate::types::VestingGrant> {
    e.storage().instance().get(&(VESTING_GRANT, vesting_id))
}

// ========== ISSUE #223: Voting Power (Total Unvested Balance per address) ==========
pub const TOTAL_UNVESTED_BALANCE: &str = "TOTAL_UNVESTED_BALANCE";

pub fn get_unvested_balance(e: &Env, address: &Address) -> i128 {
    e.storage()
        .instance()
        .get(&(TOTAL_UNVESTED_BALANCE, address))
        .unwrap_or(0i128)
}

pub fn set_unvested_balance(e: &Env, address: &Address, balance: i128) {
    e.storage().instance().set(&(TOTAL_UNVESTED_BALANCE, address), &balance);
}

// ========== ISSUE #226: Admin Dead-Man's Switch ==========
pub const ADMIN_DEAD_MAN_SWITCH: &str = "ADMIN_DEAD_MAN_SWITCH";

pub fn get_admin_dead_man_switch(e: &Env) -> Option<crate::types::AdminDeadManSwitch> {
    e.storage().instance().get(&ADMIN_DEAD_MAN_SWITCH)
}

pub fn set_admin_dead_man_switch(e: &Env, switch: &crate::types::AdminDeadManSwitch) {
    e.storage().instance().set(&ADMIN_DEAD_MAN_SWITCH, switch);
}

// ========== ISSUE #228: Oracle Price Deviation Circuit Breaker ==========
pub const ORACLE_PRICE_RECORD: &str = "ORACLE_PRICE_RECORD";

pub fn get_oracle_price_record(e: &Env) -> Option<crate::types::OraclePriceRecord> {
    e.storage().instance().get(&ORACLE_PRICE_RECORD)
}

pub fn set_oracle_price_record(e: &Env, record: &crate::types::OraclePriceRecord) {
    e.storage().instance().set(&ORACLE_PRICE_RECORD, record);
}

// ========== ISSUE #231: Total Unvested Balance (contract-wide) ==========
pub const CONTRACT_TOTAL_UNVESTED: &str = "CONTRACT_TOTAL_UNVESTED";

pub fn get_contract_total_unvested(e: &Env) -> i128 {
    e.storage()
        .instance()
        .get(&CONTRACT_TOTAL_UNVESTED)
        .unwrap_or(0i128)
}

pub fn set_contract_total_unvested(e: &Env, total: i128) {
    e.storage().instance().set(&CONTRACT_TOTAL_UNVESTED, &total);
}

// ========== ISSUE #269: Zero-Knowledge Confidential Grant Amounts ==========
pub const CONFIDENTIAL_GRANTS: &str = "CONFIDENTIAL_GRANTS";
pub const MASTER_VIEWING_KEY: &str = "MASTER_VIEWING_KEY";
pub const NULLIFIER_SET: &str = "NULLIFIER_SET";

// ========== ISSUE #295: Temporary Storage for Claim-History Pagination ==========
pub const PAGINATION_STATE: &str = "PAGINATION_STATE";
pub const CLAIM_HISTORY_PAGE_SIZE: u32 = 100;

// ========== ISSUE #296: Force-Withdrawal for Expired Schedules ==========
pub const EXPIRED_SCHEDULES: &str = "EXPIRED_SCHEDULES";

// ========== ISSUE #297: Max-Allocation-Sanity-Check ==========
pub const MAX_ALLOCATION_LIMIT: &str = "MAX_ALLOCATION_LIMIT";
pub const TOTAL_ALLOCATED: &str = "TOTAL_ALLOCATED";

// Confidential grant storage functions
pub fn get_confidential_grant(e: &Env, vesting_id: u32) -> Option<ConfidentialGrant> {
    e.storage().instance().get(&(CONFIDENTIAL_GRANTS, vesting_id))
}

pub fn set_confidential_grant(e: &Env, vesting_id: u32, grant: &ConfidentialGrant) {
    e.storage().instance().set(&(CONFIDENTIAL_GRANTS, vesting_id), grant);
}

pub fn remove_confidential_grant(e: &Env, vesting_id: u32) {
    e.storage().instance().remove(&(CONFIDENTIAL_GRANTS, vesting_id));
}

// Master viewing key storage functions
pub fn get_master_viewing_key(e: &Env) -> Option<MasterViewingKey> {
    e.storage().instance().get(&MASTER_VIEWING_KEY)
}

pub fn set_master_viewing_key(e: &Env, key: &MasterViewingKey) {
    e.storage().instance().set(&MASTER_VIEWING_KEY, key);
}

pub fn remove_master_viewing_key(e: &Env) {
    e.storage().instance().remove(&MASTER_VIEWING_KEY);
}

// Nullifier set in Persistent storage (for permanent tracking)
pub fn is_nullifier_in_set(e: &Env, nullifier_hash: &BytesN<32>) -> bool {
    e.storage()
        .persistent()
        .get(&(NULLIFIER_SET, nullifier_hash))
        .unwrap_or(false)
}

pub fn add_nullifier_to_set(e: &Env, nullifier_hash: &BytesN<32>) {
    e.storage().persistent().set(&(NULLIFIER_SET, nullifier_hash), &true);
}

// Stream pause functions for suspicious activity detection
pub fn get_stream_pause(e: &Env, vesting_id: u32, beneficiary: &Address) -> Option<StreamPause> {
    e.storage()
        .instance()
        .get(&(STREAM_PAUSES, vesting_id, beneficiary))
}

pub fn set_stream_pause(e: &Env, vesting_id: u32, beneficiary: &Address, pause: &StreamPause) {
    e.storage()
        .instance()
        .set(&(STREAM_PAUSES, vesting_id, beneficiary), pause);
}

pub fn remove_stream_pause(e: &Env, vesting_id: u32, beneficiary: &Address) {
    e.storage()
        .instance()
        .remove(&(STREAM_PAUSES, vesting_id, beneficiary));
}

pub fn is_stream_paused(e: &Env, vesting_id: u32, beneficiary: &Address) -> bool {
    if let Some(pause) = get_stream_pause(e, vesting_id, beneficiary) {
        return pause.is_active;
    }
    false
}

pub fn get_stream_pause_history(e: &Env) -> Vec<StreamPause> {
    e.storage()
        .instance()
        .get(&STREAM_PAUSE_HISTORY)
        .unwrap_or(Vec::new(e))
}

pub fn add_stream_pause_to_history(e: &Env, pause: &StreamPause) {
    let mut history = get_stream_pause_history(e);
    history.push_back(pause.clone());
    e.storage().instance().set(&STREAM_PAUSE_HISTORY, &history);
}

// ========== ISSUE #280: Protocol Sunset and Migration Functions ==========

pub fn get_protocol_sunset(e: &Env) -> Option<crate::types::ProtocolSunset> {
    e.storage().instance().get(&PROTOCOL_SUNSET)
}

pub fn set_protocol_sunset(e: &Env, sunset: &crate::types::ProtocolSunset) {
    e.storage().instance().set(&PROTOCOL_SUNSET, sunset);
}

pub fn get_migration_payload(e: &Env, beneficiary: &Address, vesting_id: u32) -> Option<crate::types::MigrationPayload> {
    e.storage().instance().get(&(MIGRATION_PAYLOADS, beneficiary.clone(), vesting_id))
}

pub fn set_migration_payload(e: &Env, beneficiary: &Address, vesting_id: u32, payload: &crate::types::MigrationPayload) {
    e.storage().instance().set(&(MIGRATION_PAYLOADS, beneficiary.clone(), vesting_id), payload);
}

pub fn get_relayer_migration(e: &Env, beneficiary: &Address, vesting_id: u32) -> Option<crate::types::RelayerMigration> {
    e.storage().instance().get(&(RELAYER_MIGRATIONS, beneficiary.clone(), vesting_id))
}

pub fn set_relayer_migration(e: &Env, beneficiary: &Address, vesting_id: u32, migration: &crate::types::RelayerMigration) {
    e.storage().instance().set(&(RELAYER_MIGRATIONS, beneficiary.clone(), vesting_id), migration);
}
