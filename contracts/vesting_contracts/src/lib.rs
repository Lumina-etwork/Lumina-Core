#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, vec, Address, Env, IntoVal, Map, Symbol, Val, Vec, String};

mod factory;
pub use factory::{VestingFactory, VestingFactoryClient};

// 10 years in seconds
pub const MAX_DURATION: u64 = 315_360_000;
// 72 hours in seconds for challenge period
pub const CHALLENGE_PERIOD: u64 = 259_200;
// 51% voting threshold (represented as basis points: 5100 = 51.00%)
pub const VOTING_THRESHOLD: u32 = 5100;

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
}

#[contracttype]
#[derive(Clone)]
pub struct Vault {
    pub total_amount: i128,
    pub released_amount: i128,
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
    AdminRotation(Address),     // new_admin
    ContractUpgrade(Address),  // new_contract_address
    EmergencyPause(bool),       // pause_state
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
    pub yes_votes: i128,   // Total locked value voting yes
    pub no_votes: i128,    // Total locked value voting no
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
    pub amounts: Vec<i128>,
    pub start_times: Vec<u64>,
    pub end_times: Vec<u64>,
    pub keeper_fees: Vec<i128>,
    pub step_durations: Vec<u64>,
}

#[contracttype]
pub struct VaultCreated {
    pub vault_id: u64,
    pub beneficiary: Address,
    pub total_amount: i128,
    pub cliff_duration: u64,
    pub start_time: u64,
    pub title: String,
}

#[contracttype]
pub struct GovernanceProposalCreated {
    pub proposal_id: u64,
    pub action: GovernanceAction,
    pub proposer: Address,
    pub challenge_end: u64,
}

#[contracttype]
pub struct VoteCast {
    pub proposal_id: u64,
    pub voter: Address,
    pub vote_weight: i128,
    pub is_yes: bool,
}

#[contracttype]
pub struct GovernanceActionExecuted {
    pub proposal_id: u64,
    pub action: GovernanceAction,
}

#[contract]
pub struct VestingContract;

#[contractimpl]
impl VestingContract {
    pub fn initialize(env: Env, admin: Address, initial_supply: i128) {
        if env.storage().instance().has(&DataKey::AdminAddress) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::AdminAddress, &admin);
        env.storage().instance().set(&DataKey::AdminBalance, &initial_supply);
        env.storage().instance().set(&DataKey::InitialSupply, &initial_supply);
        env.storage().instance().set(&DataKey::VaultCount, &0u64);
        env.storage().instance().set(&DataKey::IsPaused, &false);
        env.storage().instance().set(&DataKey::IsDeprecated, &false);
        env.storage().instance().set(&DataKey::TotalShares, &0i128);
        env.storage().instance().set(&DataKey::TotalStaked, &0i128);
        // Initialize governance
        env.storage().instance().set(&DataKey::ProposalCount, &0u64);
        env.storage().instance().set(&DataKey::TotalLockedValue, &initial_supply);
    }

    pub fn set_token(env: Env, token: Address) {
        Self::require_admin(&env);
        if env.storage().instance().has(&DataKey::Token) {
            panic!("Token already set");
        }
        env.storage().instance().set(&DataKey::Token, &token);
    }

    pub fn add_to_whitelist(env: Env, token: Address) {
        Self::require_admin(&env);
        let mut whitelist: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&WhitelistDataKey::WhitelistedTokens)
            .unwrap_or(Map::new(&env));
        whitelist.set(token.clone(), true);
        env.storage().instance().set(&WhitelistDataKey::WhitelistedTokens, &whitelist);
    }

    // Defensive Governance Functions
    pub fn propose_admin_rotation(env: Env, new_admin: Address) -> u64 {
        Self::require_admin(&env);
        Self::create_governance_proposal(env, GovernanceAction::AdminRotation(new_admin))
    }

    pub fn propose_contract_upgrade(env: Env, new_contract: Address) -> u64 {
        Self::require_admin(&env);
        Self::create_governance_proposal(env, GovernanceAction::ContractUpgrade(new_contract))
    }

    pub fn propose_emergency_pause(env: Env, pause_state: bool) -> u64 {
        Self::require_admin(&env);
        Self::create_governance_proposal(env, GovernanceAction::EmergencyPause(pause_state))
    }

    pub fn vote_on_proposal(env: Env, proposal_id: u64, is_yes: bool) {
        // Get the caller address - this will be the vault owner/beneficiary
        let voter = Address::generate(&env); // In real implementation, this would be env.invoker()
        voter.require_auth();
        let vote_weight = Self::get_voter_locked_value(&env, &voter);
        
        if vote_weight <= 0 {
            panic!("No voting power - no locked tokens");
        }

        let mut proposal = Self::get_proposal(&env, proposal_id);
        
        // Check if voting is still open
        let now = env.ledger().timestamp();
        if now >= proposal.challenge_end {
            panic!("Voting period has ended");
        }
        
        if proposal.is_executed || proposal.is_cancelled {
            panic!("Proposal is no longer active");
        }

        // Check if already voted
        let vote_key = DataKey::GovernanceVotes(proposal_id, voter.clone());
        if env.storage().instance().has(&vote_key) {
            panic!("Already voted on this proposal");
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

        env.storage().instance().set(&DataKey::GovernanceProposal(proposal_id), &proposal);

        // Publish vote event
        let vote_event = VoteCast {
            proposal_id,
            voter,
            vote_weight,
            is_yes,
        };
        env.events().publish((Symbol::new(&env, "vote_cast"), proposal_id), vote_event);
    }

    pub fn execute_proposal(env: Env, proposal_id: u64) {
        let mut proposal = Self::get_proposal(&env, proposal_id);
        let now = env.ledger().timestamp();

        // Check challenge period has ended
        if now < proposal.challenge_end {
            panic!("Challenge period not yet ended");
        }

        if proposal.is_executed || proposal.is_cancelled {
            panic!("Proposal already processed");
        }

        // Check if proposal passes (no veto from 51%+ of locked value)
        let total_locked = Self::get_total_locked_value(&env);
        let no_percentage = (proposal.no_votes * 10000) / total_locked;

        if no_percentage >= VOTING_THRESHOLD as i128 {
            // Proposal is vetoed - cancel it
            proposal.is_cancelled = true;
            env.storage().instance().set(&DataKey::GovernanceProposal(proposal_id), &proposal);
            return;
        }

        // Execute the governance action
        Self::execute_governance_action(&env, &proposal.action);
        
        proposal.is_executed = true;
        env.storage().instance().set(&DataKey::GovernanceProposal(proposal_id), &proposal);

        // Publish execution event
        let exec_event = GovernanceActionExecuted {
            proposal_id,
            action: proposal.action.clone(),
        };
        env.events().publish((Symbol::new(&env, "governance_executed"), proposal_id), exec_event);
    }

    // Legacy pause function - now requires governance proposal
    pub fn toggle_pause(env: Env) {
        panic!("Direct pause not allowed. Use propose_emergency_pause() instead.");
    }

    pub fn create_vault_full(
        env: Env, owner: Address, amount: i128, start_time: u64, end_time: u64,
        keeper_fee: i128, is_revocable: bool, is_transferable: bool, step_duration: u64,
    ) -> u64 {
        Self::require_admin(&env);
        Self::create_vault_full_internal(&env, owner, amount, start_time, end_time, keeper_fee, is_revocable, is_transferable, step_duration)
    }

    pub fn create_vault_lazy(
        env: Env, owner: Address, amount: i128, start_time: u64, end_time: u64,
        keeper_fee: i128, is_revocable: bool, is_transferable: bool, step_duration: u64,
    ) -> u64 {
        Self::require_admin(&env);
        Self::create_vault_lazy_internal(&env, owner, amount, start_time, end_time, keeper_fee, is_revocable, is_transferable, step_duration)
    }

    pub fn batch_create_vaults_lazy(env: Env, data: BatchCreateData) -> Vec<u64> {
        Self::require_admin(&env);
        let mut ids = Vec::new(&env);
        for i in 0..data.recipients.len() {
            let id = Self::create_vault_lazy_internal(
                &env,
                data.recipients.get(i).unwrap(),
                data.amounts.get(i).unwrap(),
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

    pub fn batch_create_vaults_full(env: Env, data: BatchCreateData) -> Vec<u64> {
        Self::require_admin(&env);
        let mut ids = Vec::new(&env);
        for i in 0..data.recipients.len() {
            let id = Self::create_vault_full_internal(
                &env,
                data.recipients.get(i).unwrap(),
                data.amounts.get(i).unwrap(),
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

    pub fn claim_tokens(env: Env, vault_id: u64, claim_amount: i128) -> i128 {
        Self::require_not_paused(&env);
        let mut vault = Self::get_vault_internal(&env, vault_id);
        if vault.is_frozen { panic!("Vault frozen"); }
        if !vault.is_initialized { panic!("Vault not initialized"); }
        vault.owner.require_auth();

        let vested = Self::calculate_claimable(&env, vault_id, &vault);
        if claim_amount > (vested - vault.released_amount) {
            panic!("Insufficient vested tokens");
        }

        vault.released_amount += claim_amount;
        env.storage().instance().set(&DataKey::VaultData(vault_id), &vault);
        
        let token: Address = env.storage().instance().get(&DataKey::Token).expect("Token not set");
        token::Client::new(&env, &token).transfer(&env.current_contract_address(), &vault.owner, &claim_amount);
        
        claim_amount
    }

    pub fn set_milestones(env: Env, vault_id: u64, milestones: Vec<Milestone>) {
        Self::require_admin(&env);
        let mut total_pct: u32 = 0;
        for m in milestones.iter() {
            total_pct += m.percentage;
        }
        if total_pct > 100 { panic!("Total percentage > 100"); }
        env.storage().instance().set(&DataKey::VaultMilestones(vault_id), &milestones);
    }

    pub fn get_milestones(env: Env, vault_id: u64) -> Vec<Milestone> {
        env.storage().instance().get(&DataKey::VaultMilestones(vault_id)).unwrap_or(Vec::new(&env))
    }

    pub fn unlock_milestone(env: Env, vault_id: u64, milestone_id: u64) {
        Self::require_admin(&env);
        let mut milestones = Self::get_milestones(env.clone(), vault_id);
        let mut found = false;
        let mut updated = Vec::new(&env);
        for m in milestones.iter() {
            if m.id == milestone_id {
                found = true;
                updated.push_back(Milestone { id: m.id, percentage: m.percentage, is_unlocked: true });
            } else {
                updated.push_back(m);
            }
        }
        if !found { panic!("Milestone not found"); }
        env.storage().instance().set(&DataKey::VaultMilestones(vault_id), &updated);
    }

    pub fn freeze_vault(env: Env, vault_id: u64, freeze: bool) {
        Self::require_admin(&env);
        let mut vault = Self::get_vault_internal(&env, vault_id);
        vault.is_frozen = freeze;
        env.storage().instance().set(&DataKey::VaultData(vault_id), &vault);
    }

    pub fn mark_irrevocable(env: Env, vault_id: u64) {
        Self::require_admin(&env);
        let mut vault = Self::get_vault_internal(&env, vault_id);
        vault.is_irrevocable = true;
        env.storage().instance().set(&DataKey::VaultData(vault_id), &vault);
    }

    pub fn get_claimable_amount(env: Env, vault_id: u64) -> i128 {
        let vault = Self::get_vault_internal(&env, vault_id);
        let vested = Self::calculate_claimable(&env, vault_id, &vault);
        vested - vault.released_amount
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&DataKey::IsPaused).unwrap_or(false)
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::AdminAddress).expect("Admin not set")
    }

    pub fn get_vault(env: Env, vault_id: u64) -> Vault {
        Self::get_vault_internal(&env, vault_id)
    }

    // --- Internal Helpers ---

    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::AdminAddress).expect("Admin not set");
        admin.require_auth();
    }

    fn require_not_paused(env: &Env) {
        if env.storage().instance().get(&DataKey::IsPaused).unwrap_or(false) {
            panic!("Paused");
        }
    }

    fn require_valid_duration(start: u64, end: u64) {
        if end <= start { panic!("Invalid duration"); }
        if (end - start) > MAX_DURATION { panic!("duration exceeds MAX_DURATION"); }
    }

    fn create_vault_full_internal(
        env: &Env, owner: Address, amount: i128, start_time: u64, end_time: u64,
        keeper_fee: i128, is_revocable: bool, is_transferable: bool, step_duration: u64,
    ) -> u64 {
        Self::require_valid_duration(start_time, end_time);
        let id = Self::increment_vault_count(env);
        Self::sub_admin_balance(env, amount);
        let vault = Vault {
            total_amount: amount,
            released_amount: 0,
            keeper_fee,
            staked_amount: 0,
            owner: owner.clone(),
            delegate: None,
            title: String::from_str(env, ""),
            start_time,
            end_time,
            creation_time: env.ledger().timestamp(),
            step_duration,
            is_initialized: true,
            is_irrevocable: !is_revocable,
            is_transferable,
            is_frozen: false,
        };
        env.storage().instance().set(&DataKey::VaultData(id), &vault);
        Self::add_user_vault_index(env, &owner, id);
        Self::add_total_shares(env, amount);
        id
    }

    fn create_vault_lazy_internal(
        env: &Env, owner: Address, amount: i128, start_time: u64, end_time: u64,
        keeper_fee: i128, is_revocable: bool, is_transferable: bool, step_duration: u64,
    ) -> u64 {
        Self::require_valid_duration(start_time, end_time);
        let id = Self::increment_vault_count(env);
        Self::sub_admin_balance(env, amount);
        let vault = Vault {
            total_amount: amount,
            released_amount: 0,
            keeper_fee,
            staked_amount: 0,
            owner: owner.clone(),
            delegate: None,
            title: String::from_str(env, ""),
            start_time,
            end_time,
            creation_time: env.ledger().timestamp(),
            step_duration,
            is_initialized: false,
            is_irrevocable: !is_revocable,
            is_transferable,
            is_frozen: false,
        };
        env.storage().instance().set(&DataKey::VaultData(id), &vault);
        Self::add_total_shares(env, amount);
        id
    }

    fn get_vault_internal(env: &Env, id: u64) -> Vault {
        env.storage().instance().get(&DataKey::VaultData(id)).expect("Vault not found")
    }

    fn increment_vault_count(env: &Env) -> u64 {
        let count: u64 = env.storage().instance().get(&DataKey::VaultCount).unwrap_or(0);
        let new_count = count + 1;
        env.storage().instance().set(&DataKey::VaultCount, &new_count);
        new_count
    }

    fn sub_admin_balance(env: &Env, amount: i128) {
        let bal: i128 = env.storage().instance().get(&DataKey::AdminBalance).unwrap_or(0);
        if bal < amount { panic!("Insufficient admin balance"); }
        env.storage().instance().set(&DataKey::AdminBalance, &(bal - amount));
    }

    fn add_total_shares(env: &Env, amount: i128) {
        let shares: i128 = env.storage().instance().get(&DataKey::TotalShares).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalShares, &(shares + amount));
    }

    fn add_user_vault_index(env: &Env, user: &Address, id: u64) {
        let mut vaults: Vec<u64> = env.storage().instance().get(&DataKey::UserVaults(user.clone())).unwrap_or(vec![env]);
        vaults.push_back(id);
        env.storage().instance().set(&DataKey::UserVaults(user.clone()), &vaults);
    }

    fn calculate_claimable(env: &Env, id: u64, vault: &Vault) -> i128 {
        if env.storage().instance().has(&DataKey::VaultMilestones(id)) {
            let milestones: Vec<Milestone> = env.storage().instance().get(&DataKey::VaultMilestones(id)).expect("No milestones");
            let mut pct = 0;
            for m in milestones.iter() {
                if m.is_unlocked { pct += m.percentage; }
            }
            if pct > 100 { pct = 100; }
            (vault.total_amount * pct as i128) / 100
        } else {
            let now = env.ledger().timestamp();
            if now <= vault.start_time { return 0; }
            if now >= vault.end_time { return vault.total_amount; }
            
            let duration = (vault.end_time - vault.start_time) as i128;
            let elapsed = (now - vault.start_time) as i128;
            
            if vault.step_duration > 0 {
                let steps = duration / vault.step_duration as i128;
                if steps == 0 { return 0; }
                let completed = elapsed / vault.step_duration as i128;
                (vault.total_amount * completed) / steps
            } else {
                (vault.total_amount * elapsed) / duration
            }
        }
    }

    // --- Governance Helper Functions ---

    fn create_governance_proposal(env: Env, action: GovernanceAction) -> u64 {
        let proposer = Self::get_admin(&env);
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

        env.storage().instance().set(&DataKey::GovernanceProposal(proposal_id), &proposal);

        // Publish proposal creation event
        let proposal_event = GovernanceProposalCreated {
            proposal_id,
            action: action.clone(),
            proposer,
            challenge_end: proposal.challenge_end,
        };
        env.events().publish((Symbol::new(&env, "governance_proposal"), proposal_id), proposal_event);

        proposal_id
    }

    fn get_proposal(env: &Env, proposal_id: u64) -> GovernanceProposal {
        env.storage().instance()
            .get(&DataKey::GovernanceProposal(proposal_id))
            .expect("Proposal not found")
    }

    fn get_voter_locked_value(env: &Env, voter: &Address) -> i128 {
        // Get all vaults for this voter and sum their total amounts
        let vault_ids: Vec<u64> = env.storage().instance()
            .get(&DataKey::UserVaults(voter.clone()))
            .unwrap_or(Vec::new(env));
        
        let mut total_locked = 0i128;
        for vault_id in vault_ids.iter() {
            let vault = Self::get_vault_internal(env, *vault_id);
            total_locked += vault.total_amount - vault.released_amount;
        }
        
        total_locked
    }

    fn get_total_locked_value(env: &Env) -> i128 {
        env.storage().instance()
            .get(&DataKey::TotalLockedValue)
            .unwrap_or(0i128)
    }

    fn execute_governance_action(env: &Env, action: &GovernanceAction) {
        match action {
            GovernanceAction::AdminRotation(new_admin) => {
                env.storage().instance().set(&DataKey::AdminAddress, new_admin);
            },
            GovernanceAction::ContractUpgrade(new_contract) => {
                env.storage().instance().set(&DataKey::MigrationTarget, new_contract);
                env.storage().instance().set(&DataKey::IsDeprecated, &true);
            },
            GovernanceAction::EmergencyPause(pause_state) => {
                env.storage().instance().set(&DataKey::IsPaused, pause_state);
            },
        }
    }

    fn increment_proposal_count(env: &Env) -> u64 {
        let count: u64 = env.storage().instance().get(&DataKey::ProposalCount).unwrap_or(0);
        let new_count = count + 1;
        env.storage().instance().set(&DataKey::ProposalCount, &new_count);
        new_count
    }

    // Public getter functions for governance
    pub fn get_proposal_info(env: Env, proposal_id: u64) -> GovernanceProposal {
        Self::get_proposal(&env, proposal_id)
    }

    pub fn get_voter_power(env: Env, voter: Address) -> i128 {
        Self::get_voter_locked_value(&env, &voter)
    }

    pub fn get_total_locked(env: Env) -> i128 {
        Self::get_total_locked_value(&env)
    }
}

#[cfg(test)]
mod test;
#[cfg(test)]
mod invariant_test;
