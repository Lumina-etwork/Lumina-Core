#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Map, Vec, U256,
};

#[contract]
pub struct FraudClawbackContract;

// Arbitration status flags
pub const DISPUTE_STATUS_RAISED: u32 = 0b00000001;
pub const DISPUTE_STATUS_FROZEN: u32 = 0b00000010;
pub const DISPUTE_STATUS_JURY_SELECTED: u32 = 0b00000100;
pub const DISPUTE_STATUS_VOTING: u32 = 0b00001000;
pub const DISPUTE_STATUS_RESOLVED: u32 = 0b00010000;
pub const DISPUTE_STATUS_DISMISSED: u32 = 0b00100000;
pub const DISPUTE_STATUS_CONFIRMED_FRAUD: u32 = 0b01000000;

// Voting thresholds
pub const JURY_SIZE: u32 = 5;
pub const VOTING_THRESHOLD: u32 = 3; // 3-of-5 majority
pub const VOTING_PERIOD_DAYS: u64 = 7; // 7 days for voting

#[derive(Clone)]
#[contracttype]
pub enum FraudDataKey {
    Admin,
    DAO,
    SecurityCouncil,
    Dispute(u64),
    JurorVote(Address, u64), // (juror_address, dispute_id)
    FrozenGrant(u64),
}

#[derive(Clone)]
#[contracttype]
pub struct FraudDispute {
    pub dispute_id: u64,
    pub target_grant_id: u64,
    pub target_beneficiary: Address,
    pub raised_by: Address, // DAO or authorized entity
    pub raised_timestamp: u64,
    pub status_mask: u32,
    pub evidence_hash: Option<String>,
    pub description: String,
    pub jury_members: Vec<Address>,
    pub votes_for_fraud: u32,
    pub votes_against: u32,
    pub voting_deadline: u64,
    pub resolved_timestamp: Option<u64>,
    pub resolution: FraudResolution,
}

#[derive(Clone)]
#[contracttype]
pub struct FraudResolution {
    pub is_fraud_confirmed: bool,
    pub slash_amount: i128,
    pub returned_to_treasury: bool,
    pub juror_votes: Vec<JurorVote>,
}

#[derive(Clone)]
#[contracttype]
pub struct JurorVote {
    pub juror: Address,
    pub vote: bool, // true for fraud, false for dismiss
    pub vote_timestamp: u64,
    pub cryptographic_signature: Option<String>,
}

#[derive(Clone)]
#[contracttype]
pub struct SecurityCouncilMember {
    pub address: Address,
    pub is_active: bool,
    pub risk_rating: u32, // 1-100
    pub cases_participated: u64,
    pub votes_cast: u64,
}

#[contracterror]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum FraudError {
    NotInitialized = 4001,
    NotAuthorized = 4002,
    DisputeNotFound = 4003,
    DisputeAlreadyExists = 4004,
    InvalidStatus = 4005,
    GrantNotFrozen = 4006,
    JurySelectionFailed = 4007,
    VotingPeriodExpired = 4008,
    InsufficientVotes = 4009,
    AlreadyVoted = 4010,
    InvalidJuror = 4011,
    GrantAlreadyFrozen = 4012,
    NoFrozenAssets = 4013,
    SecurityCouncilEmpty = 4014,
}

// Helper functions for dispute status
pub fn has_dispute_status(status_mask: u32, flag: u32) -> bool {
    (status_mask & flag) != 0
}

pub fn set_dispute_status(status_mask: u32, flag: u32) -> u32 {
    status_mask | flag
}

// Fraud Clawback implementation
pub struct FraudClawback;

impl FraudClawback {
    // Initialize fraud clawback system
    pub fn initialize(env: &Env, admin: Address, dao: Address) -> Result<(), FraudError> {
        if env.storage()
            .instance()
            .get::<FraudDataKey, Address>(&FraudDataKey::Admin)
            .is_some()
        {
            return Err(FraudError::NotInitialized);
        }

        env.storage()
            .instance()
            .set(&FraudDataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&FraudDataKey::DAO, &dao);

        // Initialize empty security council
        env.storage()
            .instance()
            .set(&FraudDataKey::SecurityCouncil, &Vec::<SecurityCouncilMember>::new(&env));

        Ok(())
    }

    // Read admin
    pub fn read_admin(env: &Env) -> Result<Address, FraudError> {
        env.storage()
            .instance()
            .get(&FraudDataKey::Admin)
            .ok_or(FraudError::NotInitialized)
    }

    // Read DAO
    pub fn read_dao(env: &Env) -> Result<Address, FraudError> {
        env.storage()
            .instance()
            .get(&FraudDataKey::DAO)
            .ok_or(FraudError::NotInitialized)
    }

    // Add security council member
    pub fn add_security_council_member(
        env: &Env,
        admin: Address,
        member: Address,
    ) -> Result<(), FraudError> {
        let stored_admin = Self::read_admin(env)?;
        if stored_admin != admin {
            return Err(FraudError::NotAuthorized);
        }

        let mut council: Vec<SecurityCouncilMember> = env.storage()
            .instance()
            .get(&FraudDataKey::SecurityCouncil)
            .unwrap_or_else(|| Vec::new(env));

        // Check if member already exists
        for existing_member in council.iter() {
            if existing_member.address == member {
                return Err(FraudError::InvalidJuror); // Member already exists
            }
        }

        let new_member = SecurityCouncilMember {
            address: member.clone(),
            is_active: true,
            risk_rating: 50, // Default reputation
            cases_participated: 0,
            votes_cast: 0,
        };

        council.push_back(new_member);
        env.storage()
            .instance()
            .set(&FraudDataKey::SecurityCouncil, &council);

        Ok(())
    }

    // Get active security council members
    pub fn get_active_council_members(env: &Env) -> Result<Vec<SecurityCouncilMember>, FraudError> {
        let council: Vec<SecurityCouncilMember> = env.storage()
            .instance()
            .get(&FraudDataKey::SecurityCouncil)
            .unwrap_or_else(|| Vec::new(env));

        if council.is_empty() {
            return Err(FraudError::SecurityCouncilEmpty);
        }

        let mut active_members = Vec::new(env);
        for member in council.iter() {
            if member.is_active {
                active_members.push_back(member.clone());
            }
        }

        if active_members.is_empty() {
            return Err(FraudError::SecurityCouncilEmpty);
        }

        Ok(active_members)
    }

    // Randomly select jury members
    pub fn select_jury_members(
        env: &Env,
        council_members: &Vec<SecurityCouncilMember>,
        exclude_addresses: &Vec<Address>,
    ) -> Result<Vec<Address>, FraudError> {
        if council_members.len() < JURY_SIZE {
            return Err(FraudError::JurySelectionFailed);
        }

        let mut eligible_members = Vec::new(env);
        for member in council_members.iter() {
            let mut is_excluded = false;
            for exclude_addr in exclude_addresses.iter() {
                if member.address == exclude_addr {
                    is_excluded = true;
                    break;
                }
            }
            if !is_excluded {
                eligible_members.push_back(member.address.clone());
            }
        }

        if eligible_members.len() < JURY_SIZE {
            return Err(FraudError::JurySelectionFailed);
        }

        // Simple pseudo-random selection based on timestamp
        let timestamp = env.ledger().timestamp();
        let mut jury = Vec::new(env);
        let mut used_indices: Vec<u64> = Vec::new(env);

        while jury.len() < JURY_SIZE {
            let index = (timestamp + jury.len() as u64) % eligible_members.len() as u64;
            
            let mut already_used = false;
            for used_idx in used_indices.iter() {
                if used_idx == index {
                    already_used = true;
                    break;
                }
            }

            if !already_used {
            jury.push_back(eligible_members.get(index.try_into().unwrap()).unwrap().clone());
                used_indices.push_back(index);
            }
        }

        Ok(jury)
    }

    // Raise fraud dispute
    pub fn raise_fraud_dispute(
        env: &Env,
        dao: Address,
        grant_id: u64,
        beneficiary: Address,
        evidence_hash: Option<String>,
        description: String,
    ) -> Result<u64, FraudError> {
        let stored_dao = Self::read_dao(env)?;
        if stored_dao != dao {
            return Err(FraudError::NotAuthorized);
        }

        // Check if grant is already frozen
        if env.storage()
            .instance()
            .get::<FraudDataKey, ()>(&FraudDataKey::FrozenGrant(grant_id))
            .is_some()
        {
            return Err(FraudError::GrantAlreadyFrozen);
        }

        // Generate dispute ID (simple increment)
        let dispute_id = env.ledger().sequence(); // Use ledger sequence as unique ID

        // Immediately freeze the grant
        env.storage()
            .instance()
            .set(&FraudDataKey::FrozenGrant(grant_id), &());

        // Get security council members
        let council_members = Self::get_active_council_members(env)?;
        
        // Select jury (exclude DAO and beneficiary)
        let mut exclude = Vec::new(env);
        exclude.push_back(dao.clone());
        exclude.push_back(beneficiary.clone());
        
        let jury_members = Self::select_jury_members(env, &council_members, &exclude)?;

        let current_time = env.ledger().timestamp();
        let voting_deadline = current_time + (VOTING_PERIOD_DAYS * 24 * 60 * 60); // 7 days in seconds

        let dispute = FraudDispute {
            dispute_id: dispute_id.into(),
            target_grant_id: grant_id,
            target_beneficiary: beneficiary,
            raised_by: dao,
            raised_timestamp: current_time,
            status_mask: set_dispute_status(0, DISPUTE_STATUS_RAISED) | DISPUTE_STATUS_FROZEN | DISPUTE_STATUS_JURY_SELECTED,
            evidence_hash,
            description,
            jury_members: jury_members.clone(),
            votes_for_fraud: 0,
            votes_against: 0,
            voting_deadline,
            resolved_timestamp: None,
            resolution: FraudResolution {
                is_fraud_confirmed: false,
                slash_amount: 0,
                returned_to_treasury: false,
                juror_votes: Vec::new(env),
            },
        };

        // Store dispute
        env.storage()
            .instance()
            .set(&FraudDataKey::Dispute(dispute_id.into()), &dispute);

        Ok(dispute_id.into())
    }

    // Cast vote as juror
    pub fn cast_jury_vote(
        env: &Env,
        juror: Address,
        dispute_id: u64,
        vote: bool,
        cryptographic_signature: Option<String>,
    ) -> Result<(), FraudError> {
        juror.require_auth();

        let mut dispute = Self::read_dispute(env, dispute_id)?;

        // Check if juror is part of the jury
        let mut is_valid_juror = false;
        for jury_member in dispute.jury_members.iter() {
            if jury_member == juror {
                is_valid_juror = true;
                break;
            }
        }

        if !is_valid_juror {
            return Err(FraudError::InvalidJuror);
        }

        // Check if already voted
        if env.storage()
            .instance()
            .get::<FraudDataKey, bool>(&FraudDataKey::JurorVote(juror.clone(), dispute_id))
            .is_some()
        {
            return Err(FraudError::AlreadyVoted);
        }

        // Check if voting period is still active
        let current_time = env.ledger().timestamp();
        if current_time > dispute.voting_deadline {
            return Err(FraudError::VotingPeriodExpired);
        }

        // Record the vote
        env.storage()
            .instance()
            .set(&FraudDataKey::JurorVote(juror.clone(), dispute_id), &true);

        let juror_vote = JurorVote {
            juror: juror.clone(),
            vote,
            vote_timestamp: current_time,
            cryptographic_signature,
        };

        dispute.resolution.juror_votes.push_back(juror_vote);

        if vote {
            dispute.votes_for_fraud += 1;
        } else {
            dispute.votes_against += 1;
        }

        // Check if voting threshold is reached
        let total_votes = dispute.votes_for_fraud + dispute.votes_against;
        if total_votes >= VOTING_THRESHOLD {
            Self::resolve_dispute(env, dispute_id, &mut dispute)?;
        }

        // Update dispute
        env.storage()
            .instance()
            .set(&FraudDataKey::Dispute(dispute_id), &dispute);

        Ok(())
    }

    // Resolve dispute
    pub fn resolve_dispute(
        env: &Env,
        dispute_id: u64,
        dispute: &mut FraudDispute,
    ) -> Result<(), FraudError> {
        let current_time = env.ledger().timestamp();
        
        // Check if voting deadline passed (auto-dismiss)
        if current_time > dispute.voting_deadline && (dispute.votes_for_fraud + dispute.votes_against) < JURY_SIZE {
            dispute.resolution.is_fraud_confirmed = false;
            dispute.status_mask = set_dispute_status(dispute.status_mask, DISPUTE_STATUS_DISMISSED);
        } else if dispute.votes_for_fraud >= VOTING_THRESHOLD {
            // Fraud confirmed
            dispute.resolution.is_fraud_confirmed = true;
            dispute.status_mask = set_dispute_status(dispute.status_mask, DISPUTE_STATUS_CONFIRMED_FRAUD);
            
            // In a real implementation, this would:
            // 1. Calculate slash amount from frozen grant
            // 2. Transfer tokens back to treasury
            // 3. Update grant status to TERMINATED_FOR_FRAUD
            dispute.resolution.returned_to_treasury = true;
        } else {
            // Dismiss charges
            dispute.resolution.is_fraud_confirmed = false;
            dispute.status_mask = set_dispute_status(dispute.status_mask, DISPUTE_STATUS_DISMISSED);
        }

        dispute.status_mask = set_dispute_status(dispute.status_mask, DISPUTE_STATUS_RESOLVED);
        dispute.resolved_timestamp = Some(current_time);

        // Unfreeze the grant
        env.storage()
            .instance()
            .remove::<FraudDataKey>(&FraudDataKey::FrozenGrant(dispute.target_grant_id));

        Ok(())
    }

    // Read dispute
    pub fn read_dispute(env: &Env, dispute_id: u64) -> Result<FraudDispute, FraudError> {
        env.storage()
            .instance()
            .get(&FraudDataKey::Dispute(dispute_id))
            .ok_or(FraudError::DisputeNotFound)
    }

    // Check if grant is frozen
    pub fn is_grant_frozen(env: &Env, grant_id: u64) -> bool {
        env.storage()
            .instance()
            .get::<FraudDataKey, ()>(&FraudDataKey::FrozenGrant(grant_id))
            .is_some()
    }

    // Get all active disputes
    pub fn get_active_disputes(env: &Env) -> Vec<FraudDispute> {
        let mut active_disputes = Vec::new(env);
        
        // This is a simplified version - in practice you'd maintain an index
        // For now, we'll return an empty vector as the implementation would need
        // to iterate through all possible dispute IDs
        
        active_disputes
    }
}

#[contractimpl]
impl FraudClawbackContract {
    // Initialize system
    pub fn initialize(env: Env, admin: Address, dao: Address) -> Result<(), FraudError> {
        FraudClawback::initialize(&env, admin, dao)
    }

    // Add security council member
    pub fn add_security_council_member(
        env: Env,
        admin: Address,
        member: Address,
    ) -> Result<(), FraudError> {
        FraudClawback::add_security_council_member(&env, admin, member)
    }

    // Raise fraud dispute
    pub fn raise_fraud_dispute(
        env: Env,
        dao: Address,
        grant_id: u64,
        beneficiary: Address,
        evidence_hash: Option<String>,
        description: String,
    ) -> Result<u64, FraudError> {
        FraudClawback::raise_fraud_dispute(&env, dao, grant_id, beneficiary, evidence_hash, description)
    }

    // Cast jury vote
    pub fn cast_jury_vote(
        env: Env,
        juror: Address,
        dispute_id: u64,
        vote: bool,
        cryptographic_signature: Option<String>,
    ) -> Result<(), FraudError> {
        FraudClawback::cast_jury_vote(&env, juror, dispute_id, vote, cryptographic_signature)
    }

    // Check if grant is frozen
    pub fn is_grant_frozen(env: Env, grant_id: u64) -> bool {
        FraudClawback::is_grant_frozen(&env, grant_id)
    }

    // Get dispute info
    pub fn get_dispute(env: Env, dispute_id: u64) -> Result<FraudDispute, FraudError> {
        FraudClawback::read_dispute(&env, dispute_id)
    }

    // Get active disputes
    pub fn get_active_disputes(env: Env) -> Vec<FraudDispute> {
        FraudClawback::get_active_disputes(&env)
    }

    // Get security council members
    pub fn get_security_council(env: Env) -> Result<Vec<SecurityCouncilMember>, FraudError> {
        FraudClawback::get_active_council_members(&env)
    }
}
