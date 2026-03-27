#![no_std]
use soroban_sdk::{
    contract, contractimpl, vec, Env, String, Vec, Map, Symbol, Address, 
    token, TryFromVal, try_from_val, ConversionError
};

#[contract]
pub struct VestingContract;

// Storage keys for efficient access
const VAULT_COUNT: Symbol = Symbol::new(&"VAULT_COUNT");
const VAULT_DATA: Symbol = Symbol::new(&"VAULT_DATA");
const USER_VAULTS: Symbol = Symbol::new(&"USER_VAULTS");
const INITIAL_SUPPLY: Symbol = Symbol::new(&"INITIAL_SUPPLY");
const ADMIN_BALANCE: Symbol = Symbol::new(&"ADMIN_BALANCE");
const REQUIRED_SBT_ADDRESS: Symbol = Symbol::new(&"REQUIRED_SBT_ADDRESS");
const MILESTONE_EVENTS: Symbol = Symbol::new(&"MILESTONE_EVENTS");
const MILESTONE_VAULTS: Symbol = Symbol::new(&"MILESTONE_VAULTS");

// Vault structure with lazy initialization
#[contracttype]
pub struct Vault {
    pub owner: Address,
    pub total_amount: i128,
    pub released_amount: i128,
    pub start_time: u64,
    pub end_time: u64,
    pub is_initialized: bool, // Lazy initialization flag
}

#[contracttype]
pub struct BatchCreateData {
    pub recipients: Vec<Address>,
    pub amounts: Vec<i128>,
    pub start_times: Vec<u64>,
    pub end_times: Vec<u64>,
}

#[contracttype]
pub struct SplitClaimData {
    pub vault_id: u64,
    pub secondary_address: Address,
    pub split_percentage: u32, // Percentage for secondary address (0-100)
}

#[contracttype]
pub struct MilestoneEvent {
    pub milestone_id: u32,
    pub is_triggered: bool,
    pub trigger_time: u64,
    pub triggered_by: Address,
}

#[contracttype]
pub struct MilestoneVault {
    pub vault_id: u64,
    pub milestones: Vec<u32>, // Percentage for each milestone (e.g., [25, 25, 50])
    pub current_milestone: u32,
    pub milestone_events: Map<u32, MilestoneEvent>,
}

#[contractimpl]
impl VestingContract {
    // Helper function to bump storage TTL only if needed (within 30 days of expiration)
    fn bump_if_needed(env: &Env) {
        let max_ttl = env.storage().instance().max_ttl();
        let current_ledger = env.ledger().sequence();
        
        // Only bump if we're within 30 days (720*30 ledgers assuming 5s per ledger)
        let threshold = max_ttl - (720 * 30);
        
        if current_ledger >= threshold {
            env.storage().instance().extend_ttl(max_ttl, max_ttl);
        }
    }
    // Initialize contract with initial supply
    pub fn initialize(env: Env, _admin: Address, initial_supply: i128) {
        // Set initial supply
        env.storage().instance().set(&INITIAL_SUPPLY, &initial_supply);
        
        // Set admin balance (initially all tokens go to admin)
        env.storage().instance().set(&ADMIN_BALANCE, &initial_supply);
        
        // Initialize vault count
        env.storage().instance().set(&VAULT_COUNT, &0u64);
    }
    
    // Set required SBT address for DID gating
    pub fn set_required_sbt(env: Env, sbt_address: Address) {
        Self::bump_if_needed(&env);
        env.storage().instance().set(&REQUIRED_SBT_ADDRESS, &sbt_address);
    }
    
    // Full initialization - writes all metadata immediately
    pub fn create_vault_full(env: Env, owner: Address, amount: i128, start_time: u64, end_time: u64) -> u64 {
        // Get next vault ID
        let mut vault_count: u64 = env.storage().instance().get(&VAULT_COUNT).unwrap_or(0);
        vault_count += 1;
        
        // Check admin balance and transfer tokens
        let mut admin_balance: i128 = env.storage().instance().get(&ADMIN_BALANCE).unwrap_or(0);
        require!(admin_balance >= amount, "Insufficient admin balance");
        admin_balance -= amount;
        env.storage().instance().set(&ADMIN_BALANCE, &admin_balance);
        
        // Create vault with full initialization
        let vault = Vault {
            owner: owner.clone(),
            total_amount: amount,
            released_amount: 0,
            start_time,
            end_time,
            is_initialized: true, // Mark as fully initialized
        };
        
        // Store vault data immediately (expensive gas usage)
        env.storage().instance().set(&VAULT_DATA, &vault_count, &vault);
        
        // Update user vaults list
        let mut user_vaults: Vec<u64> = env.storage().instance()
            .get(&USER_VAULTS, &owner)
            .unwrap_or(Vec::new(&env));
        user_vaults.push_back(vault_count);
        env.storage().instance().set(&USER_VAULTS, &owner, &user_vaults);
        
        // Update vault count
        env.storage().instance().set(&VAULT_COUNT, &vault_count);
        
        vault_count
    }
    
    // Lazy initialization - writes minimal data initially
    pub fn create_vault_lazy(env: Env, owner: Address, amount: i128, start_time: u64, end_time: u64) -> u64 {
        // Get next vault ID
        let mut vault_count: u64 = env.storage().instance().get(&VAULT_COUNT).unwrap_or(0);
        vault_count += 1;
        
        // Check admin balance and transfer tokens
        let mut admin_balance: i128 = env.storage().instance().get(&ADMIN_BALANCE).unwrap_or(0);
        require!(admin_balance >= amount, "Insufficient admin balance");
        admin_balance -= amount;
        env.storage().instance().set(&ADMIN_BALANCE, &admin_balance);
        
        // Create vault with lazy initialization (minimal storage)
        let vault = Vault {
            owner: owner.clone(),
            total_amount: amount,
            released_amount: 0,
            start_time,
            end_time,
            is_initialized: false, // Mark as lazy initialized
        };
        
        // Store only essential data initially (cheaper gas)
        env.storage().instance().set(&VAULT_DATA, &vault_count, &vault);
        
        // Update vault count
        env.storage().instance().set(&VAULT_COUNT, &vault_count);
        
        // Don't update user vaults list yet (lazy)
        
        vault_count
    }
    
    // Initialize vault metadata when needed (on-demand)
    pub fn initialize_vault_metadata(env: Env, vault_id: u64) -> bool {
        let vault: Vault = env.storage().instance()
            .get(&VAULT_DATA, &vault_id)
            .unwrap_or_else(|| {
                // Return empty vault if not found
                Vault {
                    owner: Address::from_contract_id(&env.current_contract_address()),
                    total_amount: 0,
                    released_amount: 0,
                    start_time: 0,
                    end_time: 0,
                    is_initialized: false,
                }
            });
        
        // Only initialize if not already initialized
        if !vault.is_initialized {
            let mut updated_vault = vault.clone();
            updated_vault.is_initialized = true;
            
            // Store updated vault with full metadata
            env.storage().instance().set(&VAULT_DATA, &vault_id, &updated_vault);
            
            // Update user vaults list (deferred)
            let mut user_vaults: Vec<u64> = env.storage().instance()
                .get(&USER_VAULTS, &updated_vault.owner)
                .unwrap_or(Vec::new(&env));
            user_vaults.push_back(vault_id);
            env.storage().instance().set(&USER_VAULTS, &updated_vault.owner, &user_vaults);
            
            true
        } else {
            false // Already initialized
        }
    }
    
    // Claim tokens from vault
    pub fn claim_tokens(env: Env, vault_id: u64, claim_amount: i128) -> i128 {
        Self::bump_if_needed(&env);
        
        let mut vault: Vault = env.storage().instance()
            .get(&VAULT_DATA, &vault_id)
            .unwrap_or_else(|| {
                panic!("Vault not found");
            });
        
        require!(vault.is_initialized, "Vault not initialized");
        require!(claim_amount > 0, "Claim amount must be positive");
        
        // Check SBT balance for DID gating
        let required_sbt: Address = env.storage().instance()
            .get(&REQUIRED_SBT_ADDRESS)
            .unwrap_or_else(|| {
                panic!("SBT address not configured");
            });
        
        // Check if beneficiary holds the required SBT
        let sbt_contract = token::Client::new(&env, &required_sbt);
        let sbt_balance = sbt_contract.balance(&vault.owner);
        require!(sbt_balance > 0, "Beneficiary must hold required SBT");
        
        let available_to_claim = vault.total_amount - vault.released_amount;
        require!(claim_amount <= available_to_claim, "Insufficient tokens to claim");
        
        // Update vault
        vault.released_amount += claim_amount;
        env.storage().instance().set(&VAULT_DATA, &vault_id, &vault);
        
        claim_amount
    }
    
    // Claim tokens and split to two addresses
    pub fn claim_and_split(env: Env, vault_id: u64, secondary_address: Address, split_percentage: u32, claim_amount: i128) -> (i128, i128) {
        Self::bump_if_needed(&env);
        
        let mut vault: Vault = env.storage().instance()
            .get(&VAULT_DATA, &vault_id)
            .unwrap_or_else(|| {
                panic!("Vault not found");
            });
        
        require!(vault.is_initialized, "Vault not initialized");
        require!(claim_amount > 0, "Claim amount must be positive");
        require!(split_percentage <= 100, "Split percentage must be 0-100");
        require!(secondary_address != vault.owner, "Secondary address must be different from primary");
        
        // Check SBT balance for DID gating
        let required_sbt: Address = env.storage().instance()
            .get(&REQUIRED_SBT_ADDRESS)
            .unwrap_or_else(|| {
                panic!("SBT address not configured");
            });
        
        // Check if beneficiary holds the required SBT
        let sbt_contract = token::Client::new(&env, &required_sbt);
        let sbt_balance = sbt_contract.balance(&vault.owner);
        require!(sbt_balance > 0, "Beneficiary must hold required SBT");
        
        let available_to_claim = vault.total_amount - vault.released_amount;
        require!(claim_amount <= available_to_claim, "Insufficient tokens to claim");
        
        // Calculate split amounts
        let secondary_amount = (claim_amount * split_percentage as i128) / 100;
        let primary_amount = claim_amount - secondary_amount;
        
        // Update vault
        vault.released_amount += claim_amount;
        env.storage().instance().set(&VAULT_DATA, &vault_id, &vault);
        
        // Return the split amounts (primary, secondary)
        (primary_amount, secondary_amount)
    }
    
    // Batch create vaults with lazy initialization
    pub fn batch_create_vaults_lazy(env: Env, batch_data: BatchCreateData) -> Vec<u64> {
        let mut vault_ids = Vec::new(&env);
        let initial_count: u64 = env.storage().instance().get(&VAULT_COUNT).unwrap_or(0);
        
        // Check total admin balance
        let total_amount: i128 = batch_data.amounts.iter().sum();
        let mut admin_balance: i128 = env.storage().instance().get(&ADMIN_BALANCE).unwrap_or(0);
        require!(admin_balance >= total_amount, "Insufficient admin balance for batch");
        admin_balance -= total_amount;
        env.storage().instance().set(&ADMIN_BALANCE, &admin_balance);
        
        for i in 0..batch_data.recipients.len() {
            let vault_id = initial_count + i as u64 + 1;
            
            // Create vault with lazy initialization
            let vault = Vault {
                owner: batch_data.recipients.get(i).unwrap(),
                total_amount: batch_data.amounts.get(i).unwrap(),
                released_amount: 0,
                start_time: batch_data.start_times.get(i).unwrap(),
                end_time: batch_data.end_times.get(i).unwrap(),
                is_initialized: false, // Lazy initialization
            };
            
            // Store vault data (minimal writes)
            env.storage().instance().set(&VAULT_DATA, &vault_id, &vault);
            vault_ids.push_back(vault_id);
        }
        
        // Update vault count once (cheaper than individual updates)
        let final_count = initial_count + batch_data.recipients.len() as u64;
        env.storage().instance().set(&VAULT_COUNT, &final_count);
        
        vault_ids
    }
    
    // Batch create vaults with full initialization
    pub fn batch_create_vaults_full(env: Env, batch_data: BatchCreateData) -> Vec<u64> {
        let mut vault_ids = Vec::new(&env);
        let initial_count: u64 = env.storage().instance().get(&VAULT_COUNT).unwrap_or(0);
        
        // Check total admin balance
        let total_amount: i128 = batch_data.amounts.iter().sum();
        let mut admin_balance: i128 = env.storage().instance().get(&ADMIN_BALANCE).unwrap_or(0);
        require!(admin_balance >= total_amount, "Insufficient admin balance for batch");
        admin_balance -= total_amount;
        env.storage().instance().set(&ADMIN_BALANCE, &admin_balance);
        
        for i in 0..batch_data.recipients.len() {
            let vault_id = initial_count + i as u64 + 1;
            
            // Create vault with full initialization
            let vault = Vault {
                owner: batch_data.recipients.get(i).unwrap(),
                total_amount: batch_data.amounts.get(i).unwrap(),
                released_amount: 0,
                start_time: batch_data.start_times.get(i).unwrap(),
                end_time: batch_data.end_times.get(i).unwrap(),
                is_initialized: true, // Full initialization
            };
            
            // Store vault data (expensive writes)
            env.storage().instance().set(&VAULT_DATA, &vault_id, &vault);
            
            // Update user vaults list for each vault (expensive)
            let mut user_vaults: Vec<u64> = env.storage().instance()
                .get(&USER_VAULTS, &vault.owner)
                .unwrap_or(Vec::new(&env));
            user_vaults.push_back(vault_id);
            env.storage().instance().set(&USER_VAULTS, &vault.owner, &user_vaults);
            
            vault_ids.push_back(vault_id);
        }
        
        // Update vault count once
        let final_count = initial_count + batch_data.recipients.len() as u64;
        env.storage().instance().set(&VAULT_COUNT, &final_count);
        
        vault_ids
    }
    
    // Get vault info (initializes if needed)
    pub fn get_vault(env: Env, vault_id: u64) -> Vault {
        Self::bump_if_needed(&env);
        
        let vault: Vault = env.storage().instance()
            .get(&VAULT_DATA, &vault_id)
            .unwrap_or_else(|| {
                Vault {
                    owner: Address::from_contract_id(&env.current_contract_address()),
                    total_amount: 0,
                    released_amount: 0,
                    start_time: 0,
                    end_time: 0,
                    is_initialized: false,
                }
            });
        
        // Auto-initialize if lazy
        if !vault.is_initialized {
            Self::initialize_vault_metadata(env, vault_id);
            // Get updated vault
            env.storage().instance().get(&VAULT_DATA, &vault_id).unwrap()
        } else {
            vault
        }
    }
    
    // Get user vaults (initializes all if needed)
    pub fn get_user_vaults(env: Env, user: Address) -> Vec<u64> {
        Self::bump_if_needed(&env);
        
        let vault_ids: Vec<u64> = env.storage().instance()
            .get(&USER_VAULTS, &user)
            .unwrap_or(Vec::new(&env));
        
        // Initialize all lazy vaults for this user
        for vault_id in vault_ids.iter() {
            let vault: Vault = env.storage().instance()
                .get(&VAULT_DATA, vault_id)
                .unwrap_or_else(|| {
                    Vault {
                        owner: user.clone(),
                        total_amount: 0,
                        released_amount: 0,
                        start_time: 0,
                        end_time: 0,
                        is_initialized: false,
                    }
                });
            
            if !vault.is_initialized {
                Self::initialize_vault_metadata(env, *vault_id);
            }
        }
        
        vault_ids
    }
    
    // Get contract state for invariant checking
    pub fn get_contract_state(env: Env) -> (i128, i128, i128) {
        Self::bump_if_needed(&env);
        
        let initial_supply: i128 = env.storage().instance().get(&INITIAL_SUPPLY).unwrap_or(0);
        let admin_balance: i128 = env.storage().instance().get(&ADMIN_BALANCE).unwrap_or(0);
        
        // Calculate total locked and claimed amounts
        let vault_count: u64 = env.storage().instance().get(&VAULT_COUNT).unwrap_or(0);
        let mut total_locked = 0i128;
        let mut total_claimed = 0i128;
        
        for i in 1..=vault_count {
            if let Some(vault) = env.storage().instance().get::<_, Vault>(&VAULT_DATA, &i) {
                total_locked += vault.total_amount - vault.released_amount;
                total_claimed += vault.released_amount;
            }
        }
        
        (total_locked, total_claimed, admin_balance)
    }
    
    // Check invariant: Total Locked + Total Claimed + Admin Balance = Initial Supply
    pub fn check_invariant(env: Env) -> bool {
        Self::bump_if_needed(&env);
        
        let initial_supply: i128 = env.storage().instance().get(&INITIAL_SUPPLY).unwrap_or(0);
        let (total_locked, total_claimed, admin_balance) = Self::get_contract_state(env);
        
        let sum = total_locked + total_claimed + admin_balance;
        sum == initial_supply
    }
    
    // Create milestone-gated vault
    pub fn create_milestone_vault(env: Env, owner: Address, amount: i128, milestones: Vec<u32>) -> u64 {
        Self::bump_if_needed(&env);
        
        // Validate milestones sum to 100
        let total_percentage: u32 = milestones.iter().sum();
        require!(total_percentage == 100, "Milestone percentages must sum to 100");
        require!(milestones.len() > 0, "At least one milestone required");
        
        // Get next vault ID
        let mut vault_count: u64 = env.storage().instance().get(&VAULT_COUNT).unwrap_or(0);
        vault_count += 1;
        
        // Check admin balance and transfer tokens
        let mut admin_balance: i128 = env.storage().instance().get(&ADMIN_BALANCE).unwrap_or(0);
        require!(admin_balance >= amount, "Insufficient admin balance");
        admin_balance -= amount;
        env.storage().instance().set(&ADMIN_BALANCE, &admin_balance);
        
        // Create regular vault
        let vault = Vault {
            owner: owner.clone(),
            total_amount: amount,
            released_amount: 0,
            start_time: env.ledger().timestamp(),
            end_time: u64::MAX, // No time limit for milestone vaults
            is_initialized: true,
        };
        
        // Store vault data
        env.storage().instance().set(&VAULT_DATA, &vault_count, &vault);
        
        // Create milestone vault
        let milestone_vault = MilestoneVault {
            vault_id: vault_count,
            milestones: milestones.clone(),
            current_milestone: 0,
            milestone_events: Map::new(&env),
        };
        
        // Store milestone vault data
        env.storage().instance().set(&MILESTONE_VAULTS, &vault_count, &milestone_vault);
        
        // Update user vaults list
        let mut user_vaults: Vec<u64> = env.storage().instance()
            .get(&USER_VAULTS, &owner)
            .unwrap_or(Vec::new(&env));
        user_vaults.push_back(vault_count);
        env.storage().instance().set(&USER_VAULTS, &owner, &user_vaults);
        
        // Update vault count
        env.storage().instance().set(&VAULT_COUNT, &vault_count);
        
        vault_count
    }
    
    // Trigger milestone (admin only)
    pub fn trigger_milestone(env: Env, vault_id: u64, milestone_id: u32, admin: Address) {
        Self::bump_if_needed(&env);
        
        admin.require_auth();
        
        let mut milestone_vault: MilestoneVault = env.storage().instance()
            .get(&MILESTONE_VAULTS, &vault_id)
            .unwrap_or_else(|| {
                panic!("Milestone vault not found");
            });
        
        require!(milestone_id < milestone_vault.milestones.len() as u32, "Invalid milestone ID");
        
        // Check if previous milestones are triggered
        if milestone_id > 0 {
            for i in 0..milestone_id {
                let prev_event = milestone_vault.milestone_events.get(i);
                require!(prev_event.is_some() && prev_event.unwrap().is_triggered, 
                    "Previous milestones must be triggered first");
            }
        }
        
        // Create milestone event
        let milestone_event = MilestoneEvent {
            milestone_id,
            is_triggered: true,
            trigger_time: env.ledger().timestamp(),
            triggered_by: admin,
        };
        
        // Store milestone event
        milestone_vault.milestone_events.set(milestone_id, &milestone_event);
        milestone_vault.current_milestone = milestone_id + 1;
        
        // Update milestone vault
        env.storage().instance().set(&MILESTONE_VAULTS, &vault_id, &milestone_vault);
    }
    
    // Claim tokens from milestone vault
    pub fn claim_milestone_tokens(env: Env, vault_id: u64) -> i128 {
        Self::bump_if_needed(&env);
        
        let vault: Vault = env.storage().instance()
            .get(&VAULT_DATA, &vault_id)
            .unwrap_or_else(|| {
                panic!("Vault not found");
            });
        
        let milestone_vault: MilestoneVault = env.storage().instance()
            .get(&MILESTONE_VAULTS, &vault_id)
            .unwrap_or_else(|| {
                panic!("Milestone vault not found");
            });
        
        // Check SBT balance for DID gating
        let required_sbt: Address = env.storage().instance()
            .get(&REQUIRED_SBT_ADDRESS)
            .unwrap_or_else(|| {
                panic!("SBT address not configured");
            });
        
        let sbt_contract = token::Client::new(&env, &required_sbt);
        let sbt_balance = sbt_contract.balance(&vault.owner);
        require!(sbt_balance > 0, "Beneficiary must hold required SBT");
        
        // Calculate claimable amount based on triggered milestones
        let mut claimable_percentage = 0u32;
        for i in 0..milestone_vault.current_milestone {
            if let Some(milestone_event) = milestone_vault.milestone_events.get(i) {
                if milestone_event.is_triggered {
                    claimable_percentage += milestone_vault.milestones.get(i).unwrap();
                }
            }
        }
        
        let total_claimable = (vault.total_amount * claimable_percentage as i128) / 100;
        let available_to_claim = total_claimable - vault.released_amount;
        
        require!(available_to_claim > 0, "No tokens available to claim");
        
        // Update vault
        let mut updated_vault = vault;
        updated_vault.released_amount += available_to_claim;
        env.storage().instance().set(&VAULT_DATA, &vault_id, &updated_vault);
        
        available_to_claim
    }
    
    // Simulate claim for dry-run (returns tokens_to_release, estimated_gas_fee, tax_withholding)
    pub fn simulate_claim(env: Env, vault_id: u64, claim_amount: Option<i128>) -> (i128, i128, i128) {
        Self::bump_if_needed(&env);
        
        let vault: Vault = env.storage().instance()
            .get(&VAULT_DATA, &vault_id)
            .unwrap_or_else(|| {
                panic!("Vault not found");
            });
        
        require!(vault.is_initialized, "Vault not initialized");
        
        let available_to_claim = vault.total_amount - vault.released_amount;
        let actual_claim_amount = if let Some(amount) = claim_amount {
            require!(amount > 0, "Claim amount must be positive");
            require!(amount <= available_to_claim, "Insufficient tokens to claim");
            amount
        } else {
            available_to_claim
        };
        
        // Estimate gas fee (simplified estimation)
        let estimated_gas_fee = 1000000i128; // Base fee in stroops
        
        // Calculate tax withholding (15% for international grants - simplified)
        let tax_withholding = (actual_claim_amount * 15) / 100;
        let tokens_to_release = actual_claim_amount - tax_withholding;
        
        (tokens_to_release, estimated_gas_fee, tax_withholding)
    }
    
    // Get milestone vault info
    pub fn get_milestone_vault(env: Env, vault_id: u64) -> MilestoneVault {
        Self::bump_if_needed(&env);
        
        env.storage().instance()
            .get(&MILESTONE_VAULTS, &vault_id)
            .unwrap_or_else(|| {
                panic!("Milestone vault not found");
            })
    }
}
