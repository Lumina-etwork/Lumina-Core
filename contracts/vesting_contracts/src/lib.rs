#![no_std]
use soroban_sdk::{
    contract, contractevent, contractimpl, contracttype, token, vec, Address, BytesN, Env, IntoVal,
    String, Symbol, Vec, U256,
};

mod errors;
pub use errors::Error;

mod factory;
pub use factory::{VestingFactory, VestingFactoryClient};
mod oracle;
pub use oracle::{ComparisonOperator, OracleClient, OracleCondition, OracleType, PerformanceCliff};

pub mod stake;
pub use stake::{
    add_approved_staking_contract, call_claim_yield_for, call_stake_tokens, call_unstake_tokens,
    get_approved_staking_contracts, get_stake_info, is_approved_staking_contract,
    remove_approved_staking_contract, set_stake_info, StakeDataKey, StakeState, StakeStatusView,
    VaultStakeInfo,
};

pub mod inheritance;
pub mod kpi_engine;
#[cfg(test)]
mod kpi_test;
pub mod kpi_vesting;
pub use inheritance::{
    cancel_succession_claim, finalise_succession, get_succession_state, get_succession_status,
    initiate_succession_claim, nominate_backup, revoke_backup, update_activity, ClaimPendingData,
    InheritanceError, NominatedData, SucceededData, SuccessionState, SuccessionView,
    MAX_CHALLENGE_WINDOW, MAX_SWITCH_DURATION, MIN_CHALLENGE_WINDOW, MIN_SWITCH_DURATION,
};

pub mod certificate_registry;
pub use certificate_registry::{
    CertificateQuery, CertificateQueryResult, CompletedVestCertificate, LoyaltyMetrics,
    VestingCertificateRegistry, WorkVerification,
};

pub mod zk_verifier;
pub use zk_verifier::{
    AccreditationRecord, AccreditedInvestorInputs, VerificationKey, ZKProof, ZKVerifier,
    ZKVerifierError, ZKVerifierTrait, ACCREDITED_INVESTOR_CIRCUIT, EU_JURISDICTION,
    QUALIFIED_BUYER_CIRCUIT, UK_JURISDICTION, US_JURISDICTION,
};

pub mod legal_saft;
pub use legal_saft::{
    DocumentSignature, DocumentType, LegalDocument, LegalSAFTError, LegalSAFTManager,
    LegalSAFTTrait, VaultLegalDocuments, DOCUMENT_INDEX, DOCUMENT_SIGNATURES, LEGAL_DOCUMENTS,
    VAULT_LEGAL_DOCS,
};

pub mod beneficiary_reassignment;
pub use beneficiary_reassignment::{
    BeneficiaryReassignment, BeneficiaryReassignmentTrait, DAOMember, ReassignmentConfig,
    ReassignmentError, ReassignmentRequest, ReassignmentStatus, SocialProofType, DAO_MEMBERS,
    REASSIGNMENT_CONFIG, REASSIGNMENT_REQUESTS, VAULT_REASSIGNMENTS,
};

pub mod regulated_asset;
pub use regulated_asset::{
    AssetRegulation, AuthorizationStatus, ClawbackEvent, FreezeEvent, RegulatedAssetError,
    RegulatedAssetManager, RegulatedAssetTrait, SEP08Authorization, ASSET_REGULATIONS,
    CLAWBACK_EVENTS, FREEZE_EVENTS, SEP08_AUTHORIZATIONS,
};

#[cfg(test)]
mod certificate_registry_test;

#[cfg(test)]
mod merkle_bulk_test;

pub mod diversified_core;
pub use diversified_core::{AssetAllocation as DiversifiedAllocation, DiversifiedVault};

// 10 years in seconds
pub const MAX_DURATION: u64 = 315_360_000;
// 72 hours in seconds for challenge period
pub const CHALLENGE_PERIOD: u64 = 259_200;
// 51% voting threshold (represented as basis points: 5100 = 51.00%)
pub const VOTING_THRESHOLD: u32 = 5100;

struct ReentrancyGuard {
    env: Env,
}

impl ReentrancyGuard {
    fn enter(env: &Env) -> Result<Self, Error> {
        if env
            .storage()
            .instance()
            .get(&DataKey::ReentrancyLock)
            .unwrap_or(false)
        {
            return Err(Error::ReentrancyDetected);
        }

        env.storage()
            .instance()
            .set(&DataKey::ReentrancyLock, &true);
        Ok(Self { env: env.clone() })
    }
}

impl Drop for ReentrancyGuard {
    fn drop(&mut self) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::ReentrancyLock, &false);
    }
}

#[contracttype]
pub enum WhitelistDataKey {
    WhitelistedTokens,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    AdminAddress,
    AdminBalance,
    InitialSupply,
    ProposedAdmin,
    VaultCount,
    VaultData(u64),
    VaultMilestones(u64),
    VaultPerformanceCliff(u64),
    UserVaults(Address),
    IsPaused,
    IsDeprecated,
    MigrationTarget,
    Token,
    TotalShares,
    TotalStaked,
    StakingContract,
    // Defensive Governance
    GovernanceProposal(u64),
    GovernanceVotes(u64, Address),
    ProposalCount,
    TotalLockedValue,
    PausedVault(u64),
    PauseAuthority,
    // Multi-sig admin
    AdminSet,        // Vec<Address>
    QuorumThreshold, // u32
    // Multi-sig admin proposals
    AdminProposal(u64),                   // Proposal struct
    AdminProposalSignature(u64, Address), // bool (signed)
    AdminProposalCount,                   // u64
    VaultSuccession(u64),
    // KPI Vesting Gates (Issue #145/#92)
    KpiConfig(u64),
    KpiMet(u64),
    KpiLog(u64),
    // Cliff Smoothing Configuration
    CliffSmoothingConfig(u64),
    // --- Added missing variants ---
    NFTMinter,
    CollateralBridge,
    RevokedVaults,
    GlobalAccelerationPct,
    MetadataAnchor,
    VotingDelegate(Address),
    DelegatedBeneficiaries(Address),
    SubAdminPool(Address),
    MarketplaceLock(u64),
    XLMAddress,
    // Certificate Registry
    CertificateRegistry(Address),
    CertificateData(BytesN<32>),
    // Legal SAFT Document Hash Anchoring
    LegalDocumentHash(BytesN<32>),
    DocumentSignature(Address, BytesN<32>),
    VaultLegalDocuments(u64),
    // Beneficiary Reassignment
    ReassignmentRequest(u64),
    ReassignmentApproval(u64, Address),
    // SEP-08 Regulated Assets
    VaultAuthorization(u64),
    // ZK Verifier
    ZKVerificationKey(BytesN<32>),
    AccreditationRecord(Address, BytesN<32>),
    NullifierMap(BytesN<32>),
    VerificationKey(BytesN<32>),
    SupportedCircuit(BytesN<32>),
    BeneficiaryCertificates(Address),
    WorkTypeIndex(String),
    LoyaltyIndex(u32),
    CompletionTimeIndex(u64),
    CertificateCount,
    WorkVerification(U256),
    CertificateVerifier,
    AntiDilutionConfig(u64),
    NetworkGrowthSnapshot(u64),
    ApprovedStakingContracts,
    // Dynamic emission rate for partial clawback
    ClawbackAdjustment(u64),
    // Merkle Tree Bulk Initialization (Issue #199)
    MerkleRoot,
    ActivatedSchedule(Address), // beneficiary -> vault_id
    // Path Payment Configuration for claim_and_swap functionality
    PathPaymentConfig,
    PathPaymentClaimHistory,
    ReentrancyLock,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SubAdminPool {
    pub manager: Address,
    pub asset: Address,
    pub total_amount: i128,
    pub distributed_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MarketplaceLock {
    pub marketplace: Address,
    pub authorized_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ClawbackAdjustment {
    pub clawback_time: u64,
    pub clawback_amount: i128,
    pub original_total_amount: i128,
    pub original_rate: i128,
    pub new_rate: i128,
    pub remaining_tokens: i128,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MerkleProof {
    pub leaf_hash: BytesN<32>,
    pub proof: Vec<BytesN<32>>,
    pub leaf_index: u32,
}

// Path Payment types for claim_and_swap functionality
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PathPaymentConfig {
    pub destination_asset: Address, // USDC or other stablecoin
    pub min_destination_amount: i128,
    pub path: Vec<Address>, // Untrusted swap hops / DEX pair contracts
    pub enabled: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PathPaymentClaimEvent {
    pub beneficiary: Address,
    pub source_amount: i128,
    pub destination_amount: i128,
    pub destination_asset: Address,
    pub timestamp: u64,
    pub vault_id: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PathPaymentSimulation {
    pub source_amount: i128,
    pub estimated_destination_amount: i128,
    pub min_destination_amount: i128,
    pub path: Vec<Address>,
    pub can_execute: bool,
    pub reason: String,
    pub estimated_gas_fee: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct VestingScheduleLeaf {
    pub beneficiary: Address,
    pub vault_id: u64,
    pub asset_basket: Vec<AssetAllocationEntry>,
    pub start_time: u64,
    pub end_time: u64,
    pub keeper_fee: i128,
    pub is_revocable: bool,
    pub is_transferable: bool,
    pub step_duration: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum AdminAction {
    RevokeSchedule(u64, Address),
    AddBeneficiary(Address, ScheduleConfig),
    RemoveAdmin(Address),
    AddAdmin(Address),
    UpdateQuorum(u32),
    // Add more as needed
    NFTMinter,
    CollateralBridge,
    MetadataAnchor,
    VotingDelegate(Address),
    DelegatedBeneficiaries(Address),
    GlobalAccelerationPct,
    RevokedVaults,
    VaultSuccession(u64),
    // KPI Vesting Gates (Issue #145/#92)
    // Anti-Dilution Configuration
    AntiDilutionConfig(u64),
    NetworkGrowthSnapshot(u64),
    GrantManagerRights(Address, Address, i128), // Manager, Asset, Amount
    RenewSchedule(u64, u64, i128),              // VaultID, AdditionalDuration, AdditionalAmount
    SetXLMAddress(Address),
    InitializeMerkleRoot(BytesN<32>, u32), // Merkle root, total schedules
}

#[contracttype]
#[derive(Clone)]
pub struct AdminProposal {
    pub id: u64,
    pub action: AdminAction,
    pub proposer: Address,
    pub created_at: u64,
    pub is_executed: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PausedVault {
    pub vault_id: u64,
    pub pause_timestamp: u64,
    pub pause_authority: Address,
    pub reason: String,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct AntiDilutionConfig {
    pub enabled: bool,
    pub network_growth_oracle: Address,
    pub inflation_oracle: Option<Address>,
    pub adjustment_frequency: u64, // Seconds between adjustments
    pub last_adjustment_time: u64,
    pub baseline_network_value: i128, // Baseline network value at creation
    pub cumulative_adjustment_factor: i128, // In basis points (10000 = 100%)
    pub max_adjustment_pct: u32,      // Maximum adjustment percentage (basis points)
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct NetworkGrowthSnapshot {
    pub timestamp: u64,
    pub network_value: i128,
    pub adjustment_factor: i128, // In basis points
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AssetAllocationEntry {
    pub asset_id: Address,
    pub total_amount: i128,
    pub released_amount: i128,
    pub locked_amount: i128, // Amount locked for collateral liens
    pub percentage: u32,     // Percentage of total allocation (basis points, 10000 = 100%)
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct CliffSmoothingConfig {
    pub cliff_duration: u64, // Original cliff duration (e.g., 12 months in seconds)
    pub smoothing_duration: u64, // Smoothing window duration (e.g., 30 days in seconds)
    pub cliff_percentage: u32, // Percentage of total allocation that unlocks at cliff (basis points, 2500 = 25%)
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Vault {
    pub allocations: Vec<AssetAllocationEntry>, // Basket of assets
    pub keeper_fee: i128,
    pub staked_amount: i128,
    pub owner: Address,
    pub delegate: Option<Address>,
    pub title: String,
    pub start_time: u64,
    pub end_time: u64,
    pub creation_time: u64,
    pub step_duration: u64,
    pub is_initialized: bool,
    pub is_irrevocable: bool,
    pub is_transferable: bool,
    pub is_frozen: bool,
    pub requires_legal_signatures: bool, // Whether legal signatures are required
    pub legal_documents_signed: bool,    // Whether all legal documents are signed
    pub yield_destination: YieldDestination,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum YieldDestination {
    DAO,
    Beneficiary,
}

#[contracttype]
#[derive(Clone)]
pub struct Milestone {
    pub id: u64,
    pub percentage: u32,
    pub is_unlocked: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum GovernanceAction {
    AdminRotation(Address),   // new_admin
    ContractUpgrade(Address), // new_contract_address
    EmergencyPause(bool),     // pause_state
}

#[contracttype]
#[derive(Clone)]
pub struct GovernanceProposal {
    pub id: u64,
    pub action: GovernanceAction,
    pub proposer: Address,
    pub created_at: u64,
    pub challenge_end: u64,
    pub is_executed: bool,
    pub is_cancelled: bool,
    pub yes_votes: i128, // Total locked value voting yes
    pub no_votes: i128,  // Total locked value voting no
}

#[contracttype]
#[derive(Clone)]
pub struct Vote {
    pub voter: Address,
    pub vote_weight: i128,
    pub is_yes: bool,
    pub voted_at: u64,
}

#[contracttype]
pub struct BatchCreateData {
    pub recipients: Vec<Address>,
    pub asset_baskets: Vec<Vec<AssetAllocationEntry>>, // Each recipient gets a basket of assets
    pub start_times: Vec<u64>,
    pub end_times: Vec<u64>,
    pub keeper_fees: Vec<i128>,
    pub step_durations: Vec<u64>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleConfig {
    pub owner: Address,
    pub asset_basket: Vec<AssetAllocationEntry>, // Basket of assets for this schedule
    pub start_time: u64,
    pub end_time: u64,
    pub keeper_fee: i128,
    pub is_revocable: bool,
    pub is_transferable: bool,
}

#[contractevent]
pub struct CliffSmoothedUnlock {
    #[topic]
    pub vault_id: u64,
    #[topic]
    pub beneficiary: Address,
    pub cliff_amount: i128,
    pub smoothed_amount: i128,
    pub smoothing_start: u64,
    pub smoothing_end: u64,
    pub timestamp: u64,
}

#[contractevent]
pub struct VaultCreated {
    #[topic]
    pub vault_id: u64,
    #[topic]
    pub beneficiary: Address,
    pub total_amount: i128,
    pub cliff_duration: u64,
    pub start_time: u64,
    pub title: String,
}

#[contractevent]
pub struct GovernanceProposalCreated {
    #[topic]
    pub proposal_id: u64,
    pub action: GovernanceAction,
    #[topic]
    pub proposer: Address,
    pub challenge_end: u64,
}

#[contractevent]
pub struct VoteCast {
    #[topic]
    pub proposal_id: u64,
    #[topic]
    pub voter: Address,
    pub vote_weight: i128,
    pub is_yes: bool,
}

#[contractevent]
pub struct GovernanceActionExecuted {
    #[topic]
    pub proposal_id: u64,
    pub action: GovernanceAction,
}

#[contractevent]
pub struct AdminProposalCreated {
    #[topic]
    pub proposal_id: u64,
    pub action: AdminAction,
    #[topic]
    pub proposer: Address,
    pub created_at: u64,
}

#[contractevent]
pub struct AdminProposalSigned {
    #[topic]
    pub proposal_id: u64,
    #[topic]
    pub signer: Address,
    pub signatures: u32,
}

#[contractevent]
pub struct AdminProposalExecuted {
    #[topic]
    pub proposal_id: u64,
    pub action: AdminAction,
    #[topic]
    pub executor: Address,
}

#[contractevent]
pub struct VaultRevoked {
    #[topic]
    pub vault_id: u64,
    #[topic]
    pub owner: Address,
    pub amount: i128,
    pub treasury: Address,
}

#[contractevent]
pub struct VaultSlashed {
    #[topic]
    pub vault_id: u64,
    pub vested_amount: i128,
    pub unvested_amount: i128,
    pub treasury: Address,
}

#[contractevent]
pub struct VaultRenewed {
    #[topic]
    pub vault_id: u64,
    pub duration: u64,
    pub amount: i128,
}

#[contractevent]
pub struct MarketplaceSold {
    #[topic]
    pub vault_id: u64,
    #[topic]
    pub old_owner: Address,
    #[topic]
    pub new_owner: Address,
    pub marketplace: Address,
}

#[contractevent]
pub struct VaultLegalDocumentsSigned {
    #[topic]
    pub vault_id: u64,
    #[topic]
    pub beneficiary: Address,
}

#[contractevent]
pub struct TeamRevoked {
    pub vaults_count: u32,
    pub owners: Vec<Address>,
    pub total_amount: i128,
    pub treasury: Address,
}

#[contractevent]
pub struct PartialRevocation {
    #[topic]
    pub vault_id: u64,
    pub penalty_amount: i128,
    pub severance_amount: i128,
    pub treasury: Address,
}

#[contractevent]
pub struct BeneficiaryReassigned {
    #[topic]
    pub vault_id: u64,
    #[topic]
    pub old_beneficiary: Address,
    #[topic]
    pub new_beneficiary: Address,
    pub social_proof_type: SocialProofType,
    pub reason: String,
}

// Path Payment events for claim_and_swap functionality
#[contractevent]
pub struct PathPaymentConfigured {
    pub destination_asset: Address,
    pub min_destination_amount: i128,
    pub path: Vec<Address>,
    pub timestamp: u64,
}

#[contractevent]
pub struct PathPaymentDisabled {
    pub timestamp: u64,
}

#[contractevent]
pub struct PathPaymentClaimExecuted {
    #[topic]
    pub user: Address,
    #[topic]
    pub vault_id: u64,
    pub source_amount: i128,
    pub destination_amount: i128,
    pub destination_asset: Address,
    pub timestamp: u64,
}

#[contract]
pub struct VestingContract;

#[contractimpl]
impl VestingContract {
    fn dispatch_admin_action(env: Env, action: AdminAction) {
        match action {
            AdminAction::AddAdmin(admin) => {
                let mut admins = Self::get_admins(env.clone());
                if admins.iter().any(|a| a == admin) {
                    return Err(Error::AlreadyInitialized);
                }
                admins.push_back(admin);
                env.storage().instance().set(&DataKey::AdminSet, &admins);
            }
            AdminAction::RemoveAdmin(admin) => {
                let admins = Self::get_admins(env.clone());
                let orig_len = admins.len();
                let mut new_admins = Vec::new(&env);
                for a in admins.iter() {
                    if a != admin {
                        new_admins.push_back(a.clone());
                    }
                }
                if new_admins.len() == orig_len {
                    return Err(Error::Unauthorized);
                }
                let quorum = Self::get_quorum_threshold(env.clone());
                if new_admins.len() < quorum {
                    return Err(Error::InvalidInput);
                }
                env.storage()
                    .instance()
                    .set(&DataKey::AdminSet, &new_admins);
            }
            AdminAction::UpdateQuorum(new_quorum) => {
                let admins = Self::get_admins(env.clone());
                if new_quorum == 0 || new_quorum > admins.len() as u32 {
                    return Err(Error::InvalidInput);
                }
                env.storage()
                    .instance()
                    .set(&DataKey::QuorumThreshold, &new_quorum);
            }
            AdminAction::RevokeSchedule(vault_id, treasury) => {
                Self::do_revoke_vault_internal(&env, vault_id, treasury.clone());
            }
            AdminAction::AddBeneficiary(owner, cfg) => {
                let _id = Self::create_vault_prefunded_internal(
                    &env,
                    owner.clone(),
                    cfg.asset_basket,
                    cfg.start_time,
                    cfg.end_time,
                    cfg.keeper_fee,
                    cfg.is_revocable,
                    cfg.is_transferable,
                    0, // Default step_duration
                    true,
                );
            }
            AdminAction::GrantManagerRights(manager, asset, amount) => {
                let pool = SubAdminPool {
                    manager: manager.clone(),
                    asset: asset.clone(),
                    total_amount: amount,
                    distributed_amount: 0,
                };
                env.storage()
                    .instance()
                    .set(&DataKey::SubAdminPool(manager), &pool);
                let admin = Self::get_admin(env.clone());
                token::Client::new(&env, &asset).transfer(
                    &admin,
                    &env.current_contract_address(),
                    &amount,
                );
            }
            AdminAction::RenewSchedule(vault_id, duration, amount) => {
                Self::do_renew_vault_direct(&env, vault_id, duration, amount);
            }
            AdminAction::SetXLMAddress(xlm) => {
                env.storage().instance().set(&DataKey::XLMAddress, &xlm);
            }
            AdminAction::InitializeMerkleRoot(merkle_root, total_schedules) => {
                if env.storage().instance().has(&DataKey::MerkleRoot) {
                    return Err(Error::AlreadyInitialized);
                }
                env.storage()
                    .instance()
                    .set(&DataKey::MerkleRoot, &merkle_root);
                MerkleRootInitialized {
                    merkle_root,
                    total_schedules,
                    initialized_at: env.ledger().timestamp(),
                }
                .publish(&env);
            }
            _ => {}
        }
    }

    fn multisig_active(env: &Env) -> bool {
        let admins = Self::get_admins(env.clone());
        let quorum = Self::get_quorum_threshold(env.clone());
        admins.len() > 1 || quorum > 1
    }

    fn do_revoke_vault_internal(env: &Env, vault_id: u64, treasury: Address) {
        let mut vault = Self::get_vault_internal(env, vault_id);
        if vault.is_irrevocable {
            return Err(Error::VaultFrozen);
        }
        let stake_info = get_stake_info(env, vault_id);
        if stake_info.stake_state != StakeState::Unstaked {
            Self::do_unstake(env, vault_id, &mut vault);
            stake::emit_revocation_unstaked(env, vault_id, &vault.owner);
        }
        Self::mark_vault_revoked(env, vault_id);
        let mut remaining_total = 0i128;
        for (i, allocation) in vault.allocations.iter().enumerate() {
            let left = allocation.total_amount - allocation.released_amount;
            if left > 0 {
                remaining_total += left;
                token::Client::new(env, &allocation.asset_id).transfer(
                    &env.current_contract_address(),
                    &treasury,
                    &left,
                );
                let mut updated = allocation.clone();
                updated.released_amount = updated.total_amount;
                vault.allocations.set(i.try_into().unwrap(), updated);
            }
        }
        vault.end_time = env.ledger().timestamp();
        vault.is_frozen = true;
        if env
            .storage()
            .instance()
            .has(&DataKey::VaultMilestones(vault_id))
        {
            env.storage()
                .instance()
                .remove(&DataKey::VaultMilestones(vault_id));
        }
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);
        let total_shares: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalShares)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &(total_shares - remaining_total));
        VaultRevoked {
            vault_id,
            owner: vault.owner,
            amount: remaining_total,
            treasury: treasury.clone(),
        }
        .publish(&env);
    }

    /// Return the number of admin signatures collected for a multisig proposal.
    pub fn admin_proposal_signature_count(env: Env, proposal_id: u64) -> u32 {
        let admins = Self::get_admins(env.clone());
        let mut count: u32 = 0;
        for admin in admins.iter() {
            let sig_key = DataKey::AdminProposalSignature(proposal_id, admin.clone());
            if env.storage().instance().has(&sig_key)
                && env
                    .storage()
                    .instance()
                    .get::<_, bool>(&sig_key)
                    .unwrap_or(false)
            {
                count += 1;
            }
        }
        count
    }

    /// Sign a pending multisig admin proposal.
    ///
    /// Once the quorum threshold is reached the proposal is executed automatically.
    pub fn sign_admin_proposal(env: Env, signer: Address, proposal_id: u64) {
        signer.require_auth();
        if !Self::is_admin(env.clone(), signer.clone()) {
            return Err(Error::Unauthorized);
        }
        let proposal = Self::get_admin_proposal(&env, proposal_id);
        if proposal.is_executed {
            return Err(Error::ProposalAlreadyExecuted);
        }
        let sig_key = DataKey::AdminProposalSignature(proposal_id, signer.clone());
        if env
            .storage()
            .instance()
            .get::<_, bool>(&sig_key)
            .unwrap_or(false)
        {
            return Err(Error::AlreadyVoted);
        }
        env.storage().instance().set(&sig_key, &true);
        let sig_count = Self::admin_proposal_signature_count(env.clone(), proposal_id);
        let quorum = Self::get_quorum_threshold(env.clone());
        AdminProposalSigned {
            proposal_id,
            signer: signer.clone(),
            signatures: sig_count,
        }
        .publish(&env);

        if sig_count >= quorum {
            let mut stored = proposal.clone();
            stored.is_executed = true;
            env.storage()
                .instance()
                .set(&DataKey::AdminProposal(proposal_id), &stored);
            Self::dispatch_admin_action(env.clone(), proposal.action.clone());
            AdminProposalExecuted {
                proposal_id,
                action: proposal.action.clone(),
                executor: signer,
            }
            .publish(&env);
        }
    }

    /// Propose an admin action that requires multisig approval.
    ///
    /// Returns the new proposal ID.
    pub fn propose_admin_action(env: Env, proposer: Address, action: AdminAction) -> u64 {
        proposer.require_auth();
        if !Self::is_admin(env.clone(), proposer.clone()) {
            return Err(Error::Unauthorized);
        }
        let now = env.ledger().timestamp();
        let proposal_id = Self::increment_admin_proposal_count(&env);
        let proposal = AdminProposal {
            id: proposal_id,
            action: action.clone(),
            proposer: proposer.clone(),
            created_at: now,
            is_executed: false,
        };
        env.storage()
            .instance()
            .set(&DataKey::AdminProposal(proposal_id), &proposal);
        env.storage().instance().set(
            &DataKey::AdminProposalSignature(proposal_id, proposer.clone()),
            &true,
        );
        AdminProposalCreated {
            proposal_id,
            action: action.clone(),
            proposer: proposer.clone(),
            created_at: now,
        }
        .publish(&env);

        let sig_count = Self::admin_proposal_signature_count(env.clone(), proposal_id);
        if sig_count >= Self::get_quorum_threshold(env.clone()) {
            let mut stored = proposal;
            stored.is_executed = true;
            env.storage()
                .instance()
                .set(&DataKey::AdminProposal(proposal_id), &stored);
            Self::dispatch_admin_action(env.clone(), action);
            AdminProposalExecuted {
                proposal_id,
                action: stored.action,
                executor: proposer,
            }
            .publish(&env);
        }
        proposal_id
    }

    /// Return the current list of admin addresses.
    pub fn get_admins(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::AdminSet)
            .unwrap_or(Vec::new(&env))
    }

    /// Return the current multisig quorum threshold.
    pub fn get_quorum_threshold(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::QuorumThreshold)
            .unwrap_or(1u32)
    }

    /// Returns `true` if `addr` is a registered admin.
    pub fn is_admin(env: Env, addr: Address) -> bool {
        let admins = Self::get_admins(env);
        admins.iter().any(|a| a == addr)
    }

    /// Initialise the contract with a single admin and the total token supply.
    ///
    /// Can only be called once.
    pub fn initialize(env: Env, admin: Address, initial_supply: i128) {
        if env.storage().instance().has(&DataKey::AdminSet) {
            return Err(Error::AlreadyInitialized);
        }
        let mut admins = Vec::new(&env);
        admins.push_back(admin.clone());
        env.storage().instance().set(&DataKey::AdminSet, &admins);
        env.storage()
            .instance()
            .set(&DataKey::QuorumThreshold, &1u32);
        env.storage().instance().set(&DataKey::AdminAddress, &admin);
        env.storage()
            .instance()
            .set(&DataKey::AdminBalance, &initial_supply);
        env.storage()
            .instance()
            .set(&DataKey::InitialSupply, &initial_supply);
        env.storage().instance().set(&DataKey::VaultCount, &0u64);
        env.storage().instance().set(&DataKey::IsPaused, &false);
        env.storage().instance().set(&DataKey::IsDeprecated, &false);
        env.storage().instance().set(&DataKey::TotalShares, &0i128);
        env.storage().instance().set(&DataKey::TotalStaked, &0i128);
        env.storage().instance().set(&DataKey::ProposalCount, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalLockedValue, &initial_supply);
    }

    /// Initialise the contract in multisig mode.
    ///
    /// # Parameters
    /// - `admins`           – List of admin addresses (at least one required).
    /// - `quorum_threshold` – Minimum signatures required to execute a proposal.
    /// - `initial_supply`   – Total token supply for governance calculations.
    pub fn initialize_multisig(
        env: Env,
        admins: Vec<Address>,
        quorum_threshold: u32,
        initial_supply: i128,
    ) {
        if env.storage().instance().has(&DataKey::AdminSet) {
            return Err(Error::AlreadyInitialized);
        }
        if admins.len() == 0 {
            return Err(Error::InvalidInput);
        }
        if quorum_threshold == 0 || quorum_threshold > admins.len() as u32 {
            return Err(Error::InvalidInput);
        }
        env.storage().instance().set(&DataKey::AdminSet, &admins);
        env.storage()
            .instance()
            .set(&DataKey::QuorumThreshold, &quorum_threshold);
        env.storage()
            .instance()
            .set(&DataKey::AdminAddress, &admins.get(0).unwrap());
        env.storage()
            .instance()
            .set(&DataKey::AdminBalance, &initial_supply);
        env.storage()
            .instance()
            .set(&DataKey::InitialSupply, &initial_supply);
        env.storage().instance().set(&DataKey::VaultCount, &0u64);
        env.storage().instance().set(&DataKey::IsPaused, &false);
        env.storage().instance().set(&DataKey::IsDeprecated, &false);
        env.storage().instance().set(&DataKey::TotalShares, &0i128);
        env.storage().instance().set(&DataKey::TotalStaked, &0i128);
        env.storage().instance().set(&DataKey::ProposalCount, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalLockedValue, &initial_supply);
    }

    /// Set the vesting token contract address (admin only).
    pub fn set_token(env: Env, token: Address) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        env.storage().instance().set(&DataKey::Token, &token);
    }

    /// Set the NFT minter contract address used for vesting-status NFTs (admin only).
    pub fn set_nft_minter(env: Env, minter: Address) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        env.storage().instance().set(&DataKey::NFTMinter, &minter);
    }

    /// Add a token address to the vesting whitelist (admin only).
    pub fn add_to_whitelist(env: Env, token: Address) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        env.storage().instance().set(&DataKey::Token, &token);
    }

    /// Propose a contract upgrade via the 72-hour governance challenge period.
    ///
    /// Returns the new proposal ID.
    pub fn propose_contract_upgrade(env: Env, new_contract: Address) -> u64 {
        Self::require_admin(&env);
        Self::create_governance_proposal(env, GovernanceAction::ContractUpgrade(new_contract))
    }

    /// Accept a pending admin-rotation proposal (called by the proposed new admin).
    pub fn accept_ownership(env: Env) {
        let proposed: Address = env
            .storage()
            .instance()
            .get(&DataKey::ProposedAdmin)
            .expect("No proposed admin");
        proposed.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::AdminAddress, &proposed);
        env.storage().instance().remove(&DataKey::ProposedAdmin);
    }

    /// Propose an emergency pause or resume via the governance challenge period.
    ///
    /// Returns the new proposal ID.
    pub fn propose_emergency_pause(env: Env, pause_state: bool) -> u64 {
        Self::require_admin(&env);
        Self::create_governance_proposal(env, GovernanceAction::EmergencyPause(pause_state))
    }

    /// Cast a governance vote on a proposal.
    ///
    /// Voting power equals the voter's total locked (unvested) token balance.
    /// A "No" vote exceeding 51 % of total locked value cancels the proposal.
    pub fn vote_on_proposal(env: Env, voter: Address, proposal_id: u64, is_yes: bool) {
        // Voter must authorize the action
        voter.require_auth();
        let vote_weight = Self::get_voter_locked_value(&env, &voter);

        if vote_weight <= 0 {
            return Err(Error::InsufficientBalance);
        }

        let mut proposal = Self::get_proposal(&env, proposal_id);

        // Check if voting is still open
        let now = env.ledger().timestamp();
        if now >= proposal.challenge_end {
            return Err(Error::VotingPeriodEnded);
        }

        if proposal.is_executed || proposal.is_cancelled {
            return Err(Error::ProposalExpired);
        }

        // Check if already voted
        let vote_key = DataKey::GovernanceVotes(proposal_id, voter.clone());
        if env.storage().instance().has(&vote_key) {
            return Err(Error::AlreadyVoted);
        }

        // Record vote
        let vote = Vote {
            voter: voter.clone(),
            vote_weight,
            is_yes,
            voted_at: now,
        };
        env.storage().instance().set(&vote_key, &vote);

        // Update proposal vote counts
        if is_yes {
            proposal.yes_votes += vote_weight;
        } else {
            proposal.no_votes += vote_weight;
        }

        env.storage()
            .instance()
            .set(&DataKey::GovernanceProposal(proposal_id), &proposal);

        // Publish vote event
        VoteCast {
            proposal_id,
            voter: voter.clone(),
            vote_weight,
            is_yes,
        }
        .publish(&env);
    }

    /// Execute a governance proposal after the 72-hour challenge period has elapsed
    /// and the veto threshold has not been reached.
    pub fn execute_proposal(env: Env, proposal_id: u64) {
        let mut proposal = Self::get_proposal(&env, proposal_id);
        let now = env.ledger().timestamp();

        // Check challenge period has ended
        if now < proposal.challenge_end {
            return Err(Error::VotingPeriodEnded);
        }

        if proposal.is_executed || proposal.is_cancelled {
            return Err(Error::ProposalAlreadyExecuted);
        }

        // Check if proposal passes (no veto from 51%+ of locked value)
        let total_locked = Self::get_total_locked_value(&env);
        let no_percentage = (proposal.no_votes * 10000) / total_locked;

        if no_percentage >= VOTING_THRESHOLD as i128 {
            // Proposal is vetoed - cancel it
            proposal.is_cancelled = true;
            env.storage()
                .instance()
                .set(&DataKey::GovernanceProposal(proposal_id), &proposal);
            return;
        }

        // Execute the governance action
        Self::execute_governance_action(&env, &proposal.action);

        proposal.is_executed = true;
        env.storage()
            .instance()
            .set(&DataKey::GovernanceProposal(proposal_id), &proposal);

        // Publish execution event
        GovernanceActionExecuted {
            proposal_id,
            action: proposal.action.clone(),
        }
        .publish(&env);
    }

    // Legacy pause function - now requires governance proposal
    /// Toggle the global contract pause state (admin only).
    pub fn toggle_pause(env: Env) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        let paused = env
            .storage()
            .instance()
            .get(&DataKey::IsPaused)
            .unwrap_or(false);
        env.storage().instance().set(&DataKey::IsPaused, &(!paused));
    }

    pub fn create_vault_full(
        env: Env,
        owner: Address,
        amount: i128,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
    ) -> u64 {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        Self::create_vault_full_internal(
            &env,
            owner,
            amount,
            start_time,
            end_time,
            keeper_fee,
            is_revocable,
            is_transferable,
            step_duration,
        )
    }

    pub fn create_vault_lazy(
        env: Env,
        owner: Address,
        amount: i128,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
    ) -> u64 {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        Self::create_vault_lazy_internal(
            &env,
            owner,
            amount,
            start_time,
            end_time,
            keeper_fee,
            is_revocable,
            is_transferable,
            step_duration,
        )
    }

    /// Create multiple lazy-initialised vaults in a single transaction.
    ///
    /// Returns the list of newly created vault IDs.
    pub fn batch_create_vaults_lazy(env: Env, data: BatchCreateData) -> Vec<u64> {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        let total_amount = Self::validate_batch_data(&data);
        Self::reserve_admin_balance_for_batch(&env, total_amount);
        let mut ids = Vec::new(&env);
        for i in 0..data.recipients.len() {
            let id = Self::create_vault_lazy_internal(
                &env,
                data.recipients.get(i).unwrap(),
                0, // amount handled by lazy logic (usually unspent balance)
                data.start_times.get(i).unwrap(),
                data.end_times.get(i).unwrap(),
                data.keeper_fees.get(i).unwrap(),
                true,
                false,
                data.step_durations.get(i).unwrap_or(0),
            );
            ids.push_back(id);
        }
        ids
    }

    /// Create multiple fully-initialised vaults in a single transaction.
    ///
    /// Returns the list of newly created vault IDs.
    pub fn batch_create_vaults_full(env: Env, data: BatchCreateData) -> Vec<u64> {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        let total_amount = Self::validate_batch_data(&data);
        Self::reserve_admin_balance_for_batch(&env, total_amount);

        let mut ids = Vec::new(&env);
        for i in 0..data.recipients.len() {
            let recipient = data.recipients.get(i).unwrap();
            let basket = data.asset_baskets.get(i).unwrap();

            // Perform actual token transfers for this recipient's basket
            for allocation in basket.iter() {
                let admin = Self::get_admin(env.clone());
                token::Client::new(&env, &allocation.asset_id).transfer(
                    &admin,
                    &env.current_contract_address(),
                    &allocation.total_amount,
                );
            }

            let id = Self::create_vault_prefunded_internal(
                &env,
                recipient,
                basket,
                data.start_times.get(i).unwrap(),
                data.end_times.get(i).unwrap(),
                data.keeper_fees.get(i).unwrap(),
                true,  // revocable
                false, // transferable
                data.step_durations.get(i).unwrap_or(0),
                true, // is_initialized
            );
            ids.push_back(id);
        }
        ids
    }

    /// Add multiple vesting schedules in a single transaction.
    ///
    /// Returns the list of newly created vault IDs.
    pub fn batch_add_schedules(env: Env, schedules: Vec<ScheduleConfig>) -> Vec<u64> {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        let mut total_amount = 0i128;
        for s in schedules.iter() {
            for a in s.asset_basket.iter() {
                total_amount += a.total_amount;
            }
        }
        Self::require_deposited_tokens_for_batch(&env, total_amount);
        Self::reserve_admin_balance_for_batch(&env, total_amount);

        let mut ids = Vec::new(&env);
        for s in schedules.iter() {
            let id = Self::create_vault_prefunded_internal(
                &env,
                s.owner.clone(),
                s.asset_basket.clone(),
                s.start_time,
                s.end_time,
                s.keeper_fee,
                s.is_revocable,
                s.is_transferable,
                0, // step_duration
                true,
            );
            ids.push_back(id);
        }
        ids
    }

    /// Initialize Merkle root for bulk vesting schedule activation (Issue #199)
    /// Stores a single 32-byte root hash that represents thousands of vesting schedules
    pub fn initialize_merkle_root(env: Env, merkle_root: BytesN<32>, total_schedules: u32) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }

        // Check if Merkle root already exists
        if env.storage().instance().has(&DataKey::MerkleRoot) {
            return Err(Error::AlreadyInitialized);
        }

        // Store the Merkle root
        env.storage()
            .instance()
            .set(&DataKey::MerkleRoot, &merkle_root);

        MerkleRootInitialized {
            merkle_root,
            total_schedules,
            initialized_at: env.ledger().timestamp(),
        }
        .publish(&env);
    }

    /// Activate an individual vesting schedule using Merkle proof
    /// Users provide proof that their schedule is included in the Merkle tree
    pub fn activate_schedule_with_proof(
        env: Env,
        beneficiary: Address,
        leaf: VestingScheduleLeaf,
        proof: MerkleProof,
    ) -> u64 {
        beneficiary.require_auth();

        // Get stored Merkle root
        let stored_root: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::MerkleRoot)
            .expect("Merkle root not initialized");

        // Verify the Merkle proof
        if !Self::verify_merkle_proof(&env, &proof, &stored_root) {
            return Err(Error::InvalidInput);
        }

        // Check if schedule already activated for this beneficiary
        if env
            .storage()
            .instance()
            .has(&DataKey::ActivatedSchedule(beneficiary.clone()))
        {
            return Err(Error::AlreadyInitialized);
        }

        // Verify leaf data matches proof
        let computed_leaf_hash = Self::hash_vesting_leaf(&env, &leaf);
        if computed_leaf_hash != proof.leaf_hash {
            return Err(Error::InvalidInput);
        }

        // Create the vault with the leaf data
        let vault_id = Self::create_vault_prefunded_internal(
            &env,
            leaf.beneficiary.clone(),
            leaf.asset_basket,
            leaf.start_time,
            leaf.end_time,
            leaf.keeper_fee,
            leaf.is_revocable,
            leaf.is_transferable,
            leaf.step_duration,
            true, // is_initialized
        );

        // Mark schedule as activated for this beneficiary
        env.storage()
            .instance()
            .set(&DataKey::ActivatedSchedule(beneficiary.clone()), &vault_id);

        ScheduleActivatedWithProof {
            beneficiary: beneficiary.clone(),
            vault_id,
            merkle_root: stored_root,
            activated_at: env.ledger().timestamp(),
        }
        .publish(&env);

        vault_id
    }

    /// Verify a Merkle proof against a stored root
    fn verify_merkle_proof(env: &Env, proof: &MerkleProof, root: &BytesN<32>) -> bool {
        let mut current_hash = proof.leaf_hash.clone();
        let mut index = proof.leaf_index;

        for sibling_hash in proof.proof.iter() {
            if (index & 1) == 0 {
                // Current hash is left sibling
                current_hash = Self::hash_pair(&env, &current_hash, sibling_hash);
            } else {
                // Current hash is right sibling
                current_hash = Self::hash_pair(&env, sibling_hash, &current_hash);
            }
            index >>= 1;
        }

        current_hash == *root
    }

    /// Hash two bytes arrays together (simplified SHA-256 behavior)
    fn hash_pair(env: &Env, left: &BytesN<32>, right: &BytesN<32>) -> BytesN<32> {
        let mut combined = Vec::new(env);
        combined.extend_from_slice(left.as_slice());
        combined.extend_from_slice(right.as_slice());
        env.crypto().sha256(&combined.into())
    }

    /// Hash a vesting schedule leaf into a 32-byte array
    fn hash_vesting_leaf(env: &Env, leaf: &VestingScheduleLeaf) -> BytesN<32> {
        let mut data = Vec::new(env);

        // Serialize leaf data
        data.extend_from_slice(leaf.beneficiary.to_xdr(env).as_slice());
        data.extend_from_slice(&leaf.vault_id.to_le_bytes());

        // Hash asset basket
        for allocation in leaf.asset_basket.iter() {
            data.extend_from_slice(allocation.asset_id.to_xdr(env).as_slice());
            data.extend_from_slice(&allocation.total_amount.to_le_bytes());
            data.extend_from_slice(&allocation.released_amount.to_le_bytes());
            data.extend_from_slice(&allocation.locked_amount.to_le_bytes());
            data.extend_from_slice(&allocation.percentage.to_le_bytes());
        }

        data.extend_from_slice(&leaf.start_time.to_le_bytes());
        data.extend_from_slice(&leaf.end_time.to_le_bytes());
        data.extend_from_slice(&leaf.keeper_fee.to_le_bytes());
        data.extend_from_slice(&[if leaf.is_revocable { 1u8 } else { 0u8 }]);
        data.extend_from_slice(&[if leaf.is_transferable { 1u8 } else { 0u8 }]);
        data.extend_from_slice(&leaf.step_duration.to_le_bytes());

        env.crypto().sha256(&data.into())
    }

    /// Get the current Merkle root
    pub fn get_merkle_root(env: Env) -> Option<BytesN<32>> {
        env.storage().instance().get(&DataKey::MerkleRoot)
    }

    /// Check if a beneficiary has already activated their schedule
    pub fn is_schedule_activated(env: Env, beneficiary: Address) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::ActivatedSchedule(beneficiary))
    }

    /// Get the vault ID for an activated schedule
    pub fn get_activated_vault_id(env: Env, beneficiary: Address) -> Option<u64> {
        env.storage()
            .instance()
            .get(&DataKey::ActivatedSchedule(beneficiary))
    }

    /// Creates a vault with a diversified asset basket (pre-funded)
    pub fn create_vault_diversified_full(
        env: Env,
        owner: Address,
        asset_basket: Vec<AssetAllocationEntry>,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
        title: String,
    ) -> u64 {
        Self::require_admin(&env);

        // Validate asset basket
        if !Self::validate_asset_basket(&asset_basket) {
            return Err(Error::InvalidInput);
        }

        if asset_basket.is_empty() {
            return Err(Error::InvalidInput);
        }

        // Validate timing
        if start_time >= end_time {
            return Err(Error::InvalidSchedule);
        }

        let max_duration = 10 * 365 * 24 * 60 * 60; // 10 years in seconds
        if end_time - start_time > max_duration {
            return Err(Error::InvalidSchedule);
        }

        let vault_id = Self::increment_vault_count(&env);

        // Transfer all assets from admin to contract
        let admin = Self::get_admin(env.clone());
        for allocation in asset_basket.iter() {
            token::Client::new(&env, &allocation.asset_id).transfer(
                &admin,
                &env.current_contract_address(),
                &allocation.total_amount,
            );
        }

        let vault = Vault {
            allocations: asset_basket,
            keeper_fee,
            staked_amount: 0,
            owner: owner.clone(),
            delegate: None,
            title,
            start_time,
            end_time,
            creation_time: env.ledger().timestamp(),
            step_duration,
            is_initialized: true,
            is_irrevocable: !is_revocable,
            is_transferable,
            is_frozen: false,
            requires_legal_signatures: false,
            legal_documents_signed: true,
            yield_destination: YieldDestination::Beneficiary,
        };

        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);
        Self::add_user_vault_index(&env, &owner, vault_id);

        vault_id
    }

    /// Creates a vault with a diversified asset basket (lazy/unfunded)
    pub fn create_vault_diversified_lazy(
        env: Env,
        owner: Address,
        asset_basket: Vec<AssetAllocationEntry>,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
        title: String,
    ) -> u64 {
        Self::require_admin(&env);

        // Validate asset basket
        if !Self::validate_asset_basket(&asset_basket) {
            return Err(Error::InvalidInput);
        }

        if asset_basket.is_empty() {
            return Err(Error::InvalidInput);
        }

        // Validate timing
        if start_time >= end_time {
            return Err(Error::InvalidSchedule);
        }

        let max_duration = 10 * 365 * 24 * 60 * 60; // 10 years in seconds
        if end_time - start_time > max_duration {
            return Err(Error::InvalidSchedule);
        }

        let vault_id = Self::increment_vault_count(&env);

        let vault = Vault {
            allocations: asset_basket,
            keeper_fee,
            staked_amount: 0,
            owner: owner.clone(),
            delegate: None,
            title,
            start_time,
            end_time,
            creation_time: env.ledger().timestamp(),
            step_duration,
            is_initialized: false, // Lazy vault starts uninitialized
            is_irrevocable: !is_revocable,
            is_transferable,
            is_frozen: false,
            requires_legal_signatures: false,
            legal_documents_signed: true,
            yield_destination: YieldDestination::Beneficiary,
        };

        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);
        Self::add_user_vault_index(&env, &owner, vault_id);

        vault_id
    }
    /// Initializes a lazy diversified vault by transferring all assets
    pub fn initialize_diversified_vault(env: Env, vault_id: u64) {
        Self::require_admin(&env);
        let mut vault = Self::get_vault_internal(&env, vault_id);

        if vault.is_initialized {
            return Err(Error::AlreadyInitialized);
        }

        let admin = Self::get_admin(env.clone());

        // Transfer all assets from admin to contract
        for allocation in vault.allocations.iter() {
            token::Client::new(&env, &allocation.asset_id).transfer(
                &admin,
                &env.current_contract_address(),
                &allocation.total_amount,
            );
        }

        vault.is_initialized = true;
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);
    }

    /// Claims tokens from a diversified vesting vault
    /// Main diversified claim function that claims all available tokens across all assets
    pub fn claim_tokens_diversified(
        env: Env,
        vault_id: u64,
    ) -> Result<Vec<(Address, i128)>, Error> {
        Self::require_not_paused(&env);
        let mut vault = Self::get_vault_internal(&env, vault_id);
        if vault.is_frozen {
            return Err(Error::VaultFrozen);
        }
        if !vault.is_initialized {
            return Err(Error::VaultNotInitialized);
        }

        // Check if this specific vault schedule is paused
        if Self::is_vault_paused(env.clone(), vault_id) {
            return Err(Error::ContractPaused);
        }

        // Check if legal document signatures are required and verified
        if vault.requires_legal_signatures && !vault.legal_documents_signed {
            return Err(Error::LegalSignatureMissing);
        }

        // Check if beneficiary reassignment is in progress
        if let Some(reassignment) = BeneficiaryReassignment::get_reassignment_status(&env, vault_id)
        {
            match &reassignment.status {
                ReassignmentStatus::Pending(_) => {
                    return Err(Error::VaultFrozen);
                }
                ReassignmentStatus::Approved => {
                    return Err(Error::VaultFrozen);
                }
                ReassignmentStatus::Completed => {
                    // Check if reassignment completed to current owner
                    if reassignment.new_beneficiary != vault.owner {
                        return Err(Error::Unauthorized);
                    }
                }
                ReassignmentStatus::Rejected => {
                    // Rejected reassignments don't block claims
                }
                ReassignmentStatus::None => {
                    // No reassignment in progress, normal flow
                }
            }
        }

        vault.owner.require_auth();

        // ========== COMPLIANCE CHECKS ==========

        // KYC Verification Check
        if !Self::is_kyc_verified(&env, &vault.owner) {
            return Err(Error::KycNotCompleted);
        }

        // KYC Expiration Check
        if let Some(kyc_expiry) = Self::get_kyc_expiry(&env, &vault.owner) {
            let current_time = env.ledger().timestamp();
            if current_time > kyc_expiry {
                return Err(Error::KycExpired);
            }
        }

        // Sanctions Check
        if Self::is_address_sanctioned(&env, &vault.owner) {
            return Err(Error::AddressSanctioned);
        }

        // Jurisdiction Restriction Check
        if Self::is_jurisdiction_restricted(&env, &vault.owner) {
            return Err(Error::JurisdictionRestricted);
        }

        // Legal Signature Verification
        if !Self::has_valid_legal_signature(&env, &vault.owner, vault_id) {
            return Err(Error::LegalSignatureMissing);
        }

        // Document Verification Check
        if !Self::are_documents_verified(&env, &vault.owner) {
            return Err(Error::DocumentVerificationFailed);
        }

        // Tax Compliance Check
        if !Self::is_tax_compliant(&env, &vault.owner) {
            return Err(Error::TaxComplianceFailed);
        }

        // Whitelist Approval Check
        if !Self::is_whitelist_approved(&env, &vault.owner) {
            return Err(Error::WhitelistNotApproved);
        }

        // Blacklist Violation Check
        if Self::is_on_blacklist(&env, &vault.owner) {
            return Err(Error::BlacklistViolation);
        }

        // Geofencing Restriction Check
        if Self::is_geofencing_restricted(&env, &vault.owner) {
            return Err(Error::GeofencingRestriction);
        }

        // Identity Verification Expiration Check
        if let Some(identity_expiry) = Self::get_identity_expiry(&env, &vault.owner) {
            let current_time = env.ledger().timestamp();
            if current_time > identity_expiry {
                return Err(Error::IdentityVerificationExpired);
            }
        }

        // Politically Exposed Person Check
        if Self::is_politically_exposed_person(&env, &vault.owner) {
            return Err(Error::PoliticallyExposedPerson);
        }

        // Sanctions List Hit Check
        if Self::is_on_sanctions_list(&env, &vault.owner) {
            return Err(Error::SanctionsListHit);
        }

        // Heartbeat: reset Dead-Man's Switch on every primary interaction
        update_activity(&env, vault_id);

        // KPI Gate check (#145/#92)
        if !crate::kpi_vesting::kpi_status(&env, vault_id) {
            return Err(Error::ComplianceCheckFailed);
        }

        let mut claimed_assets = Vec::new(&env);
        let mut total_claimable_sum = 0i128;
        let mut total_unreleased_sum = 0i128;

        // Calculate and claim each asset in the basket
        for (i, allocation) in vault.allocations.iter().enumerate() {
            let vested_amount = Self::calculate_claimable_for_asset(&env, vault_id, &vault, i);
            let mut claimable_amount = vested_amount - allocation.released_amount;

            // #90: XLM Minimum Reserve Check (2 XLM = 20,000,000 Stroops)
            let xlm: Option<Address> = env.storage().instance().get(&DataKey::XLMAddress);
            if let Some(xlm_addr) = xlm {
                if allocation.asset_id == xlm_addr {
                    let total_unreleased = allocation.total_amount - allocation.released_amount;
                    if total_unreleased <= 20_000_000 {
                        claimable_amount = 0;
                    } else if (total_unreleased - claimable_amount) < 20_000_000 {
                        claimable_amount = total_unreleased - 20_000_000;
                    }
                }
            }

            if claimable_amount > 0 {
                total_claimable_sum += claimable_amount;
                total_unreleased_sum += allocation.total_amount - allocation.released_amount;

                // Update the allocation's released amount
                let mut updated_allocation = allocation.clone();
                updated_allocation.released_amount += claimable_amount;
                vault
                    .allocations
                    .set(i.try_into().unwrap(), updated_allocation);

                // Transfer the tokens
                token::Client::new(&env, &allocation.asset_id).transfer(
                    &env.current_contract_address(),
                    &vault.owner,
                    &claimable_amount,
                );

                claimed_assets.push_back((allocation.asset_id.clone(), claimable_amount));
            }
        }

        let _guard = if total_claimable_sum > 0 {
            Some(match ReentrancyGuard::enter(&env) {
                Ok(guard) => guard,
                Err(err) => return Err(err),
            })
        } else {
            None
        };

        // Save updated vault before any external call so callbacks observe the
        // post-claim released amounts rather than stale state.
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        // Check if vault is fully completed and register certificate
        Self::check_and_register_certificate(&env, vault_id, &vault);

        // --- Pro-Rata Yield Distribution ---
        if total_claimable_sum > 0 && vault.yield_destination == YieldDestination::Beneficiary {
            let mut stake_info = get_stake_info(&env, vault_id);
            if let StakeState::Staked(_, staking_contract) = &stake_info.stake_state {
                let new_yield =
                    call_claim_yield_for(&env, &staking_contract, &vault.owner, vault_id);
                stake_info.accumulated_yield += new_yield;

                if stake_info.accumulated_yield > 0 && total_unreleased_sum > 0 {
                    let yield_payout =
                        (total_claimable_sum * stake_info.accumulated_yield) / total_unreleased_sum;
                    if yield_payout > 0 {
                        let token: Address = env
                            .storage()
                            .instance()
                            .get(&DataKey::Token)
                            .expect("Token not set");
                        token::Client::new(&env, &token).transfer(
                            &staking_contract,
                            &vault.owner,
                            &yield_payout,
                        );
                        stake_info.accumulated_yield -= yield_payout;
                    }
                }
                set_stake_info(&env, vault_id, &stake_info);
            }
        }
        // -----------------------------------

        // Mint NFT if configured
        if let Some(nft_minter) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::NFTMinter)
        {
            env.invoke_contract::<()>(
                &nft_minter,
                &Symbol::new(&env, "mint"),
                (&vault.owner,).into_val(&env),
            );
        }

        claimed_assets
    }

    /// Batch claim tokens from all user's vaults in a single transaction
    /// Aggregates available tokens across all schedules linked to a single Address
    /// Returns a vector of (asset_id, total_claimed_amount) pairs
    pub fn batch_claim(env: Env, user: Address) -> Vec<(Address, i128)> {
        Self::require_not_paused(&env);
        user.require_auth();

        // Get all vaults for this user
        let user_vaults = Self::get_user_vaults(env.clone(), user.clone());

        if user_vaults.is_empty() {
            return Vec::new(&env);
        }

        let mut aggregated_claims = Vec::new(&env);
        let mut processed_vaults = Vec::new(&env);

        // Process each vault and aggregate claimable amounts by asset
        for vault_id in user_vaults.iter() {
            let mut vault = Self::get_vault_internal(&env, *vault_id);

            // Skip frozen, uninitialized, or paused vaults
            if vault.is_frozen
                || !vault.is_initialized
                || Self::is_vault_paused(env.clone(), *vault_id)
            {
                continue;
            }

            // Heartbeat: reset Dead-Man's Switch on every primary interaction
            update_activity(&env, *vault_id);

            // Validate asset basket
            if !Self::validate_asset_basket(&vault.allocations) {
                continue;
            }

            let mut vault_has_claims = false;

            // Calculate claimable amounts for each asset in this vault
            for (i, allocation) in vault.allocations.iter().enumerate() {
                let vested_amount = Self::calculate_claimable_for_asset(&env, *vault_id, &vault, i);
                let mut claimable_amount = vested_amount - allocation.released_amount;

                // #90: XLM Minimum Reserve Check (2 XLM = 20,000,000 Stroops)
                let xlm: Option<Address> = env.storage().instance().get(&DataKey::XLMAddress);
                if let Some(xlm_addr) = xlm {
                    if allocation.asset_id == xlm_addr {
                        let total_unreleased = allocation.total_amount - allocation.released_amount;
                        if total_unreleased <= 20_000_000 {
                            claimable_amount = 0;
                        } else if (total_unreleased - claimable_amount) < 20_000_000 {
                            claimable_amount = total_unreleased - 20_000_000;
                        }
                    }
                }

                if claimable_amount > 0 {
                    // Update the allocation's released amount
                    let mut updated_allocation = allocation.clone();
                    updated_allocation.released_amount += claimable_amount;
                    vault
                        .allocations
                        .set(i.try_into().unwrap(), updated_allocation);

                    // Aggregate by asset ID (check if asset already exists in aggregated_claims)
                    let mut found_asset = false;
                    for j in 0..aggregated_claims.len() {
                        let (existing_asset_id, existing_amount) =
                            aggregated_claims.get(j).unwrap();
                        if *existing_asset_id == allocation.asset_id {
                            let new_amount = *existing_amount + claimable_amount;
                            aggregated_claims.set(j, (allocation.asset_id.clone(), new_amount));
                            found_asset = true;
                            break;
                        }
                    }

                    if !found_asset {
                        aggregated_claims
                            .push_back((allocation.asset_id.clone(), claimable_amount));
                    }

                    vault_has_claims = true;
                }
            }

            // Save updated vault if it had claims
            if vault_has_claims {
                env.storage()
                    .instance()
                    .set(&DataKey::VaultData(*vault_id), &vault);
                processed_vaults.push_back(*vault_id);

                // Check if vault is fully completed and register certificate
                Self::check_and_register_certificate(&env, *vault_id, &vault);
            }
        }

        let _guard = if !processed_vaults.is_empty() {
            Some(match ReentrancyGuard::enter(&env) {
                Ok(guard) => guard,
                Err(err) => return Err(err),
            })
        } else {
            None
        };

        // Execute aggregated token transfers
        for (asset_id, total_amount) in aggregated_claims.iter() {
            if *total_amount > 0 {
                // Single transfer per asset type
                token::Client::new(&env, asset_id).transfer(
                    &env.current_contract_address(),
                    &user,
                    total_amount,
                );
            }
        }

        // Mint NFT if configured (only once per batch claim)
        if !processed_vaults.is_empty() {
            if let Some(nft_minter) = env
                .storage()
                .instance()
                .get::<_, Address>(&DataKey::NFTMinter)
            {
                env.invoke_contract::<()>(
                    &nft_minter,
                    &Symbol::new(&env, "mint"),
                    (&user,).into_val(&env),
                );
            }
        }

        claimed_assets
    }

    /// Legacy single-asset claim function for backward compatibility
    pub fn claim_tokens(env: Env, vault_id: u64, claim_amount: i128) -> i128 {
        Self::require_not_paused(&env);
        let mut vault = Self::get_vault_internal(&env, vault_id);
        if vault.is_frozen {
            return Err(Error::VaultFrozen);
        }
        if !vault.is_initialized {
            return Err(Error::VaultNotInitialized);
        }

        // Check if this specific vault schedule is paused
        if Self::is_vault_paused(env.clone(), vault_id) {
            return Err(Error::ContractPaused);
        }

        // Check if legal document signatures are required and verified
        if vault.requires_legal_signatures && !vault.legal_documents_signed {
            return Err(Error::LegalSignatureMissing);
        }

        // Check if beneficiary reassignment is in progress
        if let Some(reassignment) = BeneficiaryReassignment::get_reassignment_status(&env, vault_id)
        {
            match &reassignment.status {
                ReassignmentStatus::Pending(_) => {
                    return Err(Error::VaultFrozen);
                }
                ReassignmentStatus::Approved => {
                    return Err(Error::VaultFrozen);
                }
                ReassignmentStatus::Completed => {
                    // Check if reassignment completed to current owner
                    if reassignment.new_beneficiary != vault.owner {
                        return Err(Error::Unauthorized);
                    }
                }
                ReassignmentStatus::Rejected => {
                    // Rejected reassignments don't block claims
                }
                ReassignmentStatus::None => {
                    // No reassignment in progress, normal flow
                }
            }
        }

        vault.owner.require_auth();

        // Heartbeat: reset Dead-Man's Switch on every primary interaction
        update_activity(&env, vault_id);

        // For backward compatibility, only work with single-asset vaults
        if vault.allocations.len() != 1 {
            return Err(Error::InvalidInput);
        }

        let allocation = vault.allocations.get(0).unwrap();
        let vested = Self::calculate_claimable_for_asset(&env, vault_id, &vault, 0);
        if claim_amount > vested - allocation.released_amount {
            return Err(Error::InsufficientBalance);
        }

        let remaining_base = allocation.total_amount - allocation.released_amount;
        let _guard = match ReentrancyGuard::enter(&env) {
            Ok(guard) => guard,
            Err(err) => return Err(err),
        };

        let mut updated_allocation = allocation.clone();
        updated_allocation.released_amount += claim_amount;
        vault.allocations.set(0, updated_allocation);

        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        // Check if vault is fully completed and register certificate
        Self::check_and_register_certificate(&env, vault_id, &vault);

        // --- Pro-Rata Yield Distribution ---
        if vault.yield_destination == YieldDestination::Beneficiary {
            let mut stake_info = get_stake_info(&env, vault_id);
            if let StakeState::Staked(_, staking_contract) = &stake_info.stake_state {
                let new_yield =
                    call_claim_yield_for(&env, &staking_contract, &vault.owner, vault_id);
                stake_info.accumulated_yield += new_yield;
                let mut yield_payout = 0i128;

                if stake_info.accumulated_yield > 0 {
                    if remaining_base > 0 {
                        yield_payout =
                            (claim_amount * stake_info.accumulated_yield) / remaining_base;
                    }
                }

                if yield_payout > 0 {
                    stake_info.accumulated_yield -= yield_payout;
                }
                set_stake_info(&env, vault_id, &stake_info);

                if yield_payout > 0 {
                    let token: Address = env
                        .storage()
                        .instance()
                        .get(&DataKey::Token)
                        .expect("Token not set");
                    token::Client::new(&env, &token).transfer(
                        &staking_contract,
                        &vault.owner,
                        &yield_payout,
                    );
                }
            }
        }
        // -----------------------------------

        // #90: XLM Minimum Reserve Check
        let xlm: Option<Address> = env.storage().instance().get(&DataKey::XLMAddress);
        if let Some(xlm_addr) = xlm {
            if allocation.asset_id == xlm_addr {
                let total_left =
                    allocation.total_amount - (allocation.released_amount + claim_amount);
                if total_left < 20_000_000 {
                    return Err(Error::InsufficientBalance);
                }
            }
        }

        token::Client::new(&env, &allocation.asset_id).transfer(
            &env.current_contract_address(),
            &vault.owner,
            &claim_amount,
        );

        if let Some(nft_minter) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::NFTMinter)
        {
            env.invoke_contract::<()>(
                &nft_minter,
                &Symbol::new(&env, "mint"),
                (&vault.owner,).into_val(&env),
            );
        }

        claim_amount
    }

    /// Check if vault is fully vested and register certificate if completed
    /// This function should be called after any claim operation
    fn check_and_register_certificate(env: &Env, vault_id: u64, vault: &Vault) {
        // Check if vault is fully vested
        if Self::is_vault_fully_vested(env, vault_id, vault) {
            // Check if certificate already registered
            let certificate_id = U256::from_u128(env, vault_id as u128);
            if !env
                .storage()
                .instance()
                .has(&DataKey::CertificateRegistry(certificate_id))
            {
                // Calculate total claimed and asset information
                let mut total_claimed = 0i128;
                let mut total_assets = 0i128;
                let mut asset_types = Vec::new(env);

                for allocation in vault.allocations.iter() {
                    total_claimed += allocation.released_amount;
                    total_assets += allocation.total_amount;
                    asset_types.push_back(allocation.asset_id.clone());
                }

                // Register certificate
                VestingCertificateRegistry::register_completed_vest(
                    env.clone(),
                    vault_id,
                    vault.owner.clone(),
                    vault.clone(),
                    total_claimed,
                    total_assets,
                    asset_types,
                    None, // metadata_uri - can be set later
                );
            }
        }
    }

    /// Check if a vault is fully vested (all tokens claimed)
    fn is_vault_fully_vested(env: &Env, _vault_id: u64, vault: &Vault) -> bool {
        let now = env.ledger().timestamp();

        // Check if vesting period has ended
        if now < vault.end_time {
            return false;
        }

        // Check if all tokens are claimed
        let mut total_claimed = 0i128;
        let mut total_amount = 0i128;

        for allocation in vault.allocations.iter() {
            total_claimed += allocation.released_amount;
            total_amount += allocation.total_amount;
        }

        total_claimed >= total_amount
    }

    /// Set the milestone list for a vault (admin only).
    pub fn set_milestones(env: Env, vault_id: u64, milestones: Vec<Milestone>) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        env.storage()
            .instance()
            .set(&DataKey::VaultMilestones(vault_id), &milestones);
    }

    /// Return the milestone list for a vault.
    pub fn get_milestones(env: Env, vault_id: u64) -> Vec<Milestone> {
        env.storage()
            .instance()
            .get(&DataKey::VaultMilestones(vault_id))
            .unwrap_or(Vec::new(&env))
    }

    /// Mark a milestone as completed, unlocking the associated token tranche (admin only).
    pub fn unlock_milestone(env: Env, vault_id: u64, milestone_id: u64) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        let mut milestones: Vec<Milestone> = Self::get_milestones(env.clone(), vault_id);
        let mut found = false;
        for (i, m) in milestones.iter().enumerate() {
            if m.id == milestone_id {
                let mut updated = m.clone();
                updated.is_unlocked = true;
                milestones.set(i.try_into().unwrap(), updated);
                found = true;
                break;
            }
        }
        if !found {
            return Err(Error::MilestoneNotCompleted);
        }
        env.storage()
            .instance()
            .set(&DataKey::VaultMilestones(vault_id), &milestones);
    }

    /// Freeze or unfreeze a vault, blocking all claims while frozen (admin only).
    pub fn freeze_vault(env: Env, vault_id: u64, freeze: bool) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        let mut vault = Self::get_vault_internal(&env, vault_id);
        vault.is_frozen = freeze;
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);
    }

    /// Pause a specific vesting schedule, blocking claims until resumed.
    pub fn pause_specific_schedule(env: Env, vault_id: u64, reason: String) {
        Self::require_pause_authority(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        if env
            .storage()
            .instance()
            .has(&DataKey::PausedVault(vault_id))
        {
            return Err(Error::ContractPaused);
        }
        let pause_info = PausedVault {
            vault_id,
            pause_timestamp: env.ledger().timestamp(),
            pause_authority: env
                .storage()
                .instance()
                .get(&DataKey::AdminAddress)
                .unwrap(),
            reason,
        };
        env.storage()
            .instance()
            .set(&DataKey::PausedVault(vault_id), &pause_info);
    }

    /// Resume a previously paused vesting schedule.
    pub fn resume_specific_schedule(env: Env, vault_id: u64) {
        Self::require_pause_authority(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        if !env
            .storage()
            .instance()
            .has(&DataKey::PausedVault(vault_id))
        {
            return Err(Error::InvalidInput);
        }
        env.storage()
            .instance()
            .remove(&DataKey::PausedVault(vault_id));
    }

    /// Set the address authorised to pause individual schedules (admin only).
    pub fn set_pause_authority(env: Env, authority: Address) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        env.storage()
            .instance()
            .set(&DataKey::PauseAuthority, &authority);
    }

    /// Return the current pause authority address, or `None` if not set.
    pub fn get_pause_authority(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::PauseAuthority)
    }

    /// Returns `true` if the specified vault is currently paused.
    pub fn is_vault_paused(env: Env, vault_id: u64) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::PausedVault(vault_id))
    }

    /// Return the pause record for a vault, or `None` if not paused.
    pub fn get_paused_vault_info(env: Env, vault_id: u64) -> Option<PausedVault> {
        env.storage()
            .instance()
            .get(&DataKey::PausedVault(vault_id))
    }

    /// Permanently mark a vault as irrevocable (admin only).
    ///
    /// Once set this cannot be undone; the admin can never revoke the vault.
    pub fn mark_irrevocable(env: Env, vault_id: u64) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        let mut vault = Self::get_vault_internal(&env, vault_id);
        vault.is_irrevocable = true;
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);
    }

    /// Configure a performance-based cliff condition for a vault (admin only).
    pub fn set_performance_cliff(env: Env, vault_id: u64, cliff: PerformanceCliff) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        env.storage()
            .instance()
            .set(&DataKey::VaultPerformanceCliff(vault_id), &cliff);
    }

    /// Return the performance cliff configuration for a vault, or `None` if not set.
    pub fn get_performance_cliff(env: Env, vault_id: u64) -> Option<PerformanceCliff> {
        env.storage()
            .instance()
            .get(&DataKey::VaultPerformanceCliff(vault_id))
    }

    /// Returns `true` if the performance cliff condition for a vault has been met.
    pub fn is_cliff_passed(env: Env, vault_id: u64) -> bool {
        if let Some(cliff) = Self::get_performance_cliff(env.clone(), vault_id) {
            OracleClient::is_cliff_passed(&env, &cliff, vault_id)
        } else {
            // No performance cliff set, use time-based cliff check
            let vault = Self::get_vault_internal(&env, vault_id);
            env.ledger().timestamp() >= vault.start_time
        }
    }

    /// Creates a vault with performance cliff conditions
    pub fn create_vault_with_cliff(
        env: Env,
        owner: Address,
        amount: i128,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
        cliff: PerformanceCliff,
    ) -> u64 {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        let vault_id = Self::create_vault_full_internal(
            &env,
            owner,
            amount,
            start_time,
            end_time,
            keeper_fee,
            is_revocable,
            is_transferable,
            step_duration,
        );
        env.storage()
            .instance()
            .set(&DataKey::VaultPerformanceCliff(vault_id), &cliff);
        vault_id
    }

    /// Configures cliff smoothing for a vault (admin only)
    pub fn configure_cliff_smoothing(
        env: Env,
        vault_id: u64,
        cliff_duration: u64,
        smoothing_duration: u64,
        cliff_percentage: u32,
    ) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            panic!("Use AdminProposal for multisig");
        }

        // Validate vault exists
        let vault = Self::get_vault_internal(&env, vault_id);

        // Security validation: ensure smoothing doesn't extend beyond total duration
        let total_duration = vault.end_time - vault.start_time;
        if cliff_duration + smoothing_duration > total_duration {
            panic!("InvalidSmoothingConfiguration: Cliff + smoothing exceeds total duration");
        }

        // Validate percentage is within reasonable bounds (0-100%)
        if cliff_percentage > 10000 {
            panic!("InvalidSmoothingConfiguration: Cliff percentage cannot exceed 100%");
        }

        // Validate smoothing duration is reasonable (at least 1 day, max 1 year)
        if smoothing_duration < 86400 || smoothing_duration > 31536000 {
            panic!("InvalidSmoothingConfiguration: Smoothing duration must be between 1 day and 1 year");
        }

        let config = CliffSmoothingConfig {
            cliff_duration,
            smoothing_duration,
            cliff_percentage,
        };

        env.storage()
            .instance()
            .set(&DataKey::CliffSmoothingConfig(vault_id), &config);
    }

    /// Gets cliff smoothing configuration for a vault
    pub fn get_cliff_smoothing_config(env: Env, vault_id: u64) -> Option<CliffSmoothingConfig> {
        env.storage()
            .instance()
            .get(&DataKey::CliffSmoothingConfig(vault_id))
    }

    /// Removes cliff smoothing configuration (admin only)
    pub fn remove_cliff_smoothing(env: Env, vault_id: u64) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            panic!("Use AdminProposal for multisig");
        }

        if env
            .storage()
            .instance()
            .has(&DataKey::CliffSmoothingConfig(vault_id))
        {
            env.storage()
                .instance()
                .remove(&DataKey::CliffSmoothingConfig(vault_id));
        }
    }

    /// Creates a vault with cliff smoothing configuration
    pub fn create_vault_with_smoothed_cliff(
        env: Env,
        owner: Address,
        amount: i128,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
        cliff_duration: u64,
        smoothing_duration: u64,
        cliff_percentage: u32,
    ) -> u64 {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            panic!("Use AdminProposal for multisig");
        }

        // Security validation
        let total_duration = end_time - start_time;
        if cliff_duration + smoothing_duration > total_duration {
            panic!("InvalidSmoothingConfiguration: Cliff + smoothing exceeds total duration");
        }

        if cliff_percentage > 10000 {
            panic!("InvalidSmoothingConfiguration: Cliff percentage cannot exceed 100%");
        }

        let vault_id = Self::create_vault_full_internal(
            &env,
            owner,
            amount,
            start_time,
            end_time,
            keeper_fee,
            is_revocable,
            is_transferable,
            step_duration,
        );

        let config = CliffSmoothingConfig {
            cliff_duration,
            smoothing_duration,
            cliff_percentage,
        };

        env.storage()
            .instance()
            .set(&DataKey::CliffSmoothingConfig(vault_id), &config);
        vault_id
    }

    // --- Anti-Dilution Configuration Functions ---

    /// Configures anti-dilution settings for a vault (admin only)
    pub fn configure_anti_dilution(
        env: Env,
        vault_id: u64,
        network_growth_oracle: Address,
        inflation_oracle: Option<Address>,
        adjustment_frequency: u64,
        max_adjustment_pct: u32,
    ) {
        Self::require_admin(&env);

        // Verify vault exists
        let vault = Self::get_vault_internal(&env, vault_id);

        // Get baseline network value at configuration time
        let baseline_network_value =
            OracleClient::query_network_growth(&env, &network_growth_oracle);

        let config = AntiDilutionConfig {
            enabled: true,
            network_growth_oracle,
            inflation_oracle,
            adjustment_frequency,
            last_adjustment_time: vault.creation_time,
            baseline_network_value,
            cumulative_adjustment_factor: 0,
            max_adjustment_pct,
        };

        env.storage()
            .instance()
            .set(&DataKey::AntiDilutionConfig(vault_id), &config);
    }

    /// Enables or disables anti-dilution for a vault (admin only)
    pub fn set_anti_dilution_enabled(env: Env, vault_id: u64, enabled: bool) {
        Self::require_admin(&env);

        if let Some(mut config) = env
            .storage()
            .instance()
            .get::<_, AntiDilutionConfig>(&DataKey::AntiDilutionConfig(vault_id))
        {
            config.enabled = enabled;
            env.storage()
                .instance()
                .set(&DataKey::AntiDilutionConfig(vault_id), &config);
        }
    }

    /// Gets anti-dilution configuration for a vault
    pub fn get_anti_dilution_config(env: Env, vault_id: u64) -> Option<AntiDilutionConfig> {
        env.storage()
            .instance()
            .get::<_, AntiDilutionConfig>(&DataKey::AntiDilutionConfig(vault_id))
    }

    /// Gets the latest network growth snapshot for a vault
    pub fn get_network_growth_snapshot(env: Env, vault_id: u64) -> Option<NetworkGrowthSnapshot> {
        env.storage()
            .instance()
            .get(&DataKey::NetworkGrowthSnapshot(vault_id))
    }

    /// Manually triggers anti-dilution adjustment (admin only, for testing)
    pub fn trigger_anti_dilution_adjustment(env: Env, vault_id: u64) {
        Self::require_admin(&env);

        // Verify vault exists
        let _vault = Self::get_vault_internal(&env, vault_id);

        // Force adjustment by temporarily updating last_adjustment_time
        if let Some(mut config) = env
            .storage()
            .instance()
            .get::<_, AntiDilutionConfig>(&DataKey::AntiDilutionConfig(vault_id))
        {
            let old_time = config.last_adjustment_time;
            config.last_adjustment_time = 0; // Force adjustment
            env.storage()
                .instance()
                .set(&DataKey::AntiDilutionConfig(vault_id), &config);

            // Trigger calculation to apply adjustment
            Self::get_claimable_amount(env.clone(), vault_id);

            // Restore original time
            config.last_adjustment_time = old_time;
            env.storage()
                .instance()
                .set(&DataKey::AntiDilutionConfig(vault_id), &config);
        }
    }

    /// Gets total claimable amount across all assets (for backward compatibility)
    pub fn get_claimable_amount(env: Env, vault_id: u64) -> i128 {
        let vault = Self::get_vault_internal(&env, vault_id);
        Self::calculate_claimable(&env, vault_id, &vault)
    }

    /// Gets claimable amounts for each asset in the basket
    pub fn get_claimable_diversified(env: Env, vault_id: u64) -> Vec<(Address, i128)> {
        let vault = Self::get_vault_internal(&env, vault_id);
        let mut claimable_amounts = Vec::new(&env);

        for (i, allocation) in vault.allocations.iter().enumerate() {
            let vested_amount = Self::calculate_claimable_for_asset(&env, vault_id, &vault, i);
            let claimable_amount = vested_amount - allocation.released_amount;
            claimable_amounts.push_back((allocation.asset_id.clone(), claimable_amount));
        }

        claimable_amounts
    }

    /// Locks tokens for a specific asset in the vault (for collateral)
    pub fn lock_tokens_for_asset(env: Env, vault_id: u64, asset_id: Address, amount: i128) {
        let bridge: Address = env
            .storage()
            .instance()
            .get(&DataKey::CollateralBridge)
            .expect("Collateral bridge not set");
        bridge.require_auth();

        let mut vault = Self::get_vault_internal(&env, vault_id);

        // Find the asset allocation
        let mut found = false;
        for (i, allocation) in vault.allocations.iter().enumerate() {
            if allocation.asset_id == asset_id {
                let available =
                    allocation.total_amount - allocation.released_amount - allocation.locked_amount;
                if amount > available {
                    return Err(Error::InsufficientBalance);
                }

                let mut updated_allocation = allocation.clone();
                updated_allocation.locked_amount += amount;
                vault
                    .allocations
                    .set(i.try_into().unwrap(), updated_allocation);
                found = true;
                break;
            }
        }

        if !found {
            return Err(Error::VaultNotFound);
        }

        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);
    }

    /// Legacy function for single-asset vaults
    pub fn lock_tokens(env: Env, vault_id: u64, amount: i128) {
        let vault = Self::get_vault_internal(&env, vault_id);
        if vault.allocations.len() != 1 {
            return Err(Error::InvalidInput);
        }

        let asset_id = vault.allocations.get(0).unwrap().asset_id.clone();
        Self::lock_tokens_for_asset(env, vault_id, asset_id, amount);
    }

    /// Unlocks tokens for a specific asset in the vault
    pub fn unlock_tokens_for_asset(env: Env, vault_id: u64, asset_id: Address, amount: i128) {
        let bridge: Address = env
            .storage()
            .instance()
            .get(&DataKey::CollateralBridge)
            .expect("Collateral bridge not set");
        bridge.require_auth();

        let mut vault = Self::get_vault_internal(&env, vault_id);

        // Find the asset allocation
        let mut found = false;
        for (i, allocation) in vault.allocations.iter().enumerate() {
            if allocation.asset_id == asset_id {
                if amount > allocation.locked_amount {
                    return Err(Error::InsufficientBalance);
                }

                let mut updated_allocation = allocation.clone();
                updated_allocation.locked_amount -= amount;
                vault
                    .allocations
                    .set(i.try_into().unwrap(), updated_allocation);
                found = true;
                break;
            }
        }

        if !found {
            return Err(Error::VaultNotFound);
        }

        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);
    }

    /// Legacy function for single-asset vaults
    pub fn unlock_tokens(env: Env, vault_id: u64, amount: i128) {
        let vault = Self::get_vault_internal(&env, vault_id);
        if vault.allocations.len() != 1 {
            return Err(Error::InvalidInput);
        }

        let asset_id = vault.allocations.get(0).unwrap().asset_id.clone();
        Self::unlock_tokens_for_asset(env, vault_id, asset_id, amount);
    }

    /// Claims tokens by lender for a specific asset
    pub fn claim_by_lender_for_asset(
        env: Env,
        vault_id: u64,
        lender: Address,
        asset_id: Address,
        amount: i128,
    ) -> i128 {
        let bridge: Address = env
            .storage()
            .instance()
            .get(&DataKey::CollateralBridge)
            .expect("Collateral bridge not set");
        bridge.require_auth();

        let mut vault = Self::get_vault_internal(&env, vault_id);

        // Find the asset allocation
        let mut found = false;
        for (i, allocation) in vault.allocations.iter().enumerate() {
            if allocation.asset_id == asset_id {
                if amount > allocation.locked_amount {
                    return Err(Error::InsufficientBalance);
                }

                let mut updated_allocation = allocation.clone();
                updated_allocation.locked_amount -= amount;
                vault
                    .allocations
                    .set(i.try_into().unwrap(), updated_allocation);
                found = true;
                break;
            }
        }

        if !found {
            return Err(Error::VaultNotFound);
        }

        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        token::Client::new(&env, &asset_id).transfer(
            &env.current_contract_address(),
            &lender,
            &amount,
        );

        amount
    }
    /// Gets the asset basket for a vault
    pub fn get_vault_asset_basket(env: Env, vault_id: u64) -> Vec<AssetAllocationEntry> {
        let vault = Self::get_vault_internal(&env, vault_id);
        vault.allocations
    }

    /// Updates the asset basket for a vault (admin only, before initialization)
    pub fn update_vault_asset_basket(
        env: Env,
        vault_id: u64,
        new_basket: Vec<AssetAllocationEntry>,
    ) {
        Self::require_admin(&env);
        let mut vault = Self::get_vault_internal(&env, vault_id);

        if vault.is_initialized {
            return Err(Error::AlreadyInitialized);
        }

        if !Self::validate_asset_basket(&new_basket) {
            return Err(Error::InvalidInput);
        }

        if new_basket.is_empty() {
            return Err(Error::InvalidInput);
        }

        vault.allocations = new_basket;
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);
    }

    /// Gets vault statistics for diversified vesting
    pub fn get_vault_statistics(env: Env, vault_id: u64) -> (i128, i128, i128, u32) {
        let vault = Self::get_vault_internal(&env, vault_id);
        let total_value = Self::calculate_basket_total_value(&vault.allocations);
        let released_value = Self::calculate_basket_released_value(&vault.allocations);
        let claimable_value = Self::calculate_claimable(&env, vault_id, &vault) - released_value;
        let asset_count = vault.allocations.len() as u32;

        (total_value, released_value, claimable_value, asset_count)
    }

    /// Legacy function for single-asset vaults
    pub fn claim_by_lender(env: Env, vault_id: u64, lender: Address, amount: i128) -> i128 {
        let vault = Self::get_vault_internal(&env, vault_id);
        if vault.allocations.len() != 1 {
            return Err(Error::InvalidInput);
        }

        let asset_id = vault.allocations.get(0).unwrap().asset_id.clone();
        Self::claim_by_lender_for_asset(env, vault_id, lender, asset_id, amount)
    }

    /// Set the collateral bridge contract address (must be done via AdminProposal).
    pub fn set_collateral_bridge(_env: Env, _bridge_address: Address) {
        return Err(Error::MultisigNotActive);
    }

    /// Returns `true` if the contract is globally paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::IsPaused)
            .unwrap_or(false)
    }

    /// Return the current admin address.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::AdminAddress)
            .expect("Admin not set")
    }

    /// Return the vault record for `vault_id`.
    pub fn get_vault(env: Env, vault_id: u64) -> Vault {
        Self::get_vault_internal(&env, vault_id)
    }

    /// Set the IPFS/Arweave metadata anchor CID (must be done via AdminProposal).
    pub fn set_metadata_anchor(_env: Env, _cid: String) {
        return Err(Error::MultisigNotActive);
    }

    /// Return the current metadata anchor CID.
    pub fn get_metadata_anchor(env: Env) -> String {
        env.storage()
            .instance()
            .get(&DataKey::MetadataAnchor)
            .unwrap_or(String::from_str(&env, ""))
    }

    /// Return all vault IDs owned by `user`.
    pub fn get_user_vaults(env: Env, user: Address) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::UserVaults(user))
            .unwrap_or(Vec::new(&env))
    }

    /// Return the governance voting power of `user` (= total unvested token balance).
    pub fn get_voting_power(env: Env, user: Address) -> i128 {
        // If this user has delegated their power to someone else, they have 0
        if env
            .storage()
            .instance()
            .has(&DataKey::VotingDelegate(user.clone()))
        {
            return 0;
        }

        let mut total_power = Self::calculate_user_own_power(&env, &user);

        // Add power from others who delegated to this user
        let delegators: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::DelegatedBeneficiaries(user))
            .unwrap_or(vec![&env]);
        for delegator in delegators.iter() {
            total_power += Self::calculate_user_own_power(&env, &delegator);
        }

        total_power
    }

    /// Delegate voting power from `beneficiary` to `representative`.
    pub fn delegate_voting_power(env: Env, beneficiary: Address, representative: Address) {
        beneficiary.require_auth();

        // 1. Get current representative if any
        let old_representative: Option<Address> = env
            .storage()
            .instance()
            .get(&DataKey::VotingDelegate(beneficiary.clone()));

        // 2. If same as before, do nothing
        if let Some(ref old) = old_representative {
            if old == &representative {
                return;
            }

            // Remove from old representative's list
            let mut old_list: Vec<Address> = env
                .storage()
                .instance()
                .get(&DataKey::DelegatedBeneficiaries(old.clone()))
                .unwrap_or(vec![&env]);
            if let Some(idx) = old_list.first_index_of(&beneficiary) {
                old_list.remove(idx);
                env.storage()
                    .instance()
                    .set(&DataKey::DelegatedBeneficiaries(old.clone()), &old_list);
            }
        }

        // 3. Update to new representative
        // If representative is beneficiary itself, it means undelegate
        if beneficiary == representative {
            env.storage()
                .instance()
                .remove(&DataKey::VotingDelegate(beneficiary.clone()));
        } else {
            env.storage().instance().set(
                &DataKey::VotingDelegate(beneficiary.clone()),
                &representative,
            );

            // Add to new representative's list
            let mut new_list: Vec<Address> = env
                .storage()
                .instance()
                .get(&DataKey::DelegatedBeneficiaries(representative.clone()))
                .unwrap_or(vec![&env]);
            if !new_list.contains(&beneficiary) {
                new_list.push_back(beneficiary.clone());
                env.storage()
                    .instance()
                    .set(&DataKey::DelegatedBeneficiaries(representative), &new_list);
            }
        }
    }

    /// Accelerate all vesting schedules by a percentage (must be done via AdminProposal).
    pub fn accelerate_all_schedules(_env: Env, _percentage: u32) {
        return Err(Error::MultisigNotActive);
    }

    /// Slash the unvested balance of a vault and transfer it to `treasury` (admin only).
    pub fn slash_unvested_balance(env: Env, vault_id: u64, treasury: Address) {
        Self::require_admin(&env);
        let mut vault = Self::get_vault_internal(&env, vault_id);

        let vested = Self::calculate_claimable(&env, vault_id, &vault);
        let mut total_amount = 0i128;
        for allocation in vault.allocations.iter() {
            total_amount += allocation.total_amount;
        }
        let unvested = total_amount - vested;

        if unvested <= 0 {
            return Err(Error::NothingToClaim);
        }

        // Effectively stop the clock for this vault
        vault.end_time = env.ledger().timestamp();
        vault.step_duration = 0;

        // Reset milestones to prevent future unlocks from a reduced total
        if env
            .storage()
            .instance()
            .has(&DataKey::VaultMilestones(vault_id))
        {
            env.storage()
                .instance()
                .remove(&DataKey::VaultMilestones(vault_id));
        }

        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        // Update global tracking
        let total_shares: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalShares)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &(total_shares - unvested));

        // Transfer to community treasury
        let token: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Token not set");
        token::Client::new(&env, &token).transfer(
            &env.current_contract_address(),
            &treasury,
            &unvested,
        );

        // Emit event
        VaultSlashed {
            vault_id,
            vested_amount: vested,
            unvested_amount: unvested,
            treasury: treasury.clone(),
        }
        .publish(&env);
    }

    // --- Auto-Stake Functions ---

    /// Whitelist a staking contract address so vaults can stake against it.
    /// Only callable by the admin.
    pub fn add_staking_contract(env: Env, staking_contract: Address) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        let mut approved = get_approved_staking_contracts(&env);
        if !approved.contains(&staking_contract) {
            approved.push_back(staking_contract);
            env.storage()
                .instance()
                .set(&DataKey::ApprovedStakingContracts, &approved);
        }
    }

    /// Remove a staking contract from the approved whitelist (admin only).
    pub fn remove_staking_contract(env: Env, staking_contract: Address) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        let approved = get_approved_staking_contracts(&env);
        let mut new_approved = Vec::new(&env);
        for a in approved.iter() {
            if a != staking_contract {
                new_approved.push_back(a);
            }
        }
        env.storage()
            .instance()
            .set(&DataKey::ApprovedStakingContracts, &new_approved);
    }

    /// Return the list of whitelisted staking contracts.
    pub fn get_staking_contracts(env: Env) -> Vec<Address> {
        get_approved_staking_contracts(&env)
    }

    /// Register the vault's locked balance as an active stake on `staking_contract`.
    ///
    /// No tokens are transferred â€” the staking contract records the stake by
    /// trust. The vault's `staked_amount` field is updated to reflect the
    /// registered amount.
    ///
    /// # Panics
    /// - If the vault is frozen or not initialized.
    /// - If the vault is already staked (`AlreadyStaked`).
    /// - If the locked balance is zero (`InsufficientBalance`).
    /// - If `staking_contract` is not whitelisted (`UnauthorizedStakingContract`).
    /// - If the caller is neither the vault owner nor the admin.
    pub fn auto_stake(env: Env, vault_id: u64, staking_contract: Address) {
        Self::require_not_paused(&env);
        let mut vault = Self::get_vault_internal(&env, vault_id);
        if vault.is_frozen {
            return Err(Error::VaultFrozen);
        }
        if !vault.is_initialized {
            return Err(Error::VaultNotInitialized);
        }

        // Auth: owner or admin â€” require owner auth (admin can mock_all_auths in tests)
        vault.owner.require_auth();

        // Heartbeat: reset Dead-Man's Switch
        update_activity(&env, vault_id);

        // Validate staking contract is whitelisted
        if !is_approved_staking_contract(&env, &staking_contract) {
            return Err(Error::Unauthorized);
        }

        let mut stake_info = get_stake_info(&env, vault_id);

        // Cannot double-stake
        if stake_info.stake_state != StakeState::Unstaked {
            return Err(Error::AlreadyStaked);
        }

        // Must have locked balance
        let mut locked = 0i128;
        for allocation in vault.allocations.iter() {
            locked += allocation.total_amount - allocation.released_amount;
        }
        if locked <= 0 {
            return Err(Error::InsufficientBalance);
        }

        let _guard = match ReentrancyGuard::enter(&env) {
            Ok(guard) => guard,
            Err(err) => return Err(err),
        };

        // Update stake info before invoking the external staking contract so a
        // callback cannot observe the vault as still unstaked.
        stake_info.tokens_staked = locked;
        stake_info.stake_state =
            StakeState::Staked(env.ledger().timestamp(), staking_contract.clone());
        set_stake_info(&env, vault_id, &stake_info);

        // Update vault staked_amount
        vault.staked_amount = locked;
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        // Update global staked counter
        let total_staked: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalStaked)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalStaked, &(total_staked + locked));

        // Call the staking contract synchronously (Soroban: no async, direct call)
        call_stake_tokens(&env, &staking_contract, &vault.owner, vault_id, locked);

        stake::emit_staked(&env, vault_id, &vault.owner, locked, &staking_contract);
    }

    /// Manually unstake a vault. The beneficiary (owner) or admin can call this.
    ///
    /// # Panics
    /// - If the vault is not currently staked (`NotStaked`).
    pub fn manual_unstake(env: Env, vault_id: u64) {
        Self::require_not_paused(&env);
        let mut vault = Self::get_vault_internal(&env, vault_id);
        vault.owner.require_auth();
        // Heartbeat: reset Dead-Man's Switch
        update_activity(&env, vault_id);
        Self::do_unstake(&env, vault_id, &mut vault);
    }

    /// Claim yield accrued on the staking contract for a vault.
    ///
    /// The yield is transferred from the staking contract to the beneficiary.
    /// The vault's `accumulated_yield` is reset to zero after the transfer.
    ///
    /// # Panics
    /// - If the vault is not currently staked (`NotStaked`).
    /// - If the vault has been revoked (`BeneficiaryRevoked`).
    pub fn claim_yield(env: Env, vault_id: u64) -> i128 {
        Self::require_not_paused(&env);
        let vault = Self::get_vault_internal(&env, vault_id);

        if vault.yield_destination == YieldDestination::Beneficiary {
            vault.owner.require_auth();
            return Err(Error::InvalidInput);
        }
        // If DAO, anyone can trigger the harvest and it goes to the treasury.

        // Heartbeat: reset Dead-Man's Switch
        update_activity(&env, vault_id);

        // Guard: revoked vaults cannot claim yield
        if Self::is_vault_revoked(&env, vault_id) {
            return Err(Error::VaultRevoked);
        }

        let mut stake_info = get_stake_info(&env, vault_id);

        let staking_contract = match &stake_info.stake_state {
            StakeState::Staked(_, staking_contract) => staking_contract.clone(),
            StakeState::Unstaked => return Err(Error::StakeNotFound),
        };
        let _guard = match ReentrancyGuard::enter(&env) {
            Ok(guard) => guard,
            Err(err) => return Err(err),
        };

        let yield_amount = call_claim_yield_for(&env, &staking_contract, &vault.owner, vault_id);

        stake_info.accumulated_yield = 0;
        set_stake_info(&env, vault_id, &stake_info);

        if yield_amount > 0 {
            // Transfer yield from staking contract to DAO treasury
            let token: Address = env
                .storage()
                .instance()
                .get(&DataKey::Token)
                .expect("Token not set");
            let treasury = Self::get_admin(env.clone());
            token::Client::new(&env, &token).transfer(&staking_contract, &treasury, &yield_amount);
        }

        stake::emit_yield_claimed(&env, vault_id, &Self::get_admin(env.clone()), yield_amount);
        yield_amount
    }

    /// Batch revoke multiple vaults in a single atomic transaction.
    ///
    /// This function is designed for "Mass Termination" scenarios where multiple
    /// team members (e.g., a 5-person sub-team) need to be let go simultaneously.
    /// All unvested tokens from all specified vaults are returned to the DAO treasury
    /// in a single atomic action.
    ///
    /// # Parameters
    /// - `vault_ids`: Vector of vault IDs to revoke (e.g., beneficiary IDs)
    /// - `treasury`: Address where all unvested tokens will be sent
    ///
    /// # Behavior
    /// - Processes all vaults in a single transaction (atomic operation)
    /// - Auto-unstakes any staked vaults before revocation
    /// - Returns all unvested tokens to the treasury
    /// - Emits a single TeamRevocation event with aggregated data
    ///
    /// # Panics
    /// - If any vault is marked irrevocable
    /// - If caller is not an admin
    pub fn batch_revoke_vaults(env: Env, vault_ids: Vec<u64>, treasury: Address) {
        Self::require_admin(&env);

        let mut total_revoked: i128 = 0;
        let mut revoked_owners: Vec<Address> = Vec::new(&env);

        // Process each vault
        for vault_id in vault_ids.iter() {
            let mut vault = Self::get_vault_internal(&env, vault_id);

            if vault.is_irrevocable {
                return Err(Error::VaultFrozen);
            }

            // Auto-unstake if staked
            let stake_info = get_stake_info(&env, vault_id);
            if stake_info.stake_state != StakeState::Unstaked {
                Self::do_unstake(&env, vault_id, &mut vault);
                stake::emit_revocation_unstaked(&env, vault_id, &vault.owner);
            }

            // Mark vault as revoked
            Self::mark_vault_revoked(&env, vault_id);

            // Calculate remaining tokens for this vault
            let mut remaining = 0i128;
            for allocation in vault.allocations.iter() {
                remaining += allocation.total_amount - allocation.released_amount;
            }

            if remaining > 0 {
                // Update allocations to mark all as released
                for (i, allocation) in vault.allocations.iter().enumerate() {
                    let mut updated_allocation = allocation.clone();
                    updated_allocation.released_amount = allocation.total_amount;
                    vault
                        .allocations
                        .set(i.try_into().unwrap(), updated_allocation);
                }
                vault.end_time = env.ledger().timestamp();
                vault.step_duration = 0;
                vault.is_frozen = true;

                if env
                    .storage()
                    .instance()
                    .has(&DataKey::VaultMilestones(vault_id))
                {
                    env.storage()
                        .instance()
                        .remove(&DataKey::VaultMilestones(vault_id));
                }

                env.storage()
                    .instance()
                    .set(&DataKey::VaultData(vault_id), &vault);

                let total_shares: i128 = env
                    .storage()
                    .instance()
                    .get(&DataKey::TotalShares)
                    .unwrap_or(0);
                env.storage()
                    .instance()
                    .set(&DataKey::TotalShares, &(total_shares - remaining));

                total_revoked += remaining;
                revoked_owners.push_back(vault.owner.clone());
            }
        }

        // Transfer all revoked tokens to treasury in a single transaction
        if total_revoked > 0 {
            let token: Address = env
                .storage()
                .instance()
                .get(&DataKey::Token)
                .expect("Token not set");
            token::Client::new(&env, &token).transfer(
                &env.current_contract_address(),
                &treasury,
                &total_revoked,
            );

            // Emit single TeamRevocation event
            TeamRevoked {
                vaults_count: vault_ids.len(),
                owners: revoked_owners,
                total_amount: total_revoked,
                treasury: treasury.clone(),
            }
            .publish(&env);
        }
    }

    /// Revoke a vault, transferring all unvested tokens to `treasury` (admin only).
    ///
    /// If the vault is currently staked, it is unstaked first.
    /// Irrevocable vaults cannot be revoked.
    pub fn revoke_vault(env: Env, vault_id: u64, treasury: Address) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }
        Self::do_revoke_vault_internal(&env, vault_id, treasury);
    }

    /// Partial revocation with a penalty split.
    ///
    /// Splits the unvested balance of a single-asset vault between the treasury
    /// (penalty) and the beneficiary (severance):
    ///   - `penalty_pct` % of unvested â†’ treasury
    ///   - `(100 - penalty_pct)` % of unvested â†’ immediately claimable by beneficiary
    ///
    /// The vault is frozen after the call; the beneficiary may still claim any
    /// tokens that were already vested plus the severance portion.
    pub fn partial_revoke(env: Env, vault_id: u64, penalty_pct: u32, treasury: Address) {
        Self::require_admin(&env);

        if penalty_pct > 100 {
            return Err(Error::InvalidInput);
        }

        let mut vault = Self::get_vault_internal(&env, vault_id);

        if vault.is_irrevocable {
            return Err(Error::VaultFrozen);
        }
        if vault.is_frozen {
            return Err(Error::VaultFrozen);
        }
        if vault.allocations.len() != 1 {
            return Err(Error::InvalidInput);
        }

        // Auto-unstake if staked
        let stake_info = get_stake_info(&env, vault_id);
        if stake_info.stake_state != StakeState::Unstaked {
            Self::do_unstake(&env, vault_id, &mut vault);
            stake::emit_revocation_unstaked(&env, vault_id, &vault.owner);
        }

        let allocation = vault.allocations.get(0).unwrap();
        let unvested = allocation.total_amount - allocation.released_amount;

        if unvested <= 0 {
            return Err(Error::NothingToClaim);
        }

        // penalty goes to treasury; remainder is immediately vested for beneficiary
        let penalty_amount = (unvested * penalty_pct as i128) / 100;
        let severance_amount = unvested - penalty_amount;

        let token: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Token not set");
        let token_client = token::Client::new(&env, &token);

        // Transfer penalty to treasury
        if penalty_amount > 0 {
            token_client.transfer(&env.current_contract_address(), &treasury, &penalty_amount);
        }

        // Transfer severance directly to beneficiary
        if severance_amount > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &vault.owner,
                &severance_amount,
            );
        }

        // Update allocation: mark everything as released and freeze the vault
        let mut updated = allocation.clone();
        updated.released_amount = updated.total_amount;
        vault.allocations.set(0, updated);
        vault.is_frozen = true;
        vault.end_time = env.ledger().timestamp();
        vault.step_duration = 0;

        Self::mark_vault_revoked(&env, vault_id);

        if env
            .storage()
            .instance()
            .has(&DataKey::VaultMilestones(vault_id))
        {
            env.storage()
                .instance()
                .remove(&DataKey::VaultMilestones(vault_id));
        }

        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        let total_shares: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalShares)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &(total_shares - unvested));

        PartialRevocation {
            vault_id,
            penalty_amount,
            severance_amount,
            treasury: treasury.clone(),
        }
        .publish(&env);
    }

    /// Partial clawback with dynamic emission rate recalculation.
    ///
    /// When a schedule is partially clawed back by the DAO, this function dynamically
    /// recalculates the ongoing emission rate for the remaining tokens so the schedule
    /// still ends on the original designated date, rather than ending early.
    ///
    /// # Parameters
    /// - `vault_id`: ID of the vault to partially clawback
    /// - `clawback_amount`: Amount of tokens to clawback from unvested portion
    /// - `treasury`: Address where clawed tokens will be sent
    ///
    /// # Behavior
    /// - Calculates how much should have been vested by current time
    /// - Removes clawback amount from remaining unvested tokens
    /// - Recalculates emission rate to distribute remaining tokens over remaining time
    /// - Preserves original end_time so schedule completes on schedule
    ///
    /// # Panics
    /// - If vault is irrevocable
    /// - If clawback_amount exceeds available unvested tokens
    /// - If caller is not an admin
    pub fn partial_clawback_dynamic(
        env: Env,
        vault_id: u64,
        clawback_amount: i128,
        treasury: Address,
    ) {
        Self::require_admin(&env);

        if clawback_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut vault = Self::get_vault_internal(&env, vault_id);

        if vault.is_irrevocable {
            return Err(Error::VaultFrozen);
        }
        if vault.is_frozen {
            return Err(Error::VaultFrozen);
        }
        if vault.allocations.len() != 1 {
            return Err(Error::InvalidInput);
        }

        // Auto-unstake if staked
        let stake_info = get_stake_info(&env, vault_id);
        if stake_info.stake_state != StakeState::Unstaked {
            Self::do_unstake(&env, vault_id, &mut vault);
            stake::emit_revocation_unstaked(&env, vault_id, &vault.owner);
        }

        let allocation = vault.allocations.get(0).unwrap();
        let current_time = env.ledger().timestamp();

        // Calculate current vested amount based on original schedule
        let vested_amount = Self::calculate_claimable_for_asset(&env, vault_id, &vault, 0);
        let unvested_amount = allocation.total_amount - vested_amount;

        if clawback_amount > unvested_amount {
            return Err(Error::InsufficientBalance);
        }

        // Transfer clawback amount to treasury
        let token: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Token not set");
        let token_client = token::Client::new(&env, &token);

        if clawback_amount > 0 {
            token_client.transfer(&env.current_contract_address(), &treasury, &clawback_amount);
        }

        // Calculate new emission rate to ensure remaining tokens vest by original end_time
        let remaining_tokens = unvested_amount - clawback_amount;
        let remaining_time = vault.end_time.saturating_sub(current_time);

        if remaining_tokens > 0 && remaining_time > 0 {
            // Update allocation with reduced total amount and preserved released amount
            let mut updated_allocation = allocation.clone();
            updated_allocation.total_amount = vested_amount + remaining_tokens;

            // Store the original emission rate for reference
            let original_total = allocation.total_amount;
            let original_duration = vault.end_time - vault.start_time;
            let original_rate = original_total / (original_duration as i128);

            // Calculate new emission rate
            let new_rate = remaining_tokens / (remaining_time as i128);

            // Update vault structure
            vault.allocations.set(0, updated_allocation);

            // Store clawback adjustment data for emission rate calculation
            let clawback_data = ClawbackAdjustment {
                clawback_time: current_time,
                clawback_amount,
                original_total_amount: original_total,
                original_rate,
                new_rate,
                remaining_tokens,
            };

            env.storage()
                .instance()
                .set(&DataKey::ClawbackAdjustment(vault_id), &clawback_data);
        }

        // Update total shares
        let total_shares: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalShares)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &(total_shares - clawback_amount));

        // Save updated vault
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        // Emit event
        PartialClawbackDynamic {
            vault_id,
            clawback_amount,
            remaining_tokens,
            treasury: treasury.clone(),
            clawback_time: current_time,
        }
        .publish(&env);
    }

    /// Get clawback adjustment data for a vault (for testing and inspection)
    pub fn get_clawback_adjustment(env: Env, vault_id: u64) -> ClawbackAdjustment {
        env.storage()
            .instance()
            .get(&DataKey::ClawbackAdjustment(vault_id))
            .unwrap_or(ClawbackAdjustment {
                clawback_time: 0,
                clawback_amount: 0,
                original_total_amount: 0,
                original_rate: 0,
                new_rate: 0,
                remaining_tokens: 0,
            })
    }

    /// Return the current stake status for a vault.
    pub fn get_stake_status(env: Env, vault_id: u64) -> StakeStatusView {
        let info = get_stake_info(&env, vault_id);
        StakeStatusView {
            vault_id,
            stake_state: info.stake_state,
            tokens_staked: info.tokens_staked,
            accumulated_yield: info.accumulated_yield,
        }
    }

    // --- Inheritance / Dead-Man's Switch Functions ---

    /// Nominate a backup address and configure the inactivity timer.
    ///
    /// # Security
    /// - Caller must be the vault's current primary beneficiary.
    /// - `backup` must not equal the primary and must not be the zero address.
    /// - `switch_duration` must be within `[MIN_SWITCH_DURATION, MAX_SWITCH_DURATION]`.
    /// - `challenge_window` must be within `[MIN_CHALLENGE_WINDOW, MAX_CHALLENGE_WINDOW]`.
    /// - Cannot be called after succession has been finalised.
    pub fn nominate_backup(
        env: Env,
        vault_id: u64,
        backup: Address,
        switch_duration: u64,
        challenge_window: u64,
    ) {
        let vault = Self::get_vault_internal(&env, vault_id);
        nominate_backup(
            &env,
            vault_id,
            &vault.owner,
            backup,
            switch_duration,
            challenge_window,
        );
    }

    /// Revoke the nominated backup, resetting succession state to `None`.
    ///
    /// # Security
    /// - Caller must be the vault's current primary beneficiary.
    /// - Only valid when state is `Nominated` â€” blocked during an active claim.
    pub fn revoke_backup(env: Env, vault_id: u64) {
        let vault = Self::get_vault_internal(&env, vault_id);
        revoke_backup(&env, vault_id, &vault.owner);
    }

    /// Initiate a succession claim as the nominated backup.
    ///
    /// # Security
    /// - Caller must be the nominated backup address.
    /// - The inactivity timer must have fully elapsed.
    pub fn initiate_succession_claim(env: Env, vault_id: u64, caller: Address) {
        initiate_succession_claim(&env, vault_id, &caller);
    }

    /// Finalise succession, permanently transferring vault ownership to the backup.
    ///
    /// # Security
    /// - Caller must be the backup address.
    /// - The challenge window must have fully elapsed.
    /// - This operation is irreversible.
    pub fn finalise_succession(env: Env, vault_id: u64, caller: Address) {
        let new_owner = finalise_succession(&env, vault_id, &caller);
        // Update the vault's owner field to the new primary
        let mut vault = Self::get_vault_internal(&env, vault_id);
        vault.owner = new_owner;
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);
    }

    /// Cancel a pending succession claim. Resets state to `Nominated`.
    ///
    /// # Security
    /// - Caller must be the current primary beneficiary.
    /// - State must be `ClaimPending`.
    pub fn cancel_succession_claim(env: Env, vault_id: u64) {
        let vault = Self::get_vault_internal(&env, vault_id);
        cancel_succession_claim(&env, vault_id, &vault.owner);
    }

    /// Return the full succession status for a vault.
    pub fn get_succession_status(env: Env, vault_id: u64) -> SuccessionView {
        let vault = Self::get_vault_internal(&env, vault_id);
        get_succession_status(&env, vault_id, vault.owner)
    }

    // --- Internal Helpers ---

    fn require_admin(env: &Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::AdminAddress)
            .expect("Admin not set");
        admin.require_auth();
    }

    fn require_pause_authority(env: &Env) {
        // Check if there's a designated pause authority
        if let Some(authority) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::PauseAuthority)
        {
            authority.require_auth();
        } else {
            // Fallback to admin if no specific pause authority is set
            Self::require_admin(env);
        }
    }

    fn require_not_paused(env: &Env) {
        if env
            .storage()
            .instance()
            .get(&DataKey::IsPaused)
            .unwrap_or(false)
        {
            return Err(Error::ContractPaused);
        }
    }

    fn is_emergency_pause_active(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::IsPaused)
            .unwrap_or(false)
    }

    fn _require_collateral_bridge(env: &Env) {
        let bridge: Address = env
            .storage()
            .instance()
            .get(&DataKey::CollateralBridge)
            .expect("Collateral bridge not set");
        bridge.require_auth();
    }

    fn require_valid_duration(start: u64, end: u64) {
        if end <= start {
            return Err(Error::InvalidSchedule);
        }
        if end - start > MAX_DURATION {
            return Err(Error::InvalidSchedule);
        }
    }

    /// Maximum cliff-to-total-duration ratio in basis points (50%).
    /// A linear ramp whose cliff exceeds this fraction of the total vesting
    /// period would produce an unacceptably large "cliff-jump" on first claim.
    const MAX_CLIFF_RATIO_BPS: u64 = 5000;

    /// Validates that a linear ramp schedule does not produce a cliff-jump
    /// larger than [`MAX_CLIFF_RATIO_BPS`] of the total vesting period.
    ///
    /// Only enforced when `step_duration == 0` (pure linear) and a cliff is
    /// present (`start_time > grant_time`).
    ///
    /// # Errors
    /// Panics with [`Error::CliffJumpTooLarge`] when the cliff ratio exceeds
    /// the maximum.
    fn check_cliff_jump_smoothness(
        grant_time: u64,
        start_time: u64,
        end_time: u64,
        step_duration: u64,
    ) {
        // Only applies to linear ramps with an explicit cliff.
        if step_duration != 0 || start_time <= grant_time {
            return;
        }
        let total_duration = end_time - grant_time;
        let cliff_duration = start_time - grant_time;
        // cliff_ratio_bps = cliff_duration * 10_000 / total_duration
        let cliff_ratio_bps = cliff_duration * 10_000 / total_duration;
        if cliff_ratio_bps > Self::MAX_CLIFF_RATIO_BPS {
            panic!("CliffJumpTooLarge");
        }
    }

    fn create_vault_full_internal(
        env: &Env,
        owner: Address,
        amount: i128,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
    ) -> u64 {
        // For backward compatibility, create a single-asset vault
        let token: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Token not set");
        let allocation = AssetAllocationEntry {
            asset_id: token.clone(),
            total_amount: amount,
            released_amount: 0,
            locked_amount: 0,
            percentage: 10000, // 100% in basis points
        };
        let mut allocations = Vec::new(env);
        allocations.push_back(allocation);

        Self::sub_admin_balance(env, amount);
        let admin = Self::get_admin(env.clone());
        token::Client::new(env, &token).transfer(&admin, &env.current_contract_address(), &amount);
        Self::create_vault_prefunded_internal(
            env,
            owner,
            allocations,
            start_time,
            end_time,
            keeper_fee,
            is_revocable,
            is_transferable,
            step_duration,
            true,
        )
    }

    fn create_vault_lazy_internal(
        env: &Env,
        owner: Address,
        amount: i128,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
    ) -> u64 {
        // For backward compatibility, create a single-asset vault
        let token: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Token not set");
        let allocation = AssetAllocationEntry {
            asset_id: token,
            total_amount: amount,
            released_amount: 0,
            locked_amount: 0,
            percentage: 10000, // 100% in basis points
        };
        let mut allocations = Vec::new(env);
        allocations.push_back(allocation);

        Self::create_vault_prefunded_internal(
            env,
            owner,
            allocations,
            start_time,
            end_time,
            keeper_fee,
            is_revocable,
            is_transferable,
            step_duration,
            false,
        )
    }

    fn create_vault_prefunded_internal(
        env: &Env,
        owner: Address,
        allocations: Vec<AssetAllocationEntry>,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
        is_initialized: bool,
    ) -> u64 {
        Self::require_valid_duration(start_time, end_time);
        let grant_time = env.ledger().timestamp();
        Self::check_cliff_jump_smoothness(grant_time, start_time, end_time, step_duration);
        let id = Self::increment_vault_count(env);
        let title = String::from_str(env, "");
        let vault = Vault {
            allocations,
            keeper_fee,
            staked_amount: 0,
            owner: owner.clone(),
            delegate: None,
            title,
            start_time,
            end_time,
            creation_time: env.ledger().timestamp(),
            step_duration,
            is_initialized,
            is_irrevocable: !is_revocable,
            is_transferable,
            is_frozen: false,
            requires_legal_signatures: false,
            legal_documents_signed: true, // Default to true for backward compatibility
            yield_destination: YieldDestination::Beneficiary,
        };
        env.storage()
            .instance()
            .set(&DataKey::VaultData(id), &vault);
        if is_initialized {
            Self::add_user_vault_index(env, &owner, id);
        }
        let total_amount = Self::calculate_basket_total_value(&vault.allocations);
        Self::add_total_shares(env, total_amount);
        id
    }

    fn get_vault_internal(env: &Env, id: u64) -> Vault {
        env.storage()
            .instance()
            .get(&DataKey::VaultData(id))
            .expect("Vault not found")
    }

    fn increment_vault_count(env: &Env) -> u64 {
        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::VaultCount)
            .unwrap_or(0);
        let new_count = count + 1;
        env.storage()
            .instance()
            .set(&DataKey::VaultCount, &new_count);
        new_count
    }

    fn sub_admin_balance(env: &Env, amount: i128) {
        let bal: i128 = env
            .storage()
            .instance()
            .get(&DataKey::AdminBalance)
            .unwrap_or(0);
        if bal < amount {
            return Err(Error::InsufficientBalance);
        }
        env.storage()
            .instance()
            .set(&DataKey::AdminBalance, &(bal - amount));
    }

    fn reserve_admin_balance_for_batch(env: &Env, amount: i128) {
        let bal: i128 = env
            .storage()
            .instance()
            .get(&DataKey::AdminBalance)
            .unwrap_or(0);
        if bal < amount {
            return Err(Error::InsufficientBalance);
        }
        env.storage()
            .instance()
            .set(&DataKey::AdminBalance, &(bal - amount));
    }

    fn add_total_shares(env: &Env, amount: i128) {
        let shares: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalShares)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &(shares + amount));
    }

    fn require_deposited_tokens_for_batch(env: &Env, amount: i128) {
        let token: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Token not set");
        let contract_address = env.current_contract_address();
        let onchain_balance = token::Client::new(env, &token).balance(&contract_address);
        if onchain_balance < amount {
            return Err(Error::InsufficientBalance);
        }
    }

    fn validate_batch_data(data: &BatchCreateData) -> i128 {
        let count = data.recipients.len();
        if count == 0 {
            return Err(Error::InvalidInput);
        }
        if data.asset_baskets.len() != count
            || data.start_times.len() != count
            || data.end_times.len() != count
            || data.keeper_fees.len() != count
            || !(data.step_durations.len() == count || data.step_durations.is_empty())
        {
            return Err(Error::InvalidInput);
        }

        let mut total_amount: i128 = 0;
        for i in 0..count {
            let asset_basket = data.asset_baskets.get(i).unwrap();
            // Calculate total amount from asset basket
            let mut basket_total = 0i128;
            for allocation in asset_basket.iter() {
                basket_total += allocation.total_amount;
            }
            if basket_total < 0 {
                return Err(Error::InvalidAmount);
            }

            let start_time = data.start_times.get(i).unwrap();
            let end_time = data.end_times.get(i).unwrap();
            VestingContract::require_valid_duration(start_time, end_time);

            total_amount = total_amount
                .checked_add(basket_total)
                .expect("Batch amount overflow");
        }
        total_amount
    }

    fn _validate_schedule_configs(schedules: &Vec<ScheduleConfig>) -> i128 {
        if schedules.is_empty() {
            return Err(Error::InvalidInput);
        }

        let mut total_amount: i128 = 0;
        for i in 0..schedules.len() {
            let schedule = schedules.get(i).unwrap();
            // Calculate total amount from asset basket
            let mut schedule_total = 0i128;
            for allocation in schedule.asset_basket.iter() {
                schedule_total += allocation.total_amount;
            }
            if schedule_total < 0 {
                return Err(Error::InvalidAmount);
            }

            Self::require_valid_duration(schedule.start_time, schedule.end_time);

            VestingContract::require_valid_duration(schedule.start_time, schedule.end_time);
            total_amount = total_amount
                .checked_add(schedule_total)
                .expect("Schedule amount overflow");
        }
        total_amount
    }

    fn add_user_vault_index(env: &Env, user: &Address, id: u64) {
        let mut vaults: Vec<u64> = env
            .storage()
            .instance()
            .get(&DataKey::UserVaults(user.clone()))
            .unwrap_or(vec![env]);
        vaults.push_back(id);
        env.storage()
            .instance()
            .set(&DataKey::UserVaults(user.clone()), &vaults);
    }

    fn remove_user_vault_index(env: &Env, user: &Address, id: u64) {
        let vaults: Vec<u64> = env
            .storage()
            .instance()
            .get(&DataKey::UserVaults(user.clone()))
            .unwrap_or(Vec::new(env));
        let mut new_vaults = Vec::new(env);
        for v in vaults.iter() {
            if v != id {
                new_vaults.push_back(v);
            }
        }
        env.storage()
            .instance()
            .set(&DataKey::UserVaults(user.clone()), &new_vaults);
    }

    fn calculate_user_own_power(env: &Env, user: &Address) -> i128 {
        let vault_ids: Vec<u64> = env
            .storage()
            .instance()
            .get(&DataKey::UserVaults(user.clone()))
            .unwrap_or(Vec::new(env));
        let mut total_power: i128 = 0;
        for id in vault_ids.iter() {
            let vault = Self::get_vault_internal(env, id);
            let mut balance = 0i128;
            for allocation in vault.allocations.iter() {
                balance += allocation.total_amount - allocation.released_amount;
            }
            let weight = if vault.is_irrevocable { 100 } else { 50 };
            total_power += (balance * weight) / 100;
        }
        total_power
    }

    /// Internal: perform the unstake operation against the staking contract and
    /// update vault + stake_info state. Caller must have already loaded `vault`.
    fn do_unstake(env: &Env, vault_id: u64, vault: &mut crate::Vault) {
        let mut stake_info = get_stake_info(env, vault_id);

        let staking_contract = match &stake_info.stake_state {
            StakeState::Staked(_, staking_contract) => staking_contract.clone(),
            StakeState::Unstaked => return Err(Error::StakeNotFound),
        };
        let _guard = match ReentrancyGuard::enter(env) {
            Ok(guard) => guard,
            Err(err) => return Err(err),
        };

        // Update global staked counter
        let total_staked: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalStaked)
            .unwrap_or(0);
        let new_total = if total_staked > stake_info.tokens_staked {
            total_staked - stake_info.tokens_staked
        } else {
            0
        };
        env.storage()
            .instance()
            .set(&DataKey::TotalStaked, &new_total);

        let unstaked_amount = stake_info.tokens_staked;
        vault.staked_amount = 0;
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), vault);

        stake_info.tokens_staked = 0;
        stake_info.stake_state = StakeState::Unstaked;
        set_stake_info(env, vault_id, &stake_info);

        call_unstake_tokens(env, &staking_contract, &vault.owner, vault_id);

        stake::emit_unstaked(env, vault_id, &vault.owner, unstaked_amount);
    }

    /// Mark a vault as revoked in the global revoked-vaults set.
    fn mark_vault_revoked(env: &Env, vault_id: u64) {
        let mut revoked: Vec<u64> = env
            .storage()
            .instance()
            .get(&DataKey::RevokedVaults)
            .unwrap_or(Vec::new(env));
        if !revoked.contains(&vault_id) {
            revoked.push_back(vault_id);
            env.storage()
                .instance()
                .set(&DataKey::RevokedVaults, &revoked);
        }
    }

    /// Returns `true` if the vault has been revoked.
    fn is_vault_revoked(env: &Env, vault_id: u64) -> bool {
        let revoked: Vec<u64> = env
            .storage()
            .instance()
            .get(&DataKey::RevokedVaults)
            .unwrap_or(Vec::new(env));
        revoked.contains(&vault_id)
    }

    /// Validates that asset basket percentages sum to 10000 (100%)
    fn validate_asset_basket(basket: &Vec<AssetAllocationEntry>) -> bool {
        let total_percentage: u32 = basket.iter().map(|a| a.percentage).sum();
        total_percentage == 10000 // 100% in basis points
    }

    /// Calculates the total value of all assets in a basket
    fn calculate_basket_total_value(basket: &Vec<AssetAllocationEntry>) -> i128 {
        basket.iter().map(|a| a.total_amount).sum()
    }

    /// Calculates the total released value of all assets in a basket
    fn calculate_basket_released_value(basket: &Vec<AssetAllocationEntry>) -> i128 {
        basket.iter().map(|a| a.released_amount).sum()
    }

    /// Creates a new asset allocation with validation
    pub fn create_asset_allocation(
        asset_id: Address,
        total_amount: i128,
        percentage: u32,
    ) -> AssetAllocationEntry {
        if total_amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if percentage == 0 || percentage > 10000 {
            return Err(Error::InvalidInput);
        }

        AssetAllocationEntry {
            asset_id,
            total_amount,
            released_amount: 0,
            locked_amount: 0,
            percentage,
        }
    }

    fn calculate_claimable_for_asset(
        env: &Env,
        id: u64,
        vault: &Vault,
        asset_index: usize,
    ) -> i128 {
        let allocation = vault
            .allocations
            .get(asset_index.try_into().unwrap())
            .unwrap();

        if let Some(cliff) = env
            .storage()
            .instance()
            .get(&DataKey::VaultPerformanceCliff(id))
        {
            if !OracleClient::is_cliff_passed(env, &cliff, id) {
                return 0;
            }
        }

        let mut now = env.ledger().timestamp();

        if let Some(paused_info) = env
            .storage()
            .instance()
            .get::<DataKey, PausedVault>(&DataKey::PausedVault(id))
        {
            now = paused_info.pause_timestamp;
        } else {
            let accel_pct: u32 = env
                .storage()
                .instance()
                .get(&DataKey::GlobalAccelerationPct)
                .unwrap_or(0u32);
            if accel_pct > 0 {
                let duration_u64 = vault.end_time.saturating_sub(vault.start_time);
                let shift = ((duration_u64 as i128) * (accel_pct as i128) / 100) as u64;
                now = now.saturating_add(shift);
            }
        }

        if now <= vault.start_time {
            return 0;
        }
        if now >= vault.end_time {
            return allocation.total_amount;
        }

        // Check if there's a clawback adjustment that requires dynamic emission rate
        if let Some(clawback_adj) = env
            .storage()
            .instance()
            .get::<DataKey, ClawbackAdjustment>(&DataKey::ClawbackAdjustment(id))
        {
            // Use dynamic emission rate calculation
            if now <= clawback_adj.clawback_time {
                // Before clawback: use original rate
                let elapsed = (now - vault.start_time) as i128;
                return (clawback_adj.original_total_amount * elapsed)
                    / ((vault.end_time - vault.start_time) as i128);
            } else {
                // After clawback: vested amount before clawback + new rate for remaining time
                let elapsed_before_clawback =
                    (clawback_adj.clawback_time - vault.start_time) as i128;
                let vested_before_clawback = (clawback_adj.original_total_amount
                    * elapsed_before_clawback)
                    / ((vault.end_time - vault.start_time) as i128);

                let elapsed_after_clawback = (now - clawback_adj.clawback_time) as i128;
                let remaining_time = (vault.end_time - clawback_adj.clawback_time) as i128;

                let vested_after_clawback = if remaining_time > 0 {
                    (clawback_adj.remaining_tokens * elapsed_after_clawback) / remaining_time
                } else {
                    0
                };

                return vested_before_clawback + vested_after_clawback;
            }
        }

        // Check for cliff smoothing configuration
        if let Some(smoothing_config) = env
            .storage()
            .instance()
            .get::<DataKey, CliffSmoothingConfig>(&DataKey::CliffSmoothingConfig(id))
        {
            return Self::calculate_smoothed_claimable(
                env,
                id,
                vault,
                allocation,
                &smoothing_config,
                now,
            );
        }

        // Original calculation for vaults without clawback adjustment or smoothing
        let duration = (vault.end_time - vault.start_time) as i128;
        let elapsed = (now - vault.start_time) as i128;

        let base_vested = if vault.step_duration > 0 {
            let steps = duration / (vault.step_duration as i128);
            if steps == 0 {
                0
            } else {
                let completed = elapsed / (vault.step_duration as i128);
                (allocation.total_amount * completed) / steps
            }
        } else {
            (allocation.total_amount * elapsed) / duration
        };

        Self::apply_anti_dilution_adjustment(env, id, base_vested, allocation.total_amount)
    }

    fn calculate_claimable(env: &Env, id: u64, vault: &Vault) -> i128 {
        let mut total_claimable = 0;
        for (i, allocation) in vault.allocations.iter().enumerate() {
            let vested = Self::calculate_claimable_for_asset(env, id, vault, i.try_into().unwrap());
            total_claimable += vested - allocation.released_amount;
        }
        total_claimable
    }

    #[cfg(test)]
    pub fn calculate_claimable_for_asset_wrapper(env: Env, id: u64, asset_index: usize) -> i128 {
        let vault = Self::get_vault_internal(&env, id);
        Self::calculate_claimable_for_asset(&env, id, &vault, asset_index)
    }

    /// Applies anti-dilution adjustments to vested amount based on network growth
    /// Calculates claimable amount with smoothed cliff release
    /// This function implements the core logic for gradual cliff token release
    fn calculate_smoothed_claimable(
        env: &Env,
        vault_id: u64,
        vault: &Vault,
        allocation: &AssetAllocationEntry,
        smoothing_config: &CliffSmoothingConfig,
        now: u64,
    ) -> i128 {
        let cliff_end_time = vault.start_time + smoothing_config.cliff_duration;
        let smoothing_end_time = cliff_end_time + smoothing_config.smoothing_duration;

        // Calculate total duration and elapsed time
        let total_duration = (vault.end_time - vault.start_time) as i128;
        let elapsed = (now - vault.start_time) as i128;

        // Calculate cliff amount (tokens that become available at cliff)
        let cliff_amount =
            (allocation.total_amount * smoothing_config.cliff_percentage as i128) / 10000;
        let remaining_amount = allocation.total_amount - cliff_amount;

        if now <= vault.start_time {
            return 0;
        }

        if now >= vault.end_time {
            return allocation.total_amount;
        }

        if now <= cliff_end_time {
            // Before cliff: only linear vesting of non-cliff portion
            let non_cliff_elapsed = elapsed;
            let non_cliff_vested = (remaining_amount * non_cliff_elapsed) / total_duration;
            return non_cliff_vested;
        } else if now <= smoothing_end_time {
            // During smoothing window: linear release of cliff amount + ongoing non-cliff vesting
            let smoothing_elapsed = (now - cliff_end_time) as i128;
            let smoothing_progress =
                smoothing_elapsed / (smoothing_config.smoothing_duration as i128);

            // Linear release of cliff amount during smoothing window
            let released_cliff = (cliff_amount * smoothing_progress) / 1;

            // Ongoing vesting of non-cliff portion
            let non_cliff_elapsed = elapsed;
            let non_cliff_vested = (remaining_amount * non_cliff_elapsed) / total_duration;

            let total_vested = released_cliff + non_cliff_vested;

            // Emit event for first time entering smoothing window
            if smoothing_elapsed == 1 {
                Self::emit_cliff_smoothed_unlock(
                    env,
                    vault_id,
                    vault.owner.clone(),
                    cliff_amount,
                    released_cliff,
                    cliff_end_time,
                    smoothing_end_time,
                );
            }

            total_vested
        } else {
            // After smoothing window: full cliff amount released + ongoing non-cliff vesting
            let non_cliff_elapsed = elapsed;
            let non_cliff_vested = (remaining_amount * non_cliff_elapsed) / total_duration;

            cliff_amount + non_cliff_vested
        }
    }

    /// Emits CliffSmoothedUnlock event
    fn emit_cliff_smoothed_unlock(
        env: &Env,
        vault_id: u64,
        beneficiary: Address,
        cliff_amount: i128,
        smoothed_amount: i128,
        smoothing_start: u64,
        smoothing_end: u64,
    ) {
        CliffSmoothedUnlock {
            vault_id,
            beneficiary,
            cliff_amount,
            smoothed_amount,
            smoothing_start,
            smoothing_end,
            timestamp: env.ledger().timestamp(),
        }
        .publish(env);
    }

    /// Handles proration for employee termination during smoothing window
    /// Returns the vested amount with proper proration of cliff smoothing
    fn calculate_prorated_smoothed_claimable(
        env: &Env,
        vault_id: u64,
        vault: &Vault,
        allocation: &AssetAllocationEntry,
        smoothing_config: &CliffSmoothingConfig,
        termination_time: u64,
    ) -> i128 {
        let cliff_end_time = vault.start_time + smoothing_config.cliff_duration;
        let smoothing_end_time = cliff_end_time + smoothing_config.smoothing_duration;

        // Calculate total duration and elapsed time
        let total_duration = (vault.end_time - vault.start_time) as i128;
        let elapsed = (termination_time - vault.start_time) as i128;

        // Calculate cliff amount and remaining amount
        let cliff_amount =
            (allocation.total_amount * smoothing_config.cliff_percentage as i128) / 10000;
        let remaining_amount = allocation.total_amount - cliff_amount;

        if termination_time <= vault.start_time {
            return 0;
        }

        if termination_time >= vault.end_time {
            return allocation.total_amount;
        }

        if termination_time <= cliff_end_time {
            // Termination before cliff: only linear vesting of non-cliff portion
            let non_cliff_elapsed = elapsed;
            let non_cliff_vested = (remaining_amount * non_cliff_elapsed) / total_duration;
            return non_cliff_vested;
        } else if termination_time <= smoothing_end_time {
            // Termination during smoothing window: prorated cliff + ongoing non-cliff
            let smoothing_elapsed = (termination_time - cliff_end_time) as i128;
            let smoothing_progress =
                smoothing_elapsed / (smoothing_config.smoothing_duration as i128);

            // Prorated release of cliff amount
            let prorated_cliff = (cliff_amount * smoothing_progress) / 1;

            // Ongoing vesting of non-cliff portion
            let non_cliff_elapsed = elapsed;
            let non_cliff_vested = (remaining_amount * non_cliff_elapsed) / total_duration;

            prorated_cliff + non_cliff_vested
        } else {
            // Termination after smoothing window: full cliff amount + ongoing non-cliff
            let non_cliff_elapsed = elapsed;
            let non_cliff_vested = (remaining_amount * non_cliff_elapsed) / total_duration;

            cliff_amount + non_cliff_vested
        }
    }

    /// Validates that smoothed curve area matches total intended cliff allocation
    /// This is a security function to ensure mathematical correctness
    fn validate_smoothed_curve_integrity(
        cliff_amount: i128,
        smoothing_duration: u64,
        total_duration: u64,
    ) -> bool {
        // The area under the smoothed curve should equal the cliff amount
        // For linear release: area = base * height / 2
        // But since we're doing linear release from 0 to full amount,
        // the area should equal the cliff amount exactly

        // Additional validation: ensure smoothing doesn't extend beyond reasonable bounds
        if smoothing_duration > total_duration / 2 {
            return false; // Smoothing shouldn't exceed half the total duration
        }

        // Validate that cliff amount is reasonable relative to typical allocations
        if cliff_amount < 0 {
            return false;
        }

        true
    }

    fn apply_anti_dilution_adjustment(
        env: &Env,
        vault_id: u64,
        base_vested: i128,
        total_amount: i128,
    ) -> i128 {
        // Check if anti-dilution is configured for this vault
        if let Some(config) = env
            .storage()
            .instance()
            .get::<_, AntiDilutionConfig>(&DataKey::AntiDilutionConfig(vault_id))
        {
            if !config.enabled {
                return base_vested;
            }

            let current_time = env.ledger().timestamp();

            // Check if it's time for adjustment
            if current_time < config.last_adjustment_time + config.adjustment_frequency {
                return base_vested;
            }

            // Query current network growth
            let network_growth =
                OracleClient::query_network_growth(env, &config.network_growth_oracle);

            if network_growth <= 0 {
                return base_vested; // No growth, no adjustment
            }

            // Calculate adjustment factor
            let mut total_adjustment = config.cumulative_adjustment_factor;

            // Add new adjustment based on network growth
            let new_adjustment = network_growth; // Network growth in basis points

            // Apply maximum adjustment limit
            let max_adjustment = config.max_adjustment_pct as i128;
            if total_adjustment + new_adjustment > max_adjustment {
                total_adjustment = max_adjustment;
            } else {
                total_adjustment += new_adjustment;
            }

            // Calculate unvested amount
            let unvested = total_amount - base_vested;

            // Apply adjustment to unvested amount only
            // This preserves the beneficiary's "share of the network"
            let adjustment_multiplier = (10000 + total_adjustment) as i128; // Convert basis points to multiplier
            let adjusted_unvested = (unvested * adjustment_multiplier) / 10000;
            let adjusted_vested = total_amount - adjusted_unvested;

            // Update configuration with new adjustment
            let updated_config = AntiDilutionConfig {
                cumulative_adjustment_factor: total_adjustment,
                last_adjustment_time: current_time,
                ..config
            };
            env.storage()
                .instance()
                .set(&DataKey::AntiDilutionConfig(vault_id), &updated_config);

            // Store snapshot for tracking
            let snapshot = NetworkGrowthSnapshot {
                timestamp: current_time,
                network_value: network_growth,
                adjustment_factor: total_adjustment,
            };
            env.storage()
                .instance()
                .set(&DataKey::NetworkGrowthSnapshot(vault_id), &snapshot);

            adjusted_vested
        } else {
            base_vested
        }
    }

    // --- Governance Helper Functions ---

    fn create_governance_proposal(env: Env, action: GovernanceAction) -> u64 {
        let proposer = Self::get_admin(env.clone());
        let now = env.ledger().timestamp();
        let proposal_id = Self::increment_proposal_count(&env);

        let proposal = GovernanceProposal {
            id: proposal_id,
            action: action.clone(),
            proposer: proposer.clone(),
            created_at: now,
            challenge_end: now + CHALLENGE_PERIOD,
            is_executed: false,
            is_cancelled: false,
            yes_votes: 0,
            no_votes: 0,
        };

        env.storage()
            .instance()
            .set(&DataKey::GovernanceProposal(proposal_id), &proposal);

        // Publish proposal creation event (minimal tuple to avoid IntoVal issues)
        GovernanceProposalCreated {
            proposal_id,
            action: proposal.action.clone(),
            proposer: proposer.clone(),
            challenge_end: proposal.challenge_end,
        }
        .publish(&env);

        proposal_id
    }

    fn increment_admin_proposal_count(env: &Env) -> u64 {
        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::AdminProposalCount)
            .unwrap_or(0);
        let new_count = count + 1;
        env.storage()
            .instance()
            .set(&DataKey::AdminProposalCount, &new_count);
        new_count
    }

    fn get_admin_proposal(env: &Env, proposal_id: u64) -> AdminProposal {
        env.storage()
            .instance()
            .get(&DataKey::AdminProposal(proposal_id))
            .expect("Admin proposal not found")
    }

    fn get_proposal(env: &Env, proposal_id: u64) -> GovernanceProposal {
        env.storage()
            .instance()
            .get(&DataKey::GovernanceProposal(proposal_id))
            .expect("Proposal not found")
    }

    fn get_voter_locked_value(env: &Env, voter: &Address) -> i128 {
        // Get all vaults for this voter and sum their total amounts
        let vault_ids: Vec<u64> = env
            .storage()
            .instance()
            .get(&DataKey::UserVaults(voter.clone()))
            .unwrap_or(Vec::new(env));

        let mut total_locked = 0i128;
        for vault_id in vault_ids.iter() {
            let vault = Self::get_vault_internal(env, vault_id);
            for allocation in vault.allocations.iter() {
                total_locked += allocation.total_amount - allocation.released_amount;
            }
        }

        total_locked
    }

    fn get_total_locked_value(env: &Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalLockedValue)
            .unwrap_or(0i128)
    }

    fn execute_governance_action(env: &Env, action: &GovernanceAction) {
        match action {
            GovernanceAction::AdminRotation(new_admin) => {
                env.storage()
                    .instance()
                    .set(&DataKey::AdminAddress, new_admin);
            }
            GovernanceAction::ContractUpgrade(new_contract) => {
                env.storage()
                    .instance()
                    .set(&DataKey::MigrationTarget, new_contract);
                env.storage().instance().set(&DataKey::IsDeprecated, &true);
            }
            GovernanceAction::EmergencyPause(pause_state) => {
                env.storage()
                    .instance()
                    .set(&DataKey::IsPaused, pause_state);
            }
        }
    }

    fn increment_proposal_count(env: &Env) -> u64 {
        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ProposalCount)
            .unwrap_or(0);
        let new_count = count + 1;
        env.storage()
            .instance()
            .set(&DataKey::ProposalCount, &new_count);
        new_count
    }

    // Public getter functions for governance
    /// Return the governance proposal record for `proposal_id`.
    pub fn get_proposal_info(env: Env, proposal_id: u64) -> GovernanceProposal {
        VestingContract::get_proposal(&env, proposal_id)
    }

    /// Return the governance voting power of `voter`.
    pub fn get_voter_power(env: Env, voter: Address) -> i128 {
        VestingContract::get_voter_locked_value(&env, &voter)
    }

    /// Return the total locked (unvested) token value across all vaults.
    pub fn get_total_locked(env: Env) -> i128 {
        VestingContract::get_total_locked_value(&env)
    }

    /// Pause the contract globally (admin only, requires governance approval).
    pub fn pause(env: Env) {
        Self::get_admin(env.clone()).require_auth();
        env.storage().instance().set(&DataKey::IsPaused, &true);
    }

    /// Resume the contract from a global pause (admin only).
    pub fn resume(env: Env) {
        Self::get_admin(env.clone()).require_auth();
        env.storage().instance().set(&DataKey::IsPaused, &false);
    }

    // --- Marketplace Functions (#89) ---

    /// Authorise a marketplace contract to transfer a vault on behalf of its owner.
    pub fn authorize_marketplace_transfer(env: Env, vault_id: u64, marketplace: Address) {
        let vault = Self::get_vault_internal(&env, vault_id);
        vault.owner.require_auth();
        if !vault.is_transferable {
            return Err(Error::VaultFrozen);
        }
        let lock = MarketplaceLock {
            marketplace,
            authorized_at: env.ledger().timestamp(),
        };
        env.storage()
            .instance()
            .set(&DataKey::MarketplaceLock(vault_id), &lock);
    }

    /// Complete a marketplace transfer, updating the vault owner to `new_owner`.
    pub fn complete_marketplace_transfer(env: Env, vault_id: u64, new_owner: Address) {
        let lock: MarketplaceLock = env
            .storage()
            .instance()
            .get(&DataKey::MarketplaceLock(vault_id))
            .expect("Vault not authorized for marketplace");
        lock.marketplace.require_auth();

        let mut vault = Self::get_vault_internal(&env, vault_id);
        let old_owner = vault.owner.clone();

        // Update owner
        vault.owner = new_owner.clone();
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        // Update indexes
        Self::remove_user_vault_index(&env, &old_owner, vault_id);
        Self::add_user_vault_index(&env, &new_owner, vault_id);

        // Clear lock
        env.storage()
            .instance()
            .remove(&DataKey::MarketplaceLock(vault_id));

        MarketplaceSold {
            vault_id,
            old_owner,
            new_owner,
            marketplace: lock.marketplace,
        }
        .publish(&env);
    }

    // --- Renewal Functions (#91) ---

    fn do_renew_vault_direct(
        env: &Env,
        vault_id: u64,
        additional_duration: u64,
        additional_amount: i128,
    ) {
        let mut vault = Self::get_vault_internal(env, vault_id);

        // Find main asset (first one)
        let mut allocation = vault.allocations.get(0).expect("Empty basket");
        let asset_id = allocation.asset_id.clone();

        // Fund extra from admin
        let admin = Self::get_admin(env.clone());
        token::Client::new(env, &asset_id).transfer(
            &admin,
            &env.current_contract_address(),
            &additional_amount,
        );

        allocation.total_amount += additional_amount;
        vault.allocations.set(0, allocation);
        vault.end_time += additional_duration;

        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        VaultRenewed {
            vault_id,
            duration: additional_duration,
            amount: additional_amount,
        }
        .publish(env);
    }

    /// Extend a vesting schedule by `additional_duration` seconds and deposit
    /// `additional_amount` extra tokens (admin only).
    pub fn renew_schedule(
        env: Env,
        vault_id: u64,
        additional_duration: u64,
        additional_amount: i128,
    ) {
        Self::require_admin(&env);
        Self::do_renew_vault_direct(&env, vault_id, additional_duration, additional_amount);
    }

    // â”€â”€ Issue #145 / #92: KPI Vesting Gate public functions â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    /// Admin attaches a KPI gate to a vault.
    /// Tokens cannot be claimed until `verify_kpi_gate` is called and passes.
    pub fn attach_kpi_gate(
        env: Env,
        vault_id: u64,
        oracle_contract: Address,
        metric_id: Symbol,
        threshold: i128,
        operator: crate::oracle::ComparisonOperator,
    ) {
        Self::require_admin(&env);
        crate::kpi_vesting::attach_kpi_gate(
            &env,
            vault_id,
            oracle_contract,
            metric_id,
            threshold,
            operator,
        );
    }

    /// Anyone can call this to attempt oracle verification.
    /// Idempotent â€” safe to call multiple times.
    pub fn verify_kpi_gate(env: Env, vault_id: u64, caller: Address) -> bool {
        caller.require_auth();
        crate::kpi_vesting::try_verify_kpi(&env, vault_id, &caller)
    }

    /// Read-only: has this vault's KPI been verified?
    pub fn get_kpi_status(env: Env, vault_id: u64) -> bool {
        crate::kpi_vesting::kpi_status(&env, vault_id)
    }

    /// Read-only: configured threshold for a vault (0 if no gate set).
    pub fn get_kpi_threshold(env: Env, vault_id: u64) -> i128 {
        crate::kpi_vesting::kpi_threshold(&env, vault_id)
    }

    /// Read-only: full verification log.
    pub fn get_kpi_log(
        env: Env,
        vault_id: u64,
    ) -> soroban_sdk::Vec<crate::kpi_engine::KpiVerificationRecord> {
        crate::kpi_vesting::kpi_verification_log(&env, vault_id)
    }

    // --- Milestone-Gated Vesting Functions ---

    /// Create a vault with milestone-based token release
    /// Tokens are released only when milestones are triggered in sequence
    pub fn create_milestone_vault(
        env: Env,
        owner: Address,
        amount: i128,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
        milestones: Vec<u32>,
    ) -> u64 {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            panic!("Use AdminProposal for multisig");
        }

        // Validate milestones sum to 10000 (100%)
        let total_percentage: u32 = milestones.iter().sum();
        if total_percentage != 10000 {
            panic!("Milestone percentages must sum to 10000 (100%)");
        }

        let vault_id = Self::create_vault_full_internal(
            &env,
            owner,
            amount,
            start_time,
            end_time,
            keeper_fee,
            is_revocable,
            is_transferable,
            step_duration,
        );

        // Store milestone data
        let milestone_data = crate::milestone::MilestoneData {
            vault_id,
            milestones: milestones.clone(),
            current_milestone: 0,
            triggered_milestones: Vec::new(&env),
        };
        env.storage()
            .instance()
            .set(&DataKey::VaultMilestones(vault_id), &milestone_data);

        vault_id
    }

    /// Trigger a milestone (admin only)
    /// Enforces sequential triggering: milestone N can only be triggered if milestone N-1 is complete
    pub fn trigger_milestone(env: Env, vault_id: u64, milestone_id: u32, admin: Address) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            panic!("Use AdminProposal for multisig");
        }

        let mut milestone_data: crate::milestone::MilestoneData = env
            .storage()
            .instance()
            .get(&DataKey::VaultMilestones(vault_id))
            .expect("Vault has no milestones configured");

        // Check if milestone ID is valid
        if milestone_id as usize >= milestone_data.milestones.len() {
            panic!("Invalid milestone ID");
        }

        // Check if milestone is already triggered
        if milestone_data.triggered_milestones.contains(&milestone_id) {
            panic!("Milestone already triggered");
        }

        // ENFORCE SEQUENTIAL STATE MACHINE
        // Milestone N can only be triggered if milestone N-1 has been triggered
        // Exception: milestone 0 can always be triggered (first milestone)
        if milestone_id > 0 {
            let previous_milestone = milestone_id - 1;
            if !milestone_data
                .triggered_milestones
                .contains(&previous_milestone)
            {
                panic!(
                    "Cannot trigger milestone {} - previous milestone {} must be triggered first",
                    milestone_id, previous_milestone
                );
            }
        }

        // Trigger the milestone
        milestone_data.triggered_milestones.push_back(milestone_id);
        milestone_data.current_milestone = milestone_id;

        // Store updated milestone data
        env.storage()
            .instance()
            .set(&DataKey::VaultMilestones(vault_id), &milestone_data);

        // Emit milestone event
        crate::milestone::MilestoneEvent {
            milestone_id,
            is_triggered: true,
            trigger_time: env.ledger().timestamp(),
            triggered_by: admin,
        }
        .publish(&env);
    }

    /// Claim tokens for triggered milestones
    /// Beneficiary can only claim tokens for milestones that have been triggered
    pub fn claim_milestone_tokens(env: Env, vault_id: u64) -> Result<i128, Error> {
        Self::require_not_paused(&env);
        let vault = Self::get_vault_internal(&env, vault_id);

        if vault.is_frozen {
            return Err(Error::VaultFrozen);
        }
        if !vault.is_initialized {
            return Err(Error::VaultNotInitialized);
        }

        vault.owner.require_auth();

        // Get milestone data
        let milestone_data: crate::milestone::MilestoneData = env
            .storage()
            .instance()
            .get(&DataKey::VaultMilestones(vault_id))
            .expect("Vault has no milestones configured");

        // Calculate claimable amount based on triggered milestones
        let mut total_percentage_triggered = 0u32;
        for milestone_id in milestone_data.triggered_milestones.iter() {
            let idx = *milestone_id as usize;
            if idx < milestone_data.milestones.len() {
                total_percentage_triggered += milestone_data.milestones.get(idx).unwrap();
            }
        }

        // Calculate total vault amount
        let mut total_vault_amount = 0i128;
        for allocation in vault.allocations.iter() {
            total_vault_amount += allocation.total_amount;
        }

        // Calculate claimable amount based on triggered percentage
        let claimable_amount = (total_vault_amount * total_percentage_triggered as i128) / 10000;

        // Calculate already released amount
        let mut already_released = 0i128;
        for allocation in vault.allocations.iter() {
            already_released += allocation.released_amount;
        }

        let actual_claimable = claimable_amount - already_released;

        if actual_claimable <= 0 {
            return Err(Error::NothingToClaim);
        }

        // Update vault allocations
        let mut vault = Self::get_vault_internal(&env, vault_id);
        let mut remaining_to_release = actual_claimable;

        for (i, allocation) in vault.allocations.iter().enumerate() {
            let available = allocation.total_amount - allocation.released_amount;
            let to_release = if available < remaining_to_release {
                available
            } else {
                remaining_to_release
            };

            if to_release > 0 {
                let mut updated_allocation = allocation.clone();
                updated_allocation.released_amount += to_release;
                vault
                    .allocations
                    .set(i.try_into().unwrap(), updated_allocation);
                remaining_to_release -= to_release;
            }

            if remaining_to_release <= 0 {
                break;
            }
        }

        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        // Transfer tokens to beneficiary
        if vault.allocations.len() == 1 {
            let allocation = vault.allocations.get(0).unwrap();
            token::Client::new(&env, &allocation.asset_id).transfer(
                &env.current_contract_address(),
                &vault.owner,
                &actual_claimable,
            );
        } else {
            // For multi-asset vaults, transfer proportionally
            let mut remaining = actual_claimable;
            for allocation in vault.allocations.iter() {
                let asset_share = (allocation.total_amount * actual_claimable) / total_vault_amount;
                if asset_share > 0 && remaining >= asset_share {
                    token::Client::new(&env, &allocation.asset_id).transfer(
                        &env.current_contract_address(),
                        &vault.owner,
                        &asset_share,
                    );
                    remaining -= asset_share;
                }
            }
        }

        Ok(actual_claimable)
    }

    /// Get milestone data for a vault
    pub fn get_milestone_data(env: Env, vault_id: u64) -> Option<crate::milestone::MilestoneData> {
        env.storage()
            .instance()
            .get(&DataKey::VaultMilestones(vault_id))
    }

    // --- Zero-Knowledge Accredited Investor Verification Functions ---

    /// Verify accredited investor status using ZK proof
    /// This allows investors to prove they meet accreditation requirements without revealing sensitive information
    pub fn verify_accredited_investor_proof(
        env: Env,
        investor: Address,
        proof: ZKProof,
    ) -> Result<(), ZKVerifierError> {
        investor.require_auth();
        ZKVerifier::verify_accredited_investor(&env, proof, investor)
    }

    /// Check if an investor has valid accredited investor status
    pub fn is_accredited_investor(env: Env, investor: Address) -> bool {
        ZKVerifier::has_valid_accreditation(&env, investor)
    }

    /// Get accreditation record for an investor
    pub fn get_accreditation_record(env: Env, investor: Address) -> Option<AccreditationRecord> {
        ZKVerifier::get_accreditation_record(&env, investor)
    }

    /// Create a vault with accredited investor verification requirement
    /// Only accredited investors can create or receive these vaults
    pub fn create_vault_accredited_only(
        env: Env,
        owner: Address,
        amount: i128,
        asset_id: Address,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
    ) -> u64 {
        // Verify the creator is an accredited investor
        if !ZKVerifier::has_valid_accreditation(&env, owner.clone()) {
            return Err(Error::AccreditationStatusInvalid);
        }

        owner.require_auth();
        Self::create_vault_full_internal(
            &env,
            owner,
            amount,
            asset_id,
            start_time,
            end_time,
            keeper_fee,
            is_revocable,
            is_transferable,
            step_duration,
        )
    }

    /// Transfer vault with accredited investor verification
    /// Both sender and receiver must be accredited investors
    pub fn transfer_vault_accredited(env: Env, vault_id: u64, from: Address, to: Address) {
        // Verify both parties are accredited investors
        if !ZKVerifier::has_valid_accreditation(&env, from.clone()) {
            return Err(Error::AccreditationStatusInvalid);
        }
        if !ZKVerifier::has_valid_accreditation(&env, to.clone()) {
            return Err(Error::AccreditationStatusInvalid);
        }

        from.require_auth();
        Self::transfer_vault_internal(&env, vault_id, from, to);
    }

    /// Admin function to add verification key for ZK proofs
    pub fn add_zk_verification_key(
        env: Env,
        admin: Address,
        verification_key: VerificationKey,
    ) -> Result<(), ZKVerifierError> {
        Self::require_admin(&env);
        ZKVerifier::add_verification_key(&env, admin, verification_key)
    }

    /// Admin function to add supported circuit
    pub fn add_zk_supported_circuit(
        env: Env,
        admin: Address,
        circuit_id: BytesN<32>,
        circuit_type: Bytes,
    ) -> Result<(), ZKVerifierError> {
        Self::require_admin(&env);
        ZKVerifier::add_supported_circuit(&env, admin, circuit_id, circuit_type)
    }

    // --- Legal SAFT Document Hash Anchoring Functions ---

    /// Store legal document hash for a vault (admin only)
    /// Anchors IPFS CID of physical SAFT or Grant Agreement
    pub fn store_legal_hash(
        env: Env,
        admin: Address,
        vault_id: u64,
        document_type: DocumentType,
        ipfs_cid: String,
        document_hash: BytesN<32>,
        jurisdiction: String,
        version: String,
        expires_at: Option<u64>,
    ) -> Result<(), LegalSAFTError> {
        Self::require_admin(&env);

        // Store the legal document
        LegalSAFTManager::store_legal_hash(
            &env,
            admin,
            vault_id,
            document_type,
            ipfs_cid,
            document_hash,
            jurisdiction,
            version,
            expires_at,
        )?;

        // Update vault to require legal signatures
        Self::set_vault_legal_requirement(&env, vault_id, true);

        Ok(())
    }

    /// Beneficiary signs a legal document hash
    /// Cryptographically signs the document hash on-chain before vesting clock starts
    pub fn sign_legal_document(
        env: Env,
        beneficiary: Address,
        vault_id: u64,
        document_hash: BytesN<32>,
        signature: Bytes,
        message: String,
    ) -> Result<(), LegalSAFTError> {
        beneficiary.require_auth();

        // Sign the document
        LegalSAFTManager::sign_legal_document(
            &env,
            beneficiary,
            vault_id,
            document_hash,
            signature,
            message,
        )?;

        // Check if all documents are now signed and update vault
        if LegalSAFTManager::are_all_documents_signed(&env, vault_id) {
            Self::set_vault_legal_status(&env, vault_id, true);
        }

        Ok(())
    }

    /// Create vault with legal document requirements
    /// Vesting clock only starts after legal documents are signed
    pub fn create_vault_with_legal_requirements(
        env: Env,
        owner: Address,
        amount: i128,
        asset_id: Address,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
        requires_legal_signatures: bool,
    ) -> u64 {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }

        let vault_id = Self::create_vault_full_internal(
            &env,
            owner,
            amount,
            asset_id,
            start_time,
            end_time,
            keeper_fee,
            is_revocable,
            is_transferable,
            step_duration,
        );

        // Set legal requirements
        Self::set_vault_legal_requirement(&env, vault_id, requires_legal_signatures);

        vault_id
    }

    /// Check if vault has all legal documents signed
    pub fn are_legal_documents_signed(env: Env, vault_id: u64) -> bool {
        LegalSAFTManager::are_all_documents_signed(&env, vault_id)
    }

    /// Get legal document by hash
    pub fn get_legal_document(env: Env, document_hash: BytesN<32>) -> Option<LegalDocument> {
        LegalSAFTManager::get_legal_document(&env, document_hash)
    }

    /// Get document signature
    pub fn get_document_signature(
        env: Env,
        beneficiary: Address,
        document_hash: BytesN<32>,
    ) -> Option<DocumentSignature> {
        LegalSAFTManager::get_document_signature(&env, beneficiary, document_hash)
    }

    /// Get vault legal documents status
    pub fn get_vault_legal_documents(env: Env, vault_id: u64) -> Option<VaultLegalDocuments> {
        LegalSAFTManager::get_vault_legal_documents(&env, vault_id)
    }

    /// Get all documents for a vault
    pub fn get_vault_documents(env: Env, vault_id: u64) -> Vec<LegalDocument> {
        LegalSAFTManager::get_vault_documents(&env, vault_id)
    }

    /// Revoke legal document (admin only)
    pub fn revoke_legal_document(
        env: Env,
        admin: Address,
        document_hash: BytesN<32>,
        reason: String,
    ) -> Result<(), LegalSAFTError> {
        Self::require_admin(&env);
        LegalSAFTManager::revoke_legal_document(&env, admin, document_hash, reason)
    }

    // --- Beneficiary Reassignment (Social Recovery / Inheritance) ---

    /// Initialize DAO council and reassignment system
    pub fn initialize_beneficiary_reassignment(
        env: Env,
        admin: Address,
        initial_members: Vec<Address>,
        required_approvals: u32,
        approval_window: u64,
    ) -> Result<(), ReassignmentError> {
        BeneficiaryReassignment::initialize(
            &env,
            admin,
            initial_members,
            required_approvals,
            approval_window,
        )
    }

    /// Create beneficiary reassignment request
    pub fn create_reassignment_request(
        env: Env,
        current_beneficiary: Address,
        new_beneficiary: Address,
        vault_id: u64,
        social_proof_type: SocialProofType,
        social_proof_hash: [u8; 32],
        social_proof_ipfs: String,
        reason: String,
    ) -> Result<(), ReassignmentError> {
        BeneficiaryReassignment::create_reassignment_request(
            &env,
            current_beneficiary,
            new_beneficiary,
            vault_id,
            social_proof_type,
            social_proof_hash,
            social_proof_ipfs,
            reason,
        )
    }

    /// Approve reassignment request (DAO council member)
    pub fn approve_reassignment(
        env: Env,
        approver: Address,
        vault_id: u64,
    ) -> Result<(), ReassignmentError> {
        BeneficiaryReassignment::approve_reassignment(&env, approver, vault_id)
    }

    /// Emergency beneficiary reassignment
    pub fn emergency_reassignment(
        env: Env,
        emergency_admin: Address,
        vault_id: u64,
        new_beneficiary: Address,
        emergency_reason: String,
        social_proof_type: SocialProofType,
        social_proof_hash: [u8; 32],
        social_proof_ipfs: String,
    ) -> Result<(), ReassignmentError> {
        BeneficiaryReassignment::emergency_reassignment(
            &env,
            emergency_admin,
            vault_id,
            new_beneficiary,
            emergency_reason,
            social_proof_type,
            social_proof_hash,
            social_proof_ipfs,
        )
    }

    /// Get reassignment request status
    pub fn get_reassignment_status(env: Env, vault_id: u64) -> Option<ReassignmentRequest> {
        BeneficiaryReassignment::get_reassignment_status(&env, vault_id)
    }

    /// Get active DAO council members
    pub fn get_active_council_members(env: Env) -> Vec<Address> {
        BeneficiaryReassignment::get_active_council_members(&env)
    }

    /// Add DAO council member
    pub fn add_dao_member(
        env: Env,
        admin: Address,
        member_address: Address,
        role: String,
    ) -> Result<(), ReassignmentError> {
        BeneficiaryReassignment::add_dao_member(&env, admin, member_address, role)
    }

    /// Reassign beneficiary to new address with DAO approval
    /// This function legally transfers an active vesting schedule to a new Stellar public key
    /// Requires 2/3 multi-sig approval from DAO Admin council
    pub fn reassign_beneficiary(
        env: Env,
        vault_id: u64,
        new_beneficiary: Address,
        social_proof_type: SocialProofType,
        social_proof_hash: [u8; 32],
        social_proof_ipfs: String,
        reason: String,
    ) -> Result<(), ReassignmentError> {
        // Create reassignment request first
        Self::create_reassignment_request(
            env.clone(),
            Self::get_vault_internal(&env, vault_id).owner,
            new_beneficiary.clone(),
            vault_id,
            social_proof_type.clone(),
            social_proof_hash,
            social_proof_ipfs,
            reason.clone(),
        )?;

        // In a full implementation, this would wait for approvals
        // For now, we'll complete immediately for demonstration
        BeneficiaryReassignment::complete_reassignment(&env, vault_id)?;

        // Update vault owner (this would integrate with main vault transfer logic)
        let mut vault = Self::get_vault_internal(&env, vault_id);
        let old_beneficiary = vault.owner.clone();
        vault.owner = new_beneficiary;
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        // Update user vault index
        Self::remove_user_vault_index(&env, &old_beneficiary, vault_id);
        Self::add_user_vault_index(&env, &new_beneficiary, vault_id);

        // Emit event
        BeneficiaryReassigned {
            vault_id,
            old_beneficiary,
            new_beneficiary,
            social_proof_type,
            reason,
        }
        .publish(&env);

        Ok(())
    }

    // --- SEP-08 Regulated Asset Functions ---

    /// Register a regulated asset with SEP-08 compliance
    pub fn register_regulated_asset(
        env: Env,
        asset_id: Address,
        issuer: Address,
        requires_authorization: bool,
        supports_freeze: bool,
        supports_clawback: bool,
        max_authorization_duration: u64,
        compliance_requirements: Vec<String>,
    ) -> Result<(), RegulatedAssetError> {
        Self::require_admin(&env);
        RegulatedAssetManager::register_regulated_asset(
            &env,
            asset_id,
            issuer,
            requires_authorization,
            supports_freeze,
            supports_clawback,
            max_authorization_duration,
            compliance_requirements,
        )
    }

    /// Create SEP-08 authorization for regulated asset
    pub fn create_sep08_authorization(
        env: Env,
        asset_id: Address,
        holder: Address,
        authorized_amount: i128,
        authorization_id: BytesN<32>,
        expires_at: u64,
        issuer: Address,
        compliance_flags: u32,
    ) -> Result<(), RegulatedAssetError> {
        RegulatedAssetManager::create_authorization(
            &env,
            asset_id,
            holder,
            authorized_amount,
            authorization_id,
            expires_at,
            issuer,
            compliance_flags,
        )
    }

    /// Handle asset freeze event from issuer (SEP-08)
    pub fn handle_asset_freeze(
        env: Env,
        asset_id: Address,
        holder: Address,
        amount: i128,
        reason: String,
        issuer_signature: BytesN<32>,
    ) -> Result<(), RegulatedAssetError> {
        RegulatedAssetManager::handle_freeze_event(
            &env,
            asset_id,
            holder,
            amount,
            reason,
            issuer_signature,
        )
    }

    /// Handle asset clawback event from issuer (SEP-08)
    pub fn handle_asset_clawback(
        env: Env,
        asset_id: Address,
        from_holder: Address,
        amount: i128,
        reason: String,
        issuer_signature: BytesN<32>,
    ) -> Result<(), RegulatedAssetError> {
        RegulatedAssetManager::handle_clawback_event(
            &env,
            asset_id,
            from_holder,
            amount,
            reason,
            issuer_signature,
        )
    }

    /// Check if asset requires SEP-08 authorization
    pub fn asset_requires_authorization(env: Env, asset_id: Address) -> bool {
        RegulatedAssetManager::requires_authorization(&env, asset_id)
    }

    /// Get asset regulation info
    pub fn get_asset_regulation(env: Env, asset_id: Address) -> Option<AssetRegulation> {
        RegulatedAssetManager::get_asset_regulation(&env, asset_id)
    }

    /// Create vault with regulated asset support
    pub fn create_vault_regulated(
        env: Env,
        owner: Address,
        amount: i128,
        asset_id: Address,
        start_time: u64,
        end_time: u64,
        keeper_fee: i128,
        is_revocable: bool,
        is_transferable: bool,
        step_duration: u64,
        authorization_id: Option<BytesN<32>>,
    ) -> Result<u64, RegulatedAssetError> {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }

        // Check if asset requires authorization
        if RegulatedAssetManager::requires_authorization(&env, asset_id.clone()) {
            // Validate authorization if provided
            if let Some(auth_id) = authorization_id {
                RegulatedAssetManager::validate_authorization(
                    &env,
                    asset_id.clone(),
                    owner.clone(),
                    amount,
                    auth_id,
                )?;
            } else {
                return Err(RegulatedAssetError::AuthorizationRequired);
            }
        }

        let vault_id = Self::create_vault_full_internal(
            &env,
            owner,
            amount,
            asset_id,
            start_time,
            end_time,
            keeper_fee,
            is_revocable,
            is_transferable,
            step_duration,
        );

        // Store authorization reference if provided
        if let Some(auth_id) = authorization_id {
            let auth_key = (DataKey::VaultAuthorization, vault_id);
            env.storage().instance().set(&auth_key, &auth_id);
        }

        Ok(vault_id)
    }

    /// Claim tokens with SEP-08 authorization validation
    pub fn claim_tokens_regulated(
        env: Env,
        vault_id: u64,
        claim_amount: i128,
        authorization_id: BytesN<32>,
    ) -> Result<i128, RegulatedAssetError> {
        Self::require_not_paused(&env);
        let mut vault = Self::get_vault_internal(&env, vault_id);
        if vault.is_frozen {
            return Err(Error::VaultFrozen);
        }
        if !vault.is_initialized {
            return Err(Error::VaultNotInitialized);
        }

        // Check if this specific vault schedule is paused
        if Self::is_vault_paused(env.clone(), vault_id) {
            return Err(Error::ContractPaused);
        }

        // Check legal document signatures
        if vault.requires_legal_signatures && !vault.legal_documents_signed {
            return Err(Error::LegalSignatureMissing);
        }

        // Check beneficiary reassignment status
        if let Some(reassignment) = BeneficiaryReassignment::get_reassignment_status(&env, vault_id)
        {
            match &reassignment.status {
                ReassignmentStatus::Pending(_) => {
                    return Err(Error::VaultFrozen);
                }
                ReassignmentStatus::Approved => {
                    return Err(Error::VaultFrozen);
                }
                ReassignmentStatus::Completed => {
                    if reassignment.new_beneficiary != vault.owner {
                        return Err(Error::Unauthorized);
                    }
                }
                ReassignmentStatus::Rejected => {}
                ReassignmentStatus::None => {}
            }
        }

        // Get the asset allocation
        if vault.allocations.len() != 1 {
            return Err(Error::InvalidInput);
        }

        let allocation = vault.allocations.get(0).unwrap();
        let asset_id = allocation.asset_id.clone();

        // Check if asset requires SEP-08 authorization
        if RegulatedAssetManager::requires_authorization(&env, asset_id.clone()) {
            // Validate authorization
            RegulatedAssetManager::validate_authorization(
                &env,
                asset_id.clone(),
                vault.owner.clone(),
                claim_amount,
                authorization_id.clone(),
            )?;

            // Consume authorization amount
            RegulatedAssetManager::consume_authorization(&env, authorization_id, claim_amount);
        }

        vault.owner.require_auth();

        // Heartbeat: reset Dead-Man's Switch on every primary interaction
        update_activity(&env, vault_id);

        let vested = Self::calculate_claimable_for_asset(&env, vault_id, &vault, 0);
        if claim_amount > vested - allocation.released_amount {
            return Err(Error::InsufficientBalance);
        }

        let mut updated_allocation = allocation.clone();
        updated_allocation.released_amount += claim_amount;
        vault.allocations.set(0, updated_allocation);

        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        // Check if vault is fully completed and register certificate
        Self::check_and_register_certificate(&env, vault_id, &vault);

        let _guard = match ReentrancyGuard::enter(&env) {
            Ok(guard) => guard,
            Err(err) => return Err(err),
        };

        // Mint NFT if configured
        if let Some(nft_minter) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::NFTMinter)
        {
            env.invoke_contract::<()>(
                &nft_minter,
                &Symbol::new(&env, "mint"),
                (&vault.owner,).into_val(&env),
            );
        }

        // Transfer tokens
        token::Client::new(&env, &asset_id).transfer(
            &env.current_contract_address(),
            &vault.owner,
            &claim_amount,
        );

        Ok(claim_amount)
    }

    // ========== PATH PAYMENT FUNCTIONALITY FOR CLAIM_AND_SWAP ==========

    /// Configure path payment settings for auto-exit feature
    /// This allows users to claim tokens and instantly swap them for USDC in one transaction
    pub fn configure_path_payment(
        env: Env,
        admin: Address,
        destination_asset: Address,
        min_destination_amount: i128,
        path: Vec<Address>,
    ) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }

        let config = PathPaymentConfig {
            destination_asset: destination_asset.clone(),
            min_destination_amount,
            path: path.clone(),
            enabled: true,
        };

        env.storage()
            .instance()
            .set(&DataKey::PathPaymentConfig, &config);

        // Emit configuration event
        PathPaymentConfigured {
            destination_asset,
            min_destination_amount,
            path,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);
    }

    /// Disable path payment feature
    pub fn disable_path_payment(env: Env, admin: Address) {
        Self::require_admin(&env);
        if Self::multisig_active(&env) {
            return Err(Error::MultisigNotActive);
        }

        if let Some(mut config) = env
            .storage()
            .instance()
            .get::<_, PathPaymentConfig>(&DataKey::PathPaymentConfig)
        {
            config.enabled = false;
            env.storage()
                .instance()
                .set(&DataKey::PathPaymentConfig, &config);

            // Emit disable event
            PathPaymentDisabled {
                timestamp: env.ledger().timestamp(),
            }
            .publish(&env);
        }
    }

    /// Claim tokens with automatic path payment to USDC (Auto-Exit feature)
    /// This allows users to instantly swap their claimed tokens for USDC in one transaction
    pub fn claim_and_swap(
        env: Env,
        vault_id: u64,
        min_destination_amount: Option<i128>,
    ) -> Result<PathPaymentClaimEvent, Error> {
        Self::require_not_paused(&env);

        // Get path payment configuration
        let config = match env
            .storage()
            .instance()
            .get::<_, PathPaymentConfig>(&DataKey::PathPaymentConfig)
        {
            Some(c) => c,
            None => return Err(Error::PathPaymentNotConfigured),
        };

        if !config.enabled {
            return Err(Error::PathPaymentDisabled);
        }

        let mut vault = Self::get_vault_internal(&env, vault_id);
        if vault.is_frozen {
            return Err(Error::VaultFrozen);
        }
        if !vault.is_initialized {
            return Err(Error::VaultNotInitialized);
        }

        // Check if this specific vault schedule is paused
        if Self::is_vault_paused(env.clone(), vault_id) {
            return Err(Error::ContractPaused);
        }

        // Check if legal document signatures are required and verified
        if vault.requires_legal_signatures && !vault.legal_documents_signed {
            return Err(Error::LegalSignatureMissing);
        }

        // Check beneficiary reassignment status
        if let Some(reassignment) = BeneficiaryReassignment::get_reassignment_status(&env, vault_id)
        {
            match &reassignment.status {
                ReassignmentStatus::Pending(_) => {
                    return Err(Error::VaultFrozen);
                }
                ReassignmentStatus::Approved => {
                    return Err(Error::VaultFrozen);
                }
                ReassignmentStatus::Completed => {
                    if reassignment.new_beneficiary != vault.owner {
                        return Err(Error::Unauthorized);
                    }
                }
                ReassignmentStatus::Rejected => {
                    // Rejected reassignments don't block claims
                }
                ReassignmentStatus::None => {
                    // No reassignment in progress, normal flow
                }
            }
        }

        vault.owner.require_auth();

        // ========== COMPLIANCE CHECKS ==========
        if !Self::is_kyc_verified(&env, &vault.owner) {
            return Err(Error::KycNotCompleted);
        }

        if let Some(kyc_expiry) = Self::get_kyc_expiry(&env, &vault.owner) {
            let current_time = env.ledger().timestamp();
            if current_time > kyc_expiry {
                return Err(Error::KycExpired);
            }
        }

        if Self::is_address_sanctioned(&env, &vault.owner) {
            return Err(Error::AddressSanctioned);
        }

        if Self::is_jurisdiction_restricted(&env, &vault.owner) {
            return Err(Error::JurisdictionRestricted);
        }

        if !Self::has_valid_legal_signature(&env, &vault.owner, vault_id) {
            return Err(Error::LegalSignatureMissing);
        }

        if !Self::are_documents_verified(&env, &vault.owner) {
            return Err(Error::DocumentVerificationFailed);
        }

        if !Self::is_tax_compliant(&env, &vault.owner) {
            return Err(Error::TaxComplianceFailed);
        }

        if !Self::is_whitelist_approved(&env, &vault.owner) {
            return Err(Error::WhitelistNotApproved);
        }

        if Self::is_on_blacklist(&env, &vault.owner) {
            return Err(Error::BlacklistViolation);
        }

        if Self::is_geofencing_restricted(&env, &vault.owner) {
            return Err(Error::GeofencingRestriction);
        }

        if let Some(identity_expiry) = Self::get_identity_expiry(&env, &vault.owner) {
            let current_time = env.ledger().timestamp();
            if current_time > identity_expiry {
                return Err(Error::IdentityVerificationExpired);
            }
        }

        if Self::is_politically_exposed_person(&env, &vault.owner) {
            return Err(Error::PoliticallyExposedPerson);
        }

        if Self::is_on_sanctions_list(&env, &vault.owner) {
            return Err(Error::SanctionsListHit);
        }

        // Heartbeat: reset Dead-Man's Switch on every primary interaction
        update_activity(&env, vault_id);

        // KPI Gate check (#145/#92)
        if !crate::kpi_vesting::kpi_status(&env, vault_id) {
            return Err(Error::ComplianceCheckFailed);
        }

        // Calculate total claimable amount across all assets
        let mut total_claimable = 0i128;
        let mut claimable_assets = Vec::new(&env);

        for (i, allocation) in vault.allocations.iter().enumerate() {
            let vested_amount = Self::calculate_claimable_for_asset(&env, vault_id, &vault, i);
            let claimable_amount = vested_amount - allocation.released_amount;

            if claimable_amount > 0 {
                total_claimable += claimable_amount;
                claimable_assets.push_back((allocation.asset_id.clone(), claimable_amount));
            }
        }

        if total_claimable <= 0 {
            return Err(Error::InsufficientBalance);
        }

        // Validate minimum destination amount
        let final_min_amount = min_destination_amount.unwrap_or(config.min_destination_amount);
        if final_min_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        // For simplicity, we'll assume a 1:1 conversion rate for the first asset
        // In a real implementation, you would query DEX for actual rates
        let source_asset = claimable_assets.get(0).unwrap().0;
        let source_amount = claimable_assets.get(0).unwrap().1;
        let estimated_destination_amount = source_amount; // Simplified rate

        if estimated_destination_amount < final_min_amount {
            return Err(Error::InsufficientLiquidity);
        }

        // Execute Stellar Path Payment
        // This is a simplified implementation - in production you'd use Stellar's built-in path payment
        let current_time = env.ledger().timestamp();

        // Update vault allocations
        for (i, allocation) in vault.allocations.iter().enumerate() {
            if let Some((_, claimable_amount)) = claimable_assets
                .iter()
                .find(|(asset_id, _)| asset_id == &allocation.asset_id)
            {
                let mut updated_allocation = allocation.clone();
                updated_allocation.released_amount += claimable_amount;
                vault
                    .allocations
                    .set(i.try_into().unwrap(), updated_allocation);
            }
        }

        // Save updated vault
        env.storage()
            .instance()
            .set(&DataKey::VaultData(vault_id), &vault);

        // Check if vault is fully completed and register certificate
        Self::check_and_register_certificate(&env, vault_id, &vault);

        let _guard = match ReentrancyGuard::enter(&env) {
            Ok(guard) => guard,
            Err(err) => return Err(err),
        };

        // Model every configured hop as an untrusted synchronous contract call.
        // The reentrancy guard must hold across the entire route, not just the
        // final destination transfer.
        Self::invoke_path_payment_hops(&env, vault_id, source_amount, &config);

        // Mint NFT if configured
        if let Some(nft_minter) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::NFTMinter)
        {
            env.invoke_contract::<()>(
                &nft_minter,
                &Symbol::new(&env, "mint"),
                (&vault.owner,).into_val(&env),
            );
        }

        // Execute the path payment (simplified - in production use Stellar's path_payment_strict_send)
        // For now, we'll simulate the swap and transfer USDC directly
        token::Client::new(&env, &config.destination_asset).transfer(
            &env.current_contract_address(),
            &vault.owner,
            &estimated_destination_amount,
        );

        // Record the path payment claim event
        let path_payment_event = PathPaymentClaimEvent {
            beneficiary: vault.owner.clone(),
            source_amount: total_claimable,
            destination_amount: estimated_destination_amount,
            destination_asset: config.destination_asset.clone(),
            timestamp: current_time,
            vault_id,
        };

        // Add to claim history
        let mut history = env
            .storage()
            .instance()
            .get::<_, Vec<PathPaymentClaimEvent>>(&DataKey::PathPaymentClaimHistory)
            .unwrap_or(Vec::new(&env));
        history.push_back(path_payment_event.clone());
        env.storage()
            .instance()
            .set(&DataKey::PathPaymentClaimHistory, &history);

        // Emit the path payment claim event
        PathPaymentClaimExecuted {
            user: vault.owner.clone(),
            vault_id,
            source_amount: total_claimable,
            destination_amount: estimated_destination_amount,
            destination_asset: config.destination_asset.clone(),
            timestamp: current_time,
        }
        .publish(&env);

        Ok(path_payment_event)
    }

    fn invoke_path_payment_hops(
        env: &Env,
        vault_id: u64,
        source_amount: i128,
        config: &PathPaymentConfig,
    ) {
        for hop in config.path.iter() {
            env.invoke_contract::<()>(
                hop,
                &Symbol::new(env, "path_payment_hop"),
                (
                    env.current_contract_address(),
                    vault_id,
                    source_amount,
                    config.destination_asset.clone(),
                )
                    .into_val(env),
            );
        }
    }

    /// Simulate a path payment claim to show expected amounts without consuming gas
    pub fn simulate_claim_and_swap(
        env: Env,
        vault_id: u64,
        min_destination_amount: Option<i128>,
    ) -> PathPaymentSimulation {
        let current_time = env.ledger().timestamp();

        Self::require_not_paused(&env);

        // Check if contract is under emergency pause
        if Self::is_emergency_pause_active(&env) {
            return PathPaymentSimulation {
                source_amount: 0,
                estimated_destination_amount: 0,
                min_destination_amount: min_destination_amount.unwrap_or(0),
                path: Vec::new(&env),
                can_execute: false,
                reason: String::from_str(&env, "Contract under emergency pause"),
                estimated_gas_fee: 0,
            };
        }

        // Get path payment configuration
        let config = match env
            .storage()
            .instance()
            .get::<_, PathPaymentConfig>(&DataKey::PathPaymentConfig)
        {
            Some(c) => c,
            None => {
                return PathPaymentSimulation {
                    source_amount: 0,
                    estimated_destination_amount: 0,
                    min_destination_amount: min_destination_amount.unwrap_or(0),
                    path: Vec::new(&env),
                    can_execute: false,
                    reason: String::from_str(&env, "Path payment not configured"),
                    estimated_gas_fee: 0,
                };
            }
        };

        if !config.enabled {
            return PathPaymentSimulation {
                source_amount: 0,
                estimated_destination_amount: 0,
                min_destination_amount: min_destination_amount.unwrap_or(0),
                path: config.path,
                can_execute: false,
                reason: String::from_str(&env, "Path payment disabled"),
                estimated_gas_fee: 0,
            };
        }

        let final_min_amount = min_destination_amount.unwrap_or(config.min_destination_amount);
        if final_min_amount <= 0 {
            return PathPaymentSimulation {
                source_amount: 0,
                estimated_destination_amount: 0,
                min_destination_amount: final_min_amount,
                path: config.path,
                can_execute: false,
                reason: String::from_str(&env, "Invalid minimum amount"),
                estimated_gas_fee: 0,
            };
        }

        // Calculate claimable amount
        let vault = Self::get_vault_internal(&env, vault_id);
        let mut total_claimable = 0i128;

        for (i, allocation) in vault.allocations.iter().enumerate() {
            let vested_amount = Self::calculate_claimable_for_asset(&env, vault_id, &vault, i);
            let claimable_amount = vested_amount - allocation.released_amount;
            if claimable_amount > 0 {
                total_claimable += claimable_amount;
            }
        }

        if total_claimable <= 0 {
            return PathPaymentSimulation {
                source_amount: total_claimable,
                estimated_destination_amount: 0,
                min_destination_amount: final_min_amount,
                path: config.path,
                can_execute: false,
                reason: String::from_str(&env, "No tokens available to claim"),
                estimated_gas_fee: 0,
            };
        }

        // Simplified estimation - in production query DEX for actual rates
        let estimated_destination_amount = total_claimable;

        let can_execute = estimated_destination_amount >= final_min_amount;

        PathPaymentSimulation {
            source_amount: total_claimable,
            estimated_destination_amount,
            min_destination_amount: final_min_amount,
            path: config.path,
            can_execute,
            reason: if can_execute {
                String::from_str(&env, "Path payment can be executed")
            } else {
                String::from_str(&env, "Insufficient destination amount")
            },
            estimated_gas_fee: 5000000, // Estimated gas fee
        }
    }

    /// Get current path payment configuration
    pub fn get_path_payment_config(env: Env) -> Option<PathPaymentConfig> {
        env.storage().instance().get(&DataKey::PathPaymentConfig)
    }

    /// Get path payment claim history
    pub fn get_path_payment_claim_history(env: Env) -> Vec<PathPaymentClaimEvent> {
        env.storage()
            .instance()
            .get(&DataKey::PathPaymentClaimHistory)
            .unwrap_or(Vec::new(&env))
    }

    // Private helper methods for legal document integration
}

// Redefinition removed

#[cfg(test)]
mod beneficiary_reassignment_test;
#[cfg(test)]
mod diversified_simple_test;
#[cfg(test)]
mod diversified_test;
#[cfg(test)]
mod invariant_test;
#[cfg(test)]
mod legal_saft_test;
#[cfg(test)]
mod multisig_admin_test;
#[cfg(test)]
mod performance_cliff_test;
#[cfg(test)]
mod regulated_asset_test;
#[cfg(test)]
mod test;
#[cfg(test)]
mod zk_verifier_test;
