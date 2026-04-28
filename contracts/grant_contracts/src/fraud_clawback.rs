#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Vec, Map,
    crypto::Hash,
};

#[contract]
pub struct FraudClawback;

// Arbitration status and voting
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ArbitrationStatus {
    None = 0,
    DisputeRaised = 1,
    JurySelected = 2,
    VotingInProgress = 3,
    FraudConfirmed = 4,
    ChargesDismissed = 5,
    Expired = 6,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VoteType {
    SlashForFraud = 0,
    DismissCharges = 1,
    Abstain = 2,
}

impl VoteType {
    pub fn from_u32(value: u32) -> Result<Self, FraudClawbackError> {
        match value {
            0 => Ok(VoteType::SlashForFraud),
            1 => Ok(VoteType::DismissCharges),
            2 => Ok(VoteType::Abstain),
            _ => Err(FraudClawbackError::InvalidVote),
        }
    }

    pub fn to_u32(&self) -> u32 {
        *self as u32
    }
}

// Fraud dispute structure
#[derive(Clone)]
pub struct FraudDispute {
    pub dispute_id: u64,
    pub grant_id: u64,
    pub target_address: Address,
    pub raised_by: Address, // DAO or authorized entity
    pub evidence_hash: Hash, // Hash of off-chain evidence
    pub reasoning: String,
    pub status: u32,
    pub raised_at: u64,
    pub jury_selection_deadline: u64,
    pub voting_deadline: u64,
    pub required_jurors: u32,
    pub selected_jurors: Vec<Address>,
    pub votes: Map<Address, u32>,
    pub slash_amount: i128, // Amount to be clawed back if fraud is confirmed
    pub treasury_address: Address, // Where clawed funds go
    pub metadata: Map<String, String>,
}

// Juror information and voting record
#[derive(Clone)]
#[contracttype]
pub struct JurorRecord {
    pub juror_address: Address,
    pub total_disputes: u32,
    pub participated_disputes: u32,
    pub correct_votes: u32, // Votes that matched majority
    pub reputation_score: u32, // 0-100 based on accuracy
    pub is_active: bool,
    pub last_activity: u64,
}

// Security Council member registry
#[derive(Clone)]
#[contracttype]
pub struct SecurityCouncilMember {
    pub address: Address,
    pub joined_at: u64,
    pub reputation_score: u32,
    pub total_votes_cast: u32,
    pub is_active: bool,
    pub verification_status: String, // "verified", "pending", "suspended"
}

// Frozen grant state during arbitration
#[derive(Clone)]
pub struct FrozenGrant {
    pub grant_id: u64,
    pub original_recipient: Address,
    pub frozen_amount: i128, // Unvested amount frozen
    pub frozen_at: u64,
    pub dispute_id: u64,
    pub is_frozen: bool,
    pub original_status: u32, // Original grant status before freeze
}

// Data keys for storage
#[derive(Clone)]
#[contracttype]
pub enum FraudDataKey {
    Admin,
    DAO,
    Treasury,
    Dispute(u64),
    ActiveDisputes,
    ResolvedDisputes,
    JurorRecord(Address),
    SecurityCouncil,
    FrozenGrant(u64),
    PendingArbitration,
    JurorPool,
    GlobalStats,
}

#[contracterror]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum FraudClawbackError {
    NotAuthorized = 4000,
    DisputeNotFound = 4001,
    DisputeAlreadyExists = 4002,
    GrantNotActive = 4003,
    GrantAlreadyFrozen = 4004,
    InvalidJurorSelection = 4005,
    VotingNotStarted = 4006,
    VotingEnded = 4007,
    InvalidVote = 4008,
    JurorNotSelected = 4009,
    AlreadyVoted = 4010,
    InsufficientJurors = 4011,
    DisputeNotExpired = 4012,
    FraudNotConfirmed = 4013,
    MathOverflow = 4014,
    InvalidEvidence = 4015,
    JurorPoolEmpty = 4016,
    TreasuryNotSet = 4017,
    ClawbackFailed = 4018,
}

// Constants for arbitration
const JURY_SIZE: u32 = 5;
const VOTING_PERIOD_SECONDS: u64 = 7 * 24 * 60 * 60; // 7 days
const JURY_SELECTION_PERIOD_SECONDS: u64 = 24 * 60 * 60; // 1 day
const MIN_REPUTATION_FOR_JURY: u32 = 50;
const FRAUD_CONFIRMATION_THRESHOLD: f32 = 0.6; // 60% of jurors must agree

// Helper functions
fn read_admin(env: &Env) -> Result<Address, FraudClawbackError> {
    env.storage()
        .instance()
        .get(&FraudDataKey::Admin)
        .ok_or(FraudClawbackError::NotAuthorized)
}

fn read_dao(env: &Env) -> Result<Address, FraudClawbackError> {
    env.storage()
        .instance()
        .get(&FraudDataKey::DAO)
        .ok_or(FraudClawbackError::NotAuthorized)
}

fn read_treasury(env: &Env) -> Result<Address, FraudClawbackError> {
    env.storage()
        .instance()
        .get(&FraudDataKey::Treasury)
        .ok_or(FraudClawbackError::TreasuryNotSet)
}

fn require_dao_auth(env: &Env) -> Result<(), FraudClawbackError> {
    let dao = read_dao(env)?;
    dao.require_auth();
    Ok(())
}

fn read_dispute(env: &Env, dispute_id: u64) -> Result<FraudDispute, FraudClawbackError> {
    env.storage()
        .instance()
        .get(&FraudDataKey::Dispute(dispute_id))
        .ok_or(FraudClawbackError::DisputeNotFound)
}

fn write_dispute(env: &Env, dispute: &FraudDispute) {
    env.storage()
        .instance()
        .set(&FraudDataKey::Dispute(dispute.dispute_id), dispute);
}

fn read_frozen_grant(env: &Env, grant_id: u64) -> Result<FrozenGrant, FraudClawbackError> {
    env.storage()
        .instance()
        .get(&FraudDataKey::FrozenGrant(grant_id))
        .ok_or(FraudClawbackError::GrantNotActive)
}

fn write_frozen_grant(env: &Env, frozen_grant: &FrozenGrant) {
    env.storage()
        .instance()
        .set(&FraudDataKey::FrozenGrant(frozen_grant.grant_id), frozen_grant);
}

// Juror selection using cryptographic randomness
fn select_jurors(env: &Env, dispute_id: u64, required_count: u32) -> Result<Vec<Address>, FraudClawbackError> {
    let juror_pool = env.storage()
        .instance()
        .get(&FraudDataKey::JurorPool)
        .unwrap_or(Vec::<Address>::new(&env));

    if juror_pool.len() < required_count {
        return Err(FraudClawbackError::JurorPoolEmpty);
    }

    let mut selected_jurors = Vec::<Address>::new(&env);
    let mut available_jurors = juror_pool.clone();
    
    // Use dispute_id and current timestamp as entropy source
    let seed_bytes = [dispute_id.to_le_bytes(), env.ledger().timestamp().to_le_bytes()].concat();
    let seed = soroban_sdk::Bytes::from_slice(&env, &seed_bytes);
    let random_value = env.prng().seed(seed).gen();

    for _ in 0..required_count {
        if available_jurors.is_empty() {
            break;
        }
        
        let index = (random_value % available_jurors.len() as u64) as usize;
        let selected = available_jurors.get(index).unwrap().clone();
        selected_jurors.push_back(selected);
        available_jurors.remove(index);
    }

    Ok(selected_jurors)
}

// Check if voting threshold is met
fn check_voting_threshold(dispute: &FraudDispute) -> (bool, u32) {
    let total_votes = dispute.votes.len();
    if total_votes == 0 {
        return (false, 2); // Abstain
    }

    let mut fraud_votes = 0;
    let mut dismiss_votes = 0;

    for (_, vote_u32) in dispute.votes.iter() {
        match vote_u32 {
            0 => fraud_votes += 1, // SlashForFraud
            1 => dismiss_votes += 1, // DismissCharges
            2 => {} // Abstain
            _ => {}
        }
    }

    let fraud_ratio = fraud_votes as f32 / total_votes as f32;
    let dismiss_ratio = dismiss_votes as f32 / total_votes as f32;

    if fraud_ratio >= FRAUD_CONFIRMATION_THRESHOLD {
        (true, 0) // SlashForFraud
    } else if dismiss_ratio >= FRAUD_CONFIRMATION_THRESHOLD {
        (true, 1) // DismissCharges
    } else {
        (false, 2) // Abstain
    }
}

#[contractimpl]
impl FraudClawback {
    /// Initialize the fraud clawback system
    pub fn initialize(env: Env, admin: Address, dao: Address, treasury: Address) -> Result<(), FraudClawbackError> {
        if env.storage().instance().has(&FraudDataKey::Admin) {
            return Err(FraudClawbackError::NotAuthorized);
        }

        admin.require_auth();
        dao.require_auth();
        
        env.storage().instance().set(&FraudDataKey::Admin, &admin);
        env.storage().instance().set(&FraudDataKey::DAO, &dao);
        env.storage().instance().set(&FraudDataKey::Treasury, &treasury);
        
        // Initialize empty collections
        env.storage().instance().set(&FraudDataKey::ActiveDisputes, &Vec::<u64>::new(&env));
        env.storage().instance().set(&FraudDataKey::ResolvedDisputes, &Vec::<u64>::new(&env));
        env.storage().instance().set(&FraudDataKey::JurorPool, &Vec::<Address>::new(&env));
        env.storage().instance().set(&FraudDataKey::PendingArbitration, &Vec::<u64>::new(&env));

        Ok(())
    }

    /// Raise a fraud dispute against a grant recipient
    pub fn raise_fraud_dispute(
        env: Env,
        grant_id: u64,
        target_address: Address,
        evidence_hash: Hash,
        reasoning: String,
        slash_amount: i128,
        metadata: Map<String, String>,
    ) -> Result<u64, FraudClawbackError> {
        require_dao_auth(&env)?;

        // Check if grant is already frozen
        if env.storage().instance().has(&FraudDataKey::FrozenGrant(grant_id)) {
            return Err(FraudClawbackError::GrantAlreadyFrozen);
        }

        let now = env.ledger().timestamp();
        let dispute_id = now; // Use timestamp as unique ID

        // Create fraud dispute
        let dispute = FraudDispute {
            dispute_id,
            grant_id,
            target_address: target_address.clone(),
            raised_by: read_dao(&env)?,
            evidence_hash,
            reasoning: reasoning.clone(),
            status: 1, // DisputeRaised
            raised_at: now,
            jury_selection_deadline: now + JURY_SELECTION_PERIOD_SECONDS,
            voting_deadline: now + JURY_SELECTION_PERIOD_SECONDS + JURY_SELECTION_PERIOD_SECONDS,
            required_jurors: JURY_SIZE,
            selected_jurors: Vec::<Address>::new(&env),
            votes: Map::<Address, u32>::new(&env),
            slash_amount,
            treasury_address: read_treasury(&env)?,
            metadata,
        };

        // Store dispute
        write_dispute(&env, &dispute);

        // Add to active disputes
        let mut active_disputes = env.storage()
            .instance()
            .get(&FraudDataKey::ActiveDisputes)
            .unwrap_or(Vec::<u64>::new(&env));
        active_disputes.push_back(dispute_id);
        env.storage().instance().set(&FraudDataKey::ActiveDisputes, &active_disputes);

        // Freeze the grant immediately
        Self::freeze_grant(&env, grant_id, target_address.clone(), dispute_id)?;

        // Emit dispute raised event
        env.events().publish(
            (symbol_short!("fraud_dispute_raised"), dispute_id),
            (grant_id, target_address.clone(), reasoning, now),
        );

        Ok(dispute_id)
    }

    /// Freeze a grant during dispute
    fn freeze_grant(
        env: &Env,
        grant_id: u64,
        target_address: Address,
        dispute_id: u64,
    ) -> Result<(), FraudClawbackError> {
        // In a real implementation, this would interact with the grant contract
        // For now, we'll create a frozen grant record
        
        let now = env.ledger().timestamp();
        
        // Calculate frozen amount (unvested portion)
        // This would typically come from the grant contract
        let frozen_amount = 0i128; // Placeholder - would be calculated from grant state
        
        let frozen_grant = FrozenGrant {
            grant_id,
            original_recipient: target_address.clone(),
            frozen_amount,
            frozen_at: now,
            dispute_id,
            is_frozen: true,
            original_status: 0, // Would get from actual grant
        };

        write_frozen_grant(&env, &frozen_grant);

        // Emit grant frozen event
        env.events().publish(
            (symbol_short!("grant_frozen"), grant_id),
            (target_address, frozen_amount, dispute_id, now),
        );

        Ok(())
    }

    /// Select jury for a dispute
    pub fn select_jury(env: Env, dispute_id: u64) -> Result<(), FraudClawbackError> {
        require_dao_auth(&env)?;

        let mut dispute = read_dispute(&env, dispute_id)?;
        
        if dispute.status != 1 { // DisputeRaised
            return Err(FraudClawbackError::InvalidJurorSelection);
        }

        let now = env.ledger().timestamp();
        if now > dispute.jury_selection_deadline {
            return Err(FraudClawbackError::InvalidJurorSelection);
        }

        // Select jurors randomly
        let selected_jurors = select_jurors(&env, dispute_id, JURY_SIZE)?;
        
        if selected_jurors.len() < JURY_SIZE {
            return Err(FraudClawbackError::InsufficientJurors);
        }

        // Update dispute with selected jurors
        dispute.selected_jurors = selected_jurors.clone();
        dispute.status = 2; // JurySelected
        write_dispute(&env, &dispute);

        // Add to pending arbitration
        let mut pending = env.storage()
            .instance()
            .get(&FraudDataKey::PendingArbitration)
            .unwrap_or(Vec::<u64>::new(&env));
        pending.push_back(dispute_id);
        env.storage().instance().set(&FraudDataKey::PendingArbitration, &pending);

        // Emit jury selection event
        env.events().publish(
            (symbol_short!("jury_selected"), dispute_id),
            (selected_jurors, now),
        );

        Ok(())
    }

    /// Start voting period for a dispute
    pub fn start_voting(env: Env, dispute_id: u64) -> Result<(), FraudClawbackError> {
        require_dao_auth(&env)?;

        let mut dispute = read_dispute(&env, dispute_id)?;
        
        if dispute.status != 2 { // JurySelected
            return Err(FraudClawbackError::VotingNotStarted);
        }

        dispute.status = 3; // VotingInProgress
        write_dispute(&env, &dispute);

        // Emit voting started event
        env.events().publish(
            (symbol_short!("voting_started"), dispute_id),
            (env.ledger().timestamp(),),
        );

        Ok(())
    }

    /// Cast vote as a juror
    pub fn cast_vote(
        env: Env,
        dispute_id: u64,
        vote: u32,
    ) -> Result<(), FraudClawbackError> {
        let dispute = read_dispute(&env, dispute_id)?;
        
        if dispute.status != 3 { // VotingInProgress
            return Err(FraudClawbackError::VotingNotStarted);
        }

        let now = env.ledger().timestamp();
        if now > dispute.voting_deadline {
            return Err(FraudClawbackError::VotingEnded);
        }

        // In a real implementation, this would get the caller address
        // For now, we'll use a placeholder approach
        let voter = soroban_sdk::Address::from_string(&soroban_sdk::String::from_str(&env, "GD..."));

        // Check if voter is a selected juror
        if !dispute.selected_jurors.contains(&voter) {
            return Err(FraudClawbackError::JurorNotSelected);
        }

        // Check if already voted
        if dispute.votes.contains_key(voter.clone()) {
            return Err(FraudClawbackError::AlreadyVoted);
        }

        // Record vote
        let mut updated_dispute = dispute;
        updated_dispute.votes.set(voter.clone(), vote);
        write_dispute(&env, &updated_dispute);

        // Update juror record
        let vote_type = VoteType::from_u32(vote).unwrap_or(VoteType::Abstain);
        Self::update_juror_record(&env, &voter, dispute_id, vote_type);

        // Emit vote cast event
        env.events().publish(
            (symbol_short!("vote_cast"), dispute_id),
            (voter, vote, now),
        );

        // Check if voting is complete
        if updated_dispute.votes.len() >= updated_dispute.required_jurors {
            Self::resolve_dispute(&env, dispute_id)?;
        }

        Ok(())
    }

    /// Update juror's record
    fn update_juror_record(
        env: &Env,
        juror: &Address,
        dispute_id: u64,
        vote: VoteType,
    ) {
        // In a real implementation, this would update juror statistics
        // For now, we just emit an event
        env.events().publish(
            (symbol_short!("juror_updated"), juror.clone()),
            (dispute_id, vote as u32),
        );
    }

    /// Resolve a dispute after voting is complete
    fn resolve_dispute(env: &Env, dispute_id: u64) -> Result<(), FraudClawbackError> {
        let mut dispute = read_dispute(&env, dispute_id)?;
        
        let (threshold_met, outcome) = check_voting_threshold(&dispute);
        
        if !threshold_met {
            // Voting period expired without threshold, default to dismiss
            dispute.status = 5; // ChargesDismissed
        } else {
            match outcome {
                0 => { // SlashForFraud
                    dispute.status = 4; // FraudConfirmed
                    Self::execute_clawback(&env, &dispute)?;
                },
                1 => { // DismissCharges
                    dispute.status = 5; // ChargesDismissed
                    Self::unfreeze_grant(&env, dispute.grant_id)?;
                },
                2 => { // Abstain
                    dispute.status = 5; // ChargesDismissed
                    Self::unfreeze_grant(&env, dispute.grant_id)?;
                },
            }
        }

        // Update dispute status
        write_dispute(&env, &dispute);

        // Move from active to resolved
        let mut active_disputes = env.storage()
            .instance()
            .get(&FraudDataKey::ActiveDisputes)
            .unwrap_or(Vec::<u64>::new(&env));
        let index = active_disputes.iter().position(|id| id == dispute_id);
        if let Some(idx) = index {
            active_disputes.remove(idx.try_into().unwrap());
        }
        env.storage().instance().set(&FraudDataKey::ActiveDisputes, &active_disputes);

        let mut resolved_disputes = env.storage()
            .instance()
            .get(&FraudDataKey::ResolvedDisputes)
            .unwrap_or(Vec::<u64>::new(&env));
        resolved_disputes.push_back(dispute_id);
        env.storage().instance().set(&FraudDataKey::ResolvedDisputes, &resolved_disputes);

        // Emit resolution event
        env.events().publish(
            (symbol_short!("arbitration_resolved"), dispute_id),
            (outcome as u32, dispute.status as u32, env.ledger().timestamp()),
        );

        Ok(())
    }

    /// Execute clawback if fraud is confirmed
    fn execute_clawback(env: &Env, dispute: &FraudDispute) -> Result<(), FraudClawbackError> {
        let frozen_grant = read_frozen_grant(&env, dispute.grant_id)?;
        
        // In a real implementation, this would:
        // 1. Transfer frozen assets to treasury
        // 2. Update grant contract state
        // 3. Emit appropriate events
        
        // For now, we'll emit the clawback event
        env.events().publish(
            (symbol_short!("clawback_executed"), dispute.dispute_id),
            (dispute.grant_id, dispute.target_address.clone(), dispute.slash_amount, dispute.treasury_address.clone()),
        );

        // Emit fraud confirmation error
        env.events().publish(
            (symbol_short!("fraud_confirmed"), dispute.grant_id),
            (dispute.target_address.clone(), dispute.slash_amount),
        );

        Ok(())
    }

    /// Unfreeze grant if charges are dismissed
    fn unfreeze_grant(env: &Env, grant_id: u64) -> Result<(), FraudClawbackError> {
        let frozen_grant = read_frozen_grant(&env, grant_id)?;
        
        // In a real implementation, this would restore the grant to its original state
        
        // Remove frozen grant record
        env.storage().instance().remove(&FraudDataKey::FrozenGrant(grant_id));

        // Emit unfreeze event
        env.events().publish(
            (symbol_short!("grant_unfrozen"), grant_id),
            (frozen_grant.original_recipient, frozen_grant.frozen_amount),
        );

        Ok(())
    }

    /// Add juror to the pool
    pub fn add_juror(env: Env, juror_address: Address, reputation_score: u32) -> Result<(), FraudClawbackError> {
        require_dao_auth(&env)?;

        if reputation_score < MIN_REPUTATION_FOR_JURY {
            return Err(FraudClawbackError::InvalidJurorSelection);
        }

        let mut juror_pool = env.storage()
            .instance()
            .get(&FraudDataKey::JurorPool)
            .unwrap_or(Vec::<Address>::new(&env));

        if !juror_pool.contains(&juror_address) {
            juror_pool.push_back(juror_address.clone());
            env.storage().instance().set(&FraudDataKey::JurorPool, &juror_pool);
        }

        // Create juror record
        let juror_record = JurorRecord {
            juror_address: juror_address.clone(),
            total_disputes: 0,
            participated_disputes: 0,
            correct_votes: 0,
            reputation_score,
            is_active: true,
            last_activity: env.ledger().timestamp(),
        };

        env.storage()
            .instance()
            .set(&FraudDataKey::JurorRecord(juror_address), &juror_record);

        Ok(())
    }

    /// Get dispute information
    pub fn get_dispute(env: Env, dispute_id: u64) -> Result<FraudDispute, FraudClawbackError> {
        read_dispute(&env, dispute_id)
    }

    /// Check if grant is frozen
    pub fn is_grant_frozen(env: Env, grant_id: u64) -> Result<bool, FraudClawbackError> {
        match read_frozen_grant(&env, grant_id) {
            Ok(_) => Ok(true),
            Err(FraudClawbackError::GrantNotActive) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Get active disputes
    pub fn get_active_disputes(env: Env) -> Result<Vec<u64>, FraudClawbackError> {
        Ok(env.storage()
            .instance()
            .get(&FraudDataKey::ActiveDisputes)
            .unwrap_or(Vec::<u64>::new(&env)))
    }

    /// Get resolved disputes
    pub fn get_resolved_disputes(env: Env) -> Result<Vec<u64>, FraudClawbackError> {
        Ok(env.storage()
            .instance()
            .get(&FraudDataKey::ResolvedDisputes)
            .unwrap_or(Vec::<u64>::new(&env)))
    }

    /// Emergency function to handle expired disputes
    pub fn handle_expired_disputes(env: Env) -> Result<u32, FraudClawbackError> {
        require_dao_auth(&env)?;

        let active_disputes = env.storage()
            .instance()
            .get(&FraudDataKey::ActiveDisputes)
            .unwrap_or(Vec::<u64>::new(&env));

        let now = env.ledger().timestamp();
        let mut resolved_count = 0;

        for dispute_id in active_disputes.iter() {
            match read_dispute(&env, dispute_id) {
                Ok(dispute) => {
                    if now > dispute.voting_deadline {
                        // Auto-dismiss expired disputes
                        Self::resolve_dispute(&env, dispute_id)?;
                        resolved_count += 1;
                    }
                },
                Err(_) => continue,
            }
        }

        Ok(resolved_count)
    }

    /// Update DAO address
    pub fn update_dao(env: Env, new_dao: Address) -> Result<(), FraudClawbackError> {
        let current_dao = read_dao(&env)?;
        current_dao.require_auth();
        new_dao.require_auth();
        
        env.storage().instance().set(&FraudDataKey::DAO, &new_dao);

        Ok(())
    }

    /// Update treasury address
    pub fn update_treasury(env: Env, new_treasury: Address) -> Result<(), FraudClawbackError> {
        let current_dao = read_dao(&env)?;
        current_dao.require_auth();
        new_treasury.require_auth();
        
        env.storage().instance().set(&FraudDataKey::Treasury, &new_treasury);

        Ok(())
    }
}
