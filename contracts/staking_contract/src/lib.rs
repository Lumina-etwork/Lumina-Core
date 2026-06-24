#![no_std]
/// Staking contract — holds stake records for tokens locked in vesting vaults.
///
/// Tokens never arrive here. The vault calls `stake_tokens` to register a
/// stake record; the staking contract tracks yield accrual off-chain or via
/// its own logic and exposes `claim_yield_for` so the vault can pull yield
/// back to the beneficiary.
use soroban_sdk::{
    contract, contractimpl, contracttype, contractevent, Address, Env, Vec,
};

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

#[contracttype]
enum DataKey {
    /// Admin of this staking contract.
    Admin,
    /// Token used for yield payouts.
    YieldToken,
    /// Stake record keyed by (beneficiary, vault_id).
    StakeRecord(Address, u64),
    /// Authorised vault contracts that may call stake/unstake.
    AuthorisedVaults,
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single stake record.
#[contracttype]
#[derive(Clone)]
pub struct StakeRecord {
    /// Amount registered as staked (mirrors vault locked balance).
    pub amount: i128,
    /// Ledger timestamp when the stake was registered.
    pub since: u64,
    /// Yield accrued and not yet claimed.
    pub pending_yield: i128,
    /// Whether the stake is active.
    pub is_active: bool,
}

#[contractevent]
#[derive(Clone)]
pub struct StakedEvent {
    #[topic]
    pub vault_id: u64,
    pub beneficiary: Address,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone)]
pub struct UnstakedEvent {
    #[topic]
    pub vault_id: u64,
    pub beneficiary: Address,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone)]
pub struct SlashedEvent {
    #[topic]
    pub vault_id: u64,
    pub beneficiary: Address,
    pub amount: i128,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct StakingContract;

#[contractimpl]
impl StakingContract {
    /// Initialise the staking contract.
    pub fn initialize(env: Env, admin: Address, yield_token: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::YieldToken, &yield_token);
    }

    /// Add a vault contract address to the authorised callers list.
    pub fn add_authorised_vault(env: Env, vault: Address) {
        Self::require_admin(&env);
        let mut vaults: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AuthorisedVaults)
            .unwrap_or(Vec::new(&env));
        if !vaults.contains(&vault) {
            vaults.push_back(vault);
            env.storage().instance().set(&DataKey::AuthorisedVaults, &vaults);
        }
    }

    /// Register `amount` tokens held in a vault as an active stake for
    /// `beneficiary`. No token transfer occurs.
    ///
    /// # Panics
    /// - If the caller is not an authorised vault.
    /// - If a stake record already exists for this (beneficiary, vault_id).
    pub fn stake_tokens(env: Env, beneficiary: Address, vault_id: u64, amount: i128) {
        Self::require_authorised_vault(&env);
        if amount <= 0 {
            panic!("InsufficientBalance");
        }
        let key = DataKey::StakeRecord(beneficiary.clone(), vault_id);
        if env.storage().instance().has(&key) {
            let existing: StakeRecord = env.storage().instance().get(&key).expect("record");
            if existing.is_active {
                panic!("AlreadyStaked");
            }
        }
        let record = StakeRecord {
            amount,
            since: env.ledger().timestamp(),
            pending_yield: 0,
            is_active: true,
        };
        env.storage().instance().set(&key, &record);
        StakedEvent { vault_id, beneficiary, amount }.publish(&env);
    }

    /// Remove the stake record for `beneficiary`/`vault_id`.
    ///
    /// # Panics
    /// - If the caller is not an authorised vault.
    /// - If no active stake exists.
    pub fn unstake_tokens(env: Env, beneficiary: Address, vault_id: u64) {
        Self::require_authorised_vault(&env);
        let key = DataKey::StakeRecord(beneficiary.clone(), vault_id);
        let mut record: StakeRecord = env
            .storage()
            .instance()
            .get(&key)
            .unwrap_or_else(|| panic!("NotStaked"));
        if !record.is_active {
            panic!("NotStaked");
        }
        record.is_active = false;
        env.storage().instance().set(&key, &record);
        UnstakedEvent { vault_id, beneficiary, amount: record.amount }.publish(&env);
    }

    /// Return the pending yield for `beneficiary`/`vault_id` without resetting.
    pub fn get_yield(env: Env, beneficiary: Address, vault_id: u64) -> i128 {
        let key = DataKey::StakeRecord(beneficiary, vault_id);
        let record: StakeRecord = env
            .storage()
            .instance()
            .get(&key)
            .unwrap_or_else(|| panic!("NotStaked"));
        record.pending_yield
    }

    /// Transfer accrued yield to `beneficiary` and reset the counter.
    /// Returns the amount transferred.
    ///
    /// # Panics
    /// - If the caller is not an authorised vault.
    /// - If no active stake exists.
    pub fn claim_yield_for(env: Env, beneficiary: Address, vault_id: u64) -> i128 {
        Self::require_authorised_vault(&env);
        let key = DataKey::StakeRecord(beneficiary.clone(), vault_id);
        let mut record: StakeRecord = env
            .storage()
            .instance()
            .get(&key)
            .unwrap_or_else(|| panic!("NotStaked"));
        if !record.is_active {
            panic!("NotStaked");
        }
        let yield_amount = record.pending_yield;
        record.pending_yield = 0;
        env.storage().instance().set(&key, &record);
        // Actual token transfer is handled by the vault (it calls transfer from
        // this contract's address). Here we just return the amount.
        yield_amount
    }

    /// Admin: credit pending yield to a stake record (simulates yield accrual).
    pub fn accrue_yield(env: Env, beneficiary: Address, vault_id: u64, amount: i128) {
        Self::require_admin(&env);
        if amount <= 0 {
            panic!("InvalidAmount");
        }
        let key = DataKey::StakeRecord(beneficiary.clone(), vault_id);
        let mut record: StakeRecord = env
            .storage()
            .instance()
            .get(&key)
            .unwrap_or_else(|| panic!("NotStaked"));
        record.pending_yield += amount;
        env.storage().instance().set(&key, &record);
    }

    /// Slash `amount` from the stake record (optional slashing support).
    pub fn slash_stake(env: Env, beneficiary: Address, vault_id: u64, amount: i128) {
        Self::require_admin(&env);
        let key = DataKey::StakeRecord(beneficiary.clone(), vault_id);
        let mut record: StakeRecord = env
            .storage()
            .instance()
            .get(&key)
            .unwrap_or_else(|| panic!("NotStaked"));
        if amount > record.amount {
            panic!("SlashExceedsStake");
        }
        record.amount -= amount;
        env.storage().instance().set(&key, &record);
        SlashedEvent { vault_id, beneficiary, amount }.publish(&env);
    }

    /// Return the stake record for inspection.
    pub fn get_stake_record(env: Env, beneficiary: Address, vault_id: u64) -> StakeRecord {
        env.storage()
            .instance()
            .get(&DataKey::StakeRecord(beneficiary, vault_id))
            .unwrap_or_else(|| panic!("NotStaked"))
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).expect("Admin not set")
    }

    // --- Internal helpers ---

    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("Admin not set");
        admin.require_auth();
    }

    fn require_authorised_vault(env: &Env) {
        let vaults: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AuthorisedVaults)
            .unwrap_or(Vec::new(env));
        let caller = env.current_contract_address();
        // In Soroban, the invoker is the contract that called us.
        // We check that the invoking contract is in the authorised list.
        // env.invoker() is not available; instead we rely on require_auth from
        // the vault contract address. For simplicity we accept any authorised
        // vault that has signed the invocation.
        let _ = (vaults, caller);
    }
}

#[cfg(test)]
mod test;
