#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Map, Vec,
    U256,
};

#[contract]
pub struct VirtualAccumulatorContract;

// Virtual accumulator precision settings
pub const PRECISION_MULTIPLIER: u128 = 1_000_000_000_000_000_000; // 18 decimals
pub const MAX_ACCUMULATION_PERIODS: u64 = 1000;

#[derive(Clone)]
#[contracttype]
pub enum AccumulatorDataKey {
    Config,
    State,
    UserState(Address),
}

#[derive(Clone)]
#[contracttype]
pub struct AccumulatorConfig {
    pub precision: u128,
    pub max_periods: u64,
    pub base_rate: u128,
    pub update_threshold: u64, // Minimum time between updates
}

#[derive(Clone)]
#[contracttype]
pub struct AccumulatorState {
    pub total_accumulated: U256,
    pub last_update_timestamp: u64,
    pub current_rate: u128,
    pub period_count: u64,
    pub precision_adjusted: bool,
}

#[derive(Clone)]
#[contracttype]
pub struct UserAccumulatorState {
    pub user_address: Address,
    pub last_claimed_amount: U256,
    pub last_claim_timestamp: u64,
    pub accumulated_balance: U256,
    pub rate_multiplier: u128,
    pub is_active: bool,
}

#[contracterror]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum AccumulatorError {
    NotInitialized = 2001,
    AlreadyInitialized = 2002,
    InvalidConfig = 2003,
    Overflow = 2004,
    Underflow = 2005,
    PeriodExceeded = 2006,
    UserNotFound = 2007,
    InsufficientBalance = 2008,
    Unauthorized = 2009,
    UpdateTooFrequent = 2010,
}

// Virtual accumulator implementation
pub struct VirtualAccumulator;

impl VirtualAccumulator {
    // Initialize accumulator with precision settings
    pub fn initialize(env: &Env, config: AccumulatorConfig) -> Result<(), AccumulatorError> {
        if env.storage()
            .instance()
            .get::<AccumulatorDataKey, AccumulatorConfig>(&AccumulatorDataKey::Config)
            .is_some()
        {
            return Err(AccumulatorError::AlreadyInitialized);
        }

        // Validate config
        if config.precision == 0 || config.max_periods == 0 || config.base_rate == 0 {
            return Err(AccumulatorError::InvalidConfig);
        }

        env.storage()
            .instance()
            .set(&AccumulatorDataKey::Config, &config);

        let state = AccumulatorState {
            total_accumulated: U256::from_u32(&env, 0),
            last_update_timestamp: env.ledger().timestamp(),
            current_rate: config.base_rate,
            period_count: 0,
            precision_adjusted: false,
        };

        env.storage()
            .instance()
            .set(&AccumulatorDataKey::State, &state);

        Ok(())
    }

    // Read accumulator config
    pub fn read_config(env: &Env) -> Result<AccumulatorConfig, AccumulatorError> {
        env.storage()
            .instance()
            .get(&AccumulatorDataKey::Config)
            .ok_or(AccumulatorError::NotInitialized)
    }

    // Read accumulator state
    pub fn read_state(env: &Env) -> Result<AccumulatorState, AccumulatorError> {
        env.storage()
            .instance()
            .get(&AccumulatorDataKey::State)
            .ok_or(AccumulatorError::NotInitialized)
    }

    // Write accumulator state
    pub fn write_state(env: &Env, state: &AccumulatorState) {
        env.storage()
            .instance()
            .set(&AccumulatorDataKey::State, state);
    }

    // Calculate accumulated amount with high precision
    pub fn calculate_accumulated_amount(
        env: &Env,
        user_state: &UserAccumulatorState,
        current_timestamp: u64,
    ) -> Result<U256, AccumulatorError> {
        let config = Self::read_config(env)?;
        let _accumulator_state = Self::read_state(env)?;

        let time_elapsed = current_timestamp.saturating_sub(user_state.last_claim_timestamp);
        
        if time_elapsed == 0 {
            return Ok(U256::from_u32(&env, 0));
        }

        // High-precision calculation: rate * time_elapsed * precision_multiplier
        let rate_precise = U256::from_u128(&env, user_state.rate_multiplier * config.precision);
        let time_precise = U256::from_u128(&env, time_elapsed as u128);
        
        let accumulated = rate_precise
            .mul(&time_precise)
            .div(&U256::from_u128(&env, config.precision));

        Ok(accumulated)
    }

    // Update user accumulator state
    pub fn update_user_state(
        env: &Env,
        user: &Address,
        rate_multiplier: u128,
    ) -> Result<(), AccumulatorError> {
        let current_timestamp = env.ledger().timestamp();
        
        let mut user_state = Self::read_user_state(env, user)?;
        
        // Check update threshold
        let config = Self::read_config(env)?;
        if current_timestamp.saturating_sub(user_state.last_claim_timestamp) < config.update_threshold {
            return Err(AccumulatorError::UpdateTooFrequent);
        }

        // Calculate new accumulated amount
        let new_accumulated = Self::calculate_accumulated_amount(env, &user_state, current_timestamp)?;
        
        // Update user state
        user_state.accumulated_balance = user_state.accumulated_balance
            .add(&new_accumulated);
        user_state.last_claim_timestamp = current_timestamp;
        user_state.rate_multiplier = rate_multiplier;

        Self::write_user_state(env, user, &user_state);

        Ok(())
    }

    // Read user state
    pub fn read_user_state(env: &Env, user: &Address) -> Result<UserAccumulatorState, AccumulatorError> {
        env.storage()
            .instance()
            .get(&AccumulatorDataKey::UserState(user.clone()))
            .ok_or(AccumulatorError::UserNotFound)
    }

    // Write user state
    pub fn write_user_state(env: &Env, user: &Address, user_state: &UserAccumulatorState) {
        env.storage()
            .instance()
            .set(&AccumulatorDataKey::UserState(user.clone()), user_state);
    }

    // Create new user accumulator
    pub fn create_user_accumulator(
        env: &Env,
        user: &Address,
        rate_multiplier: u128,
    ) -> Result<(), AccumulatorError> {
        if Self::read_user_state(env, user).is_ok() {
            return Err(AccumulatorError::UserNotFound); // User already exists
        }

        let user_state = UserAccumulatorState {
            user_address: user.clone(),
            last_claimed_amount: U256::from_u32(&env, 0),
            last_claim_timestamp: env.ledger().timestamp(),
            accumulated_balance: U256::from_u32(&env, 0),
            rate_multiplier,
            is_active: true,
        };

        Self::write_user_state(env, user, &user_state);
        Ok(())
    }

    // Claim accumulated amount
    pub fn claim_accumulated(
        env: &Env,
        user: &Address,
        claim_amount: u128,
    ) -> Result<U256, AccumulatorError> {
        let mut user_state = Self::read_user_state(env, user)?;
        
        // Update accumulated balance first
        Self::update_user_state(env, user, user_state.rate_multiplier)?;
        user_state = Self::read_user_state(env, user)?;

        let claim_amount_precise = U256::from_u128(&env, claim_amount);
        
        if user_state.accumulated_balance < claim_amount_precise {
            return Err(AccumulatorError::InsufficientBalance);
        }

        // Subtract claimed amount
        user_state.accumulated_balance = user_state.accumulated_balance
            .sub(&claim_amount_precise);
        
        user_state.last_claimed_amount = claim_amount_precise.clone();

        Self::write_user_state(env, user, &user_state);

        Ok(claim_amount_precise)
    }

    // Get user's current accumulated balance
    pub fn get_user_balance(env: &Env, user: &Address) -> Result<U256, AccumulatorError> {
        let mut user_state = Self::read_user_state(env, user)?;
        
        // Update accumulated balance to current time
        Self::update_user_state(env, user, user_state.rate_multiplier)?;
        
        user_state = Self::read_user_state(env, user)?;
        Ok(user_state.accumulated_balance)
    }
}

#[contractimpl]
impl VirtualAccumulatorContract {
    // Initialize the virtual accumulator
    pub fn initialize(env: Env, admin: Address) -> Result<(), AccumulatorError> {
        admin.require_auth();
        
        let config = AccumulatorConfig {
            precision: PRECISION_MULTIPLIER,
            max_periods: MAX_ACCUMULATION_PERIODS,
            base_rate: 1, // Base rate of 1 token per second
            update_threshold: 1, // Minimum 1 second between updates
        };

        VirtualAccumulator::initialize(&env, config)
    }

    // Create user accumulator
    pub fn create_user_accumulator(
        env: Env,
        admin: Address,
        user: Address,
        rate_multiplier: u128,
    ) -> Result<(), AccumulatorError> {
        admin.require_auth();
        VirtualAccumulator::create_user_accumulator(&env, &user, rate_multiplier)
    }

    // Update user rate
    pub fn update_user_rate(
        env: Env,
        admin: Address,
        user: Address,
        new_rate_multiplier: u128,
    ) -> Result<(), AccumulatorError> {
        admin.require_auth();
        VirtualAccumulator::update_user_state(&env, &user, new_rate_multiplier)
    }

    // Claim accumulated tokens
    pub fn claim(
        env: Env,
        user: Address,
        amount: u128,
    ) -> Result<U256, AccumulatorError> {
        user.require_auth();
        VirtualAccumulator::claim_accumulated(&env, &user, amount)
    }

    // Get user balance
    pub fn get_balance(env: Env, user: Address) -> Result<U256, AccumulatorError> {
        VirtualAccumulator::get_user_balance(&env, &user)
    }

    // Get accumulator config
    pub fn get_config(env: Env) -> Result<AccumulatorConfig, AccumulatorError> {
        VirtualAccumulator::read_config(&env)
    }

    // Get accumulator state
    pub fn get_state(env: Env) -> Result<AccumulatorState, AccumulatorError> {
        VirtualAccumulator::read_state(&env)
    }

    // Get user state
    pub fn get_user_state(env: Env, user: Address) -> Result<UserAccumulatorState, AccumulatorError> {
        VirtualAccumulator::read_user_state(&env, &user)
    }
}
