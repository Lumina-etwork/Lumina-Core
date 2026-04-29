use soroban_sdk::{contracttype, contractevent, Address, Vec, Map, String, BytesN, Bytes};

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamPauseReason {
    SuspiciousActivity,
    AnomalousClaimPattern,
    CompromisedAddress,
    RapidWithdrawal,
    UnusualBehavior,
    ManualReview,
}

#[contracttype]
#[derive(Clone)]
pub struct StreamPause {
    pub vesting_id: u32,
    pub beneficiary: Address,
    pub paused_at: u64,
    pub paused_by: Address,
    pub reason: StreamPauseReason,
    pub is_active: bool,
    pub notes: String,
}

#[contractevent]
#[derive(Clone)]
pub struct StreamPaused {
    #[topic]
    pub vesting_id: u32,
    #[topic]
    pub beneficiary: Address,
    pub paused_at: u64,
    pub paused_by: Address,
    pub reason: StreamPauseReason,
}

#[contractevent]
#[derive(Clone)]
pub struct StreamUnpaused {
    #[topic]
    pub vesting_id: u32,
    #[topic]
    pub beneficiary: Address,
    pub unpaused_at: u64,
    pub unpaused_by: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct ClaimEvent {
    pub beneficiary: Address,
    pub amount: i128,
    pub timestamp: u64,
    pub vesting_id: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct AuthorizedPayoutAddress {
    pub beneficiary: Address,
    pub authorized_address: Address,
    pub requested_at: u64,
    pub effective_at: u64,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct AddressWhitelistRequest {
    pub beneficiary: Address,
    pub requested_address: Address,
    pub requested_at: u64,
    pub effective_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct AuthorizedAddressSet {
    #[topic]
    pub beneficiary: Address,
    pub authorized_address: Address,
    pub effective_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct AddressWhitelistRequested {
    #[topic]
    pub beneficiary: Address,
    pub requested_address: Address,
    pub requested_at: u64,
    pub effective_at: u64,
}

// Milestone vesting types
#[contracttype]
#[derive(Clone)]
pub struct MilestoneConfig {
    pub vesting_id: u32,
    pub milestone_percentages: Vec<u32>, // Percentage for each milestone (e.g., [25, 25, 50])
    pub total_milestones: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct MilestoneStatus {
    pub vesting_id: u32,
    pub completed_milestones: Map<u32, bool>, // milestone_number -> completed
    pub last_completed: u32,
}

#[contractevent]
#[derive(Clone)]
pub struct MilestoneCompleted {
    #[topic]
    pub vesting_id: u32,
    pub milestone_number: u32,
    pub completed_at: u64,
}

// Simulation types
#[contracttype]
#[derive(Clone)]
pub struct ClaimSimulation {
    pub tokens_to_release: i128,
    pub estimated_gas_fee: u64,
    pub tax_withholding_amount: i128,
    pub net_amount: i128,
    pub can_claim: bool,
    pub reason: String,
}

// Tax configuration for a vesting schedule
#[contracttype]
#[derive(Clone)]
pub struct TaxConfig {
    pub tax_bps: u32, // basis points (10000 = 100%)
    pub authority: Address, // tax authority receiving payments
    pub tax_asset: Option<Address>, // if Some, tax must be paid in this asset (may require swap)
}

#[contractevent]
#[derive(Clone)]
pub struct TaxWithheld {
    #[topic]
    pub vesting_id: u32,
    pub beneficiary: Address,
    pub gross_amount: i128,
    pub tax_amount: i128,
    pub net_amount: i128,
    pub timestamp: u64,
}

// Backwards-compatible tax withholding config used by other helpers
#[contracttype]
#[derive(Clone)]
pub struct TaxWithholdingConfig {
    pub tax_treasury_address: Address,
    pub tax_withholding_bps: u32,
    pub enabled: bool,
}

#[contractevent]
#[derive(Clone)]
pub struct TaxWithholdingConfigured {
    pub tax_treasury_address: Address,
    pub tax_withholding_bps: u32,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct TaxWithholdingDisabled {
    pub timestamp: u64,
}

// SEP-12 Oracle config
#[contracttype]
#[derive(Clone)]
pub struct SEP12IdentityOracle {
    pub contract_address: Address,
    pub enabled: bool,
}

#[contractevent]
#[derive(Clone)]
pub struct SEP12OracleConfigured {
    pub oracle_address: Address,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct SEP12KYCDisabled {
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct KYCCheckFailed {
    #[topic]
    pub beneficiary: Address,
    pub reason: String,
    pub timestamp: u64,
}

// Token metadata registry
#[contracttype]
#[derive(Clone)]
pub struct TokenMetadata {
    pub asset_address: Address,
    pub decimals: u32,
}

#[contractevent]
#[derive(Clone)]
pub struct TokenMetadataRegistered {
    pub asset_address: Address,
    pub decimals: u32,
    pub timestamp: u64,
}

// Vesting grant for revocability expiration
#[contracttype]
#[derive(Clone)]
pub struct VestingGrant {
    pub vesting_id: u32,
    pub beneficiary: Address,
    pub created_at: u64,
    pub is_revocable: bool,
    pub revocability_expires_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct VestingGrantCreated {
    pub vesting_id: u32,
    pub beneficiary: Address,
    pub is_revocable: bool,
    pub revocability_expires_at: u64,
    pub created_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct RevocabilityExpired {
    pub vesting_id: u32,
    pub beneficiary: Address,
    pub expired_at: u64,
}

// Reputation bridge types
#[contracttype]
#[derive(Clone)]
pub struct ReputationBonus {
    pub beneficiary: Address,
    pub cliff_reduction_months: u32,
    pub applied_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct ReputationBonusApplied {
    #[topic]
    pub beneficiary: Address,
    pub cliff_reduction_months: u32,
    pub applied_at: u64,
}

// Zero-Knowledge Privacy Claims types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Nullifier {
    pub hash: BytesN<32>, // 256-bit hash
}

#[contracttype]
#[derive(Clone)]
pub struct Commitment {
    pub hash: BytesN<32>, // 256-bit hash
    pub created_at: u64,
    pub vesting_id: u32,
    pub amount: i128,
    pub is_used: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct ZKClaimProof {
    pub commitment_hash: BytesN<32>,
    pub nullifier_hash: BytesN<32>,
    pub merkle_root: BytesN<32>,
    pub proof_data: Bytes, // Placeholder for actual ZK-SNARK proof
}

#[contracttype]
#[derive(Clone)]
pub struct PrivacyClaimEvent {
    pub nullifier: Nullifier,
    pub amount: i128,
    pub timestamp: u64,
    pub vesting_id: u32,
    pub is_private: bool,
}

#[contractevent]
#[derive(Clone)]
pub struct CommitmentCreated {
    #[topic]
    pub commitment_hash: BytesN<32>,
    #[topic]
    pub vesting_id: u32,
    pub amount: i128,
    pub created_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct PrivateClaimExecuted {
    #[topic]
    pub nullifier_hash: BytesN<32>,
    pub amount: i128,
    pub timestamp: u64,
}

// Stellar Horizon Path Payment Claim types
#[contracttype]
#[derive(Clone)]
pub struct PathPaymentConfig {
    pub destination_asset: Address, // USDC or other stablecoin
    pub min_destination_amount: i128,
    pub path: Vec<Address>, // Path of assets for the swap
    pub enabled: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct PathPaymentClaimEvent {
    pub beneficiary: Address,
    pub source_amount: i128,
    pub destination_amount: i128,
    pub destination_asset: Address,
    pub timestamp: u64,
    pub vesting_id: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct PathPaymentSimulation {
    pub source_amount: i128,
    pub estimated_destination_amount: i128,
    pub min_destination_amount: i128,
    pub path: Vec<Address>,
    pub can_execute: bool,
    pub reason: String,
    pub estimated_gas_fee: u64,
}
#[contractevent]
#[derive(Clone)]
pub struct PathPaymentConfigured {
    pub destination_asset: Address,
    pub min_destination_amount: i128,
    pub path: Vec<Address>,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct PathPaymentDisabled {
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct PathPaymentClaimExecuted {
    #[topic]
    pub user: Address,
    pub source_amount: i128,
    pub destination_amount: i128,
    pub destination_asset: Address,
    pub timestamp: u64,
    #[topic]
    pub vesting_id: u32,
}

// Lock-up period types
#[contracttype]
#[derive(Clone)]
pub struct LockupConfig {
    pub vesting_id: u32,
    pub lockup_duration_seconds: u64,
    pub enabled: bool,
    pub lockup_token_address: Address,
}

#[contractevent]
#[derive(Clone)]
pub struct LockupConfigured {
    #[topic]
    pub vesting_id: u32,
    pub lockup_duration_seconds: u64,
    pub lockup_token_address: Address,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct LockupDisabled {
    #[topic]
    pub vesting_id: u32,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct LockupClaimExecuted {
    #[topic]
    pub user: Address,
    #[topic]
    pub vesting_id: u32,
    pub amount: i128,
    pub lockup_token_address: Address,
    pub unlock_time: u64,
    pub timestamp: u64,
}

// Beneficiary reassignment types (Issue 114)
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryReassignment {
    pub vesting_id: u32,
    pub current_beneficiary: Address,
    pub new_beneficiary: Address,
    pub requested_at: u64,
    pub effective_at: u64,
    pub total_amount: i128,
    pub requires_governance_veto: bool,
    pub is_executed: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct GovernanceVeto {
    pub reassignment_id: u32,
    pub veto_by: Address,
    pub veto_at: u64,
    pub reason: String,
    pub voting_power: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct VetoVote {
    pub voter: Address,
    pub reassignment_id: u32,
    pub vote_for_veto: bool,
    pub voting_power: i128,
    pub voted_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct TokenSupplyInfo {
    pub total_supply: i128,
    pub last_updated: u64,
}

// Governance veto events (use contracttype to avoid macro issues)
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryReassignmentRequested {
    pub reassignment_id: u32,
    pub vesting_id: u32,
    pub current_beneficiary: Address,
    pub new_beneficiary: Address,
    pub total_amount: i128,
    pub effective_at: u64,
    pub requires_governance_veto: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryReassignmentExecuted {
    pub reassignment_id: u32,
    pub vesting_id: u32,
    pub old_beneficiary: Address,
    pub new_beneficiary: Address,
    pub executed_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct VetoPeriodStarted {
    pub reassignment_id: u32,
    pub vesting_id: u32,
    pub veto_deadline: u64,
    pub threshold_percentage: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct VetoVoteCast {
    pub voter: Address,
    pub reassignment_id: u32,
    pub vote_for_veto: bool,
    pub voting_power: i128,
    pub voted_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct ReassignmentVetoed {
    pub reassignment_id: u32,
    pub veto_triggered_by: Address,
    pub veto_power: i128,
    pub vetoed_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct ReassignmentApproved {
    pub reassignment_id: u32,
    pub approved_at: u64,
    pub total_veto_power: i128,
}

// LST Deposit support
#[contracttype]
#[derive(Clone)]
pub struct LSTConfig {
    pub vesting_id: u32,
    pub enabled: bool,
    pub lst_token_address: Address,
    pub base_token_address: Address,
    pub staking_contract_address: Address,
    pub unbonding_period_seconds: u64,
}

// LST Auto-Compounding types (Issue #154)
#[contracttype]
#[derive(Clone)]
pub struct LSTPoolShares {
    /// Total shares in the pool (tracks ownership proportionally)
    pub total_shares: i128,
    /// Total underlying tokens in the pool (including compounded rewards)
    pub total_underlying: i128,
    /// Last compounding timestamp
    pub last_compounded_at: u64,
    /// Exchange rate snapshot (for security against manipulation)
    pub exchange_rate_snapshot: i128,
    /// Snapshot timestamp
    pub snapshot_timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct UserLSTShares {
    /// User's share balance in the pool
    pub shares: i128,
    /// User's vesting ID
    pub vesting_id: u32,
    /// Whether the user has an unbonding request pending
    pub unbonding_pending: bool,
    /// Unbonding request timestamp (0 if not pending)
    pub unbonding_requested_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct UnbondingRequest {
    pub user: Address,
    pub vesting_id: u32,
    pub shares: i128,
    pub requested_at: u64,
    pub unbonding_complete_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct LSTConfigured {
    #[topic]
    pub vesting_id: u32,
    pub lst_token_address: Address,
    pub base_token_address: Address,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct LSTClaimExecuted {
    #[topic]
    pub user: Address,
    #[topic]
    pub vesting_id: u32,
    pub base_amount: i128,
    pub lst_amount: i128,
    pub lst_token_address: Address,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct LSTRewardsCompounded {
    #[topic]
    pub vesting_id: u32,
    pub total_yield_generated: i128,
    pub total_shares: i128,
    pub exchange_rate: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct UnbondingRequested {
    #[topic]
    pub user: Address,
    #[topic]
    pub vesting_id: u32,
    pub shares: i128,
    pub unbonding_complete_at: u64,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct UnbondingCompleted {
    #[topic]
    pub user: Address,
    #[topic]
    pub vesting_id: u32,
    pub shares: i128,
    pub underlying_amount: i128,
    pub timestamp: u64,
}

// ========== ISSUE #223: Cross-Contract balanceOf Adapter for DAO Voting ==========

#[contractevent]
#[derive(Clone)]
pub struct VotingPowerQueried {
    #[topic]
    pub voter: Address,
    pub voting_power: i128,
    pub timestamp: u64,
}

// ========== ISSUE #226: Admin Dead-Man's Switch ==========

/// 365 days in seconds
pub const ADMIN_INACTIVITY_TIMEOUT: u64 = 31_536_000;

#[contracttype]
#[derive(Clone)]
pub struct AdminDeadManSwitch {
    /// The recovery address that can claim admin rights after inactivity
    pub recovery_address: Address,
    /// Timestamp of the last admin activity
    pub last_admin_activity: u64,
    /// Whether the switch has been triggered (recovery claimed)
    pub is_triggered: bool,
}

#[contractevent]
#[derive(Clone)]
pub struct AdminRecoveryAddressSet {
    #[topic]
    pub recovery_address: Address,
    pub set_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct AdminActivityRecorded {
    #[topic]
    pub admin: Address,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct AdminRecoveryClaimed {
    #[topic]
    pub recovery_address: Address,
    pub claimed_at: u64,
}

// ========== ISSUE #228: Oracle Price Deviation Circuit Breaker ==========

/// 30% deviation threshold (in basis points: 3000 = 30%)
pub const ORACLE_DEVIATION_THRESHOLD_BPS: u32 = 3000;

#[contracttype]
#[derive(Clone)]
pub struct OraclePriceRecord {
    /// Price at the last ledger (scaled by 10^7)
    pub last_price: i128,
    /// Ledger sequence number of the last price update
    pub last_ledger: u32,
    /// Whether the circuit breaker is currently tripped
    pub is_frozen: bool,
    /// Timestamp when the freeze was triggered (0 if not frozen)
    pub frozen_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct OraclePriceUpdated {
    pub old_price: i128,
    pub new_price: i128,
    pub ledger: u32,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct OracleCircuitBreakerTripped {
    pub old_price: i128,
    pub new_price: i128,
    pub deviation_bps: u32,
    pub tripped_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct OracleCircuitBreakerReset {
    pub reset_by: Address,
    pub reset_at: u64,
}

// ========== ISSUE #231: Self-Destruct Prevention ==========

#[contractevent]
#[derive(Clone)]
pub struct UpgradeBlocked {
    pub total_unvested_balance: i128,
    pub blocked_at: u64,
}

// ========== ISSUE #269: Zero-Knowledge Confidential Grant Amounts ==========

/// Confidential grant storing commitment instead of plaintext amount
#[contracttype]
#[derive(Clone)]
pub struct ConfidentialGrant {
    /// Hash commitment of the total grant amount (Pedersen commitment)
    pub commitment_hash: BytesN<32>,
    /// Vesting schedule identifier
    pub vesting_id: u32,
    /// Timestamp when grant was created
    pub created_at: u64,
    /// Whether this grant has been fully claimed
    pub is_fully_claimed: bool,
    /// Remaining shielded amount (encrypted, for internal tracking)
    pub remaining_shielded: i128,
}

/// Master viewing key for DAO clawback operations
#[contracttype]
#[derive(Clone)]
pub struct MasterViewingKey {
    /// Public key for viewing shielded amounts
    pub viewing_key: BytesN<32>,
    /// Admin address that authorized this key
    pub authorized_by: Address,
    /// Timestamp when key was set
    pub set_at: u64,
    /// Whether key is currently active
    pub is_active: bool,
}

/// Enhanced ZK proof for confidential claims (Circom-compatible)
#[contracttype]
#[derive(Clone)]
pub struct ConfidentialClaimProof {
    /// Public inputs for the ZK circuit
    pub commitment_hash: BytesN<32>,
    /// Nullifier to prevent double-spending
    pub nullifier: BytesN<32>,
    /// Merkle root of the commitment tree
    pub merkle_root: BytesN<32>,
    /// Claimed amount (public output)
    pub claimed_amount: i128,
    /// Remaining amount after claim (public output)
    pub remaining_amount: i128,
    /// The actual ZK-SNARK proof (Circom output)
    pub proof_a: BytesN<32>,
    pub proof_b: BytesN<32>,
    pub proof_c: BytesN<32>,
}

/// Event emitted when a confidential claim is executed
#[contractevent]
#[derive(Clone)]
pub struct ConfidentialClaimExecuted {
    /// Nullifier hash (leaks zero metadata about the claimer)
    #[topic]
    pub nullifier_hash: BytesN<32>,
    /// Updated commitment hash after claim
    #[topic]
    pub new_commitment_hash: BytesN<32>,
    /// Timestamp of the claim
    pub timestamp: u64,
}

/// Event emitted when a confidential grant is created
#[contractevent]
#[derive(Clone)]
pub struct ConfidentialGrantCreated {
    /// Vesting ID
    #[topic]
    pub vesting_id: u32,
    /// Commitment hash of the total grant
    #[topic]
    pub commitment_hash: BytesN<32>,
    /// Timestamp of creation
    pub timestamp: u64,
}

/// Event emitted when DAO performs clawback using master viewing key
#[contractevent]
#[derive(Clone)]
pub struct ConfidentialClawbackExecuted {
    /// Vesting ID
    #[topic]
    pub vesting_id: u32,
    /// Amount clawed back
    pub clawed_amount: i128,
    /// Admin who authorized the clawback
    #[topic]
    pub authorized_by: Address,
    /// Timestamp of clawback
    pub timestamp: u64,
}

// ========== ISSUE #295: Temporary Storage for Claim-History Pagination ==========

#[contracttype]
#[derive(Clone)]
pub struct PaginationState {
    pub current_page: u32,
    pub total_items: u32,
    pub last_updated: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct ClaimHistoryPage {
    pub page_number: u32,
    pub claims: Vec<ClaimEvent>,
    pub has_next: bool,
    pub total_pages: u32,
}

// ========== ISSUE #296: Force-Withdrawal for Expired Schedules ==========

#[contracttype]
#[derive(Clone)]
pub struct ExpiredSchedule {
    pub vesting_id: u32,
    pub beneficiary: Address,
    pub total_amount: i128,
    pub claimed_amount: i128,
    pub expires_at: u64,
    pub is_force_withdrawn: bool,
}

#[contractevent]
#[derive(Clone)]
pub struct ForceWithdrawalExecuted {
    #[topic]
    pub vesting_id: u32,
    #[topic]
    pub beneficiary: Address,
    pub withdrawn_amount: i128,
    pub reason: String,
    pub timestamp: u64,
}

// ========== ISSUE #297: Max-Allocation-Sanity-Check ==========

#[contractevent]
#[derive(Clone)]
pub struct MaxAllocationLimitSet {
    pub max_limit: i128,
    pub set_by: Address,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct AllocationLimitExceeded {
    #[topic]
    pub attempted_allocation: i128,
    pub max_limit: i128,
    pub rejected_at: u64,
}

// ========== ISSUE #280: Smart Contract Sunset and State Migration Hooks ==========

/// 30 days in seconds for sunset timelock
pub const SUNSET_TIMELOCK_DURATION: u64 = 2_592_000;

#[contracttype]
#[derive(Clone)]
pub struct ProtocolSunset {
    /// Whether sunset has been initiated
    pub is_initiated: bool,
    /// Timestamp when sunset was initiated
    pub initiated_at: u64,
    /// Timestamp when sunset becomes effective (30 days later)
    pub effective_at: u64,
    /// Address of the V3 contract to migrate to
    pub migration_target: Address,
    /// Whether sunset has been aborted
    pub is_aborted: bool,
    /// Whether new schedule creation is halted
    pub new_schedules_halted: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct MigrationPayload {
    /// Beneficiary address
    pub beneficiary: Address,
    /// Vesting schedule ID
    pub vesting_id: u32,
    /// Total grant amount
    pub total_amount: i128,
    /// Amount already claimed
    pub claimed_amount: i128,
    /// Remaining unvested amount
    pub remaining_amount: i128,
    /// Vesting start timestamp
    pub start_time: u64,
    /// Vesting end timestamp
    pub end_time: u64,
    /// Hash of the payload for verification
    pub payload_hash: BytesN<32>,
    /// Timestamp when payload was exported
    pub exported_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct RelayerMigration {
    /// Beneficiary address being migrated
    pub beneficiary: Address,
    /// Vesting schedule ID
    pub vesting_id: u32,
    /// Migration payload hash
    pub payload_hash: BytesN<32>,
    /// Whether migration has been completed
    pub is_completed: bool,
    /// Timestamp of migration
    pub migrated_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct SunsetInitiated {
    #[topic]
    pub initiated_by: Address,
    pub migration_target: Address,
    pub initiated_at: u64,
    pub effective_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct SunsetAborted {
    #[topic]
    pub aborted_by: Address,
    pub aborted_at: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct StateMigrated {
    #[topic]
    pub beneficiary: Address,
    #[topic]
    pub vesting_id: u32,
    pub payload_hash: BytesN<32>,
    pub migrated_at: u64,
}
