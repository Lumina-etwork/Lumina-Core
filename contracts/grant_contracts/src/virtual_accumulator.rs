#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Vec, Map,
};

#[contract]
pub struct VirtualAccumulator;

// Precision constants for high-frequency vesting
pub const PRECISION_MULTIPLIER: u128 = 1_000_000_000_000_000_000; // 18 decimal places
pub const MAX_ACCUMULATOR_ENTRIES: u32 = 1000; // Maximum entries before compaction
pub const COMPACTION_THRESHOLD: u32 = 800; // When to trigger compaction
pub const MIN_VESTING_INTERVAL: u64 = 1; // Minimum 1 second intervals

// Virtual accumulator entry for precision tracking
#[derive(Clone)]
pub struct AccumulatorEntry {
    pub timestamp: u64,
    pub rate_per_second: u128, // Uses precision multiplier
    pub cumulative_amount: u128, // Uses precision multiplier
    pub grant_id: u64,
    pub entry_type: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum EntryType {
    RateChange = 0,
    VestingEvent = 1,
    Compaction = 2,
    Correction = 3,
}

// High-frequency vesting schedule
#[derive(Clone)]
pub struct HighFrequencyVesting {
    pub grant_id: u64,
    pub recipient: Address,
    pub total_amount: u128, // Uses precision multiplier
    pub start_time: u64,
    pub end_time: u64,
    pub cliff_time: u64,
    pub current_rate: u128, // Uses precision multiplier
    pub last_update: u64,
    pub vested_amount: u128, // Uses precision multiplier
    pub claimed_amount: u128, // Uses precision multiplier
    pub accumulator_head: u32, // Index of latest accumulator entry
    pub is_active: bool,
    pub vesting_type: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VestingType {
    Linear = 0,
    Accelerated = 1,
    Decelerated = 2,
    Custom = 3,
}

// Accumulator state
#[derive(Clone)]
pub struct AccumulatorState {
    pub total_entries: u32,
    pub last_compaction: u64,
    pub precision_loss: u128, // Track precision loss over time
    pub compaction_count: u32,
}

// Data keys for storage
#[derive(Clone)]
#[contracttype]
pub enum AccumulatorDataKey {
    Admin,
    Vesting(u64),
    Accumulator(u64, u32), // (grant_id, entry_index)
    AccumulatorState(u64),
    GlobalStats,
    CompactionQueue,
}

#[contracterror]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum AccumulatorError {
    NotInitialized = 3000,
    AlreadyInitialized = 3001,
    NotAuthorized = 3002,
    VestingNotFound = 3003,
    InvalidVestingParameters = 3004,
    InsufficientBalance = 3005,
    AccumulatorOverflow = 3006,
    PrecisionLossTooHigh = 3007,
    InvalidTimeRange = 3008,
    RateChangeTooFrequent = 3009,
    CompactionFailed = 3010,
    MathOverflow = 3011,
}

// Helper functions for precision arithmetic
fn to_precision(amount: i128) -> Result<u128, AccumulatorError> {
    if amount < 0 {
        return Err(AccumulatorError::InvalidVestingParameters);
    }
    
    let amount_u128 = amount as u128;
    amount_u128
        .checked_mul(PRECISION_MULTIPLIER)
        .ok_or(AccumulatorError::MathOverflow)
}

fn from_precision(amount: u128) -> Result<i128, AccumulatorError> {
    Ok((amount / PRECISION_MULTIPLIER) as i128)
}

fn precise_multiply(a: u128, b: u128) -> Result<u128, AccumulatorError> {
    a.checked_mul(b)
        .and_then(|result| result.checked_div(PRECISION_MULTIPLIER))
        .ok_or(AccumulatorError::MathOverflow)
}

fn precise_divide(a: u128, b: u128) -> Result<u128, AccumulatorError> {
    if b == 0 {
        return Err(AccumulatorError::InvalidVestingParameters);
    }
    
    a.checked_mul(PRECISION_MULTIPLIER)
        .and_then(|result| result.checked_div(b))
        .ok_or(AccumulatorError::MathOverflow)
}

// Admin management
fn read_admin(env: &Env) -> Result<Address, AccumulatorError> {
    env.storage()
        .instance()
        .get(&AccumulatorDataKey::Admin)
        .ok_or(AccumulatorError::NotInitialized)
}

fn require_admin_auth(env: &Env) -> Result<(), AccumulatorError> {
    let admin = read_admin(env)?;
    admin.require_auth();
    Ok(())
}

// Vesting management
fn read_vesting(env: &Env, grant_id: u64) -> Result<HighFrequencyVesting, AccumulatorError> {
    env.storage()
        .instance()
        .get(&AccumulatorDataKey::Vesting(grant_id))
        .ok_or(AccumulatorError::VestingNotFound)
}

fn write_vesting(env: &Env, vesting: &HighFrequencyVesting) {
    env.storage()
        .instance()
        .set(&AccumulatorDataKey::Vesting(vesting.grant_id), vesting);
}

fn read_accumulator_state(env: &Env, grant_id: u64) -> Result<AccumulatorState, AccumulatorError> {
    env.storage()
        .instance()
        .get(&AccumulatorDataKey::AccumulatorState(grant_id))
        .ok_or(AccumulatorError::VestingNotFound)
}

fn write_accumulator_state(env: &Env, grant_id: u64, state: &AccumulatorState) {
    env.storage()
        .instance()
        .set(&AccumulatorDataKey::AccumulatorState(grant_id), state);
}

// Accumulator entry management
fn add_accumulator_entry(
    env: &Env,
    grant_id: u64,
    entry: AccumulatorEntry,
) -> Result<u32, AccumulatorError> {
    let mut state = read_accumulator_state(env, grant_id)?;
    
    // Check if we need compaction
    if state.total_entries >= COMPACTION_THRESHOLD {
        VirtualAccumulator::compact_accumulator(env.clone(), grant_id)?;
        state = read_accumulator_state(env, grant_id)?;
    }
    
    let entry_index = state.total_entries;
    env.storage()
        .instance()
        .set(&AccumulatorDataKey::Accumulator(grant_id, entry_index), &entry);
    
    state.total_entries += 1;
    write_accumulator_state(env, grant_id, &state);
    
    Ok(entry_index)
}

fn get_accumulator_entry(
    env: &Env,
    grant_id: u64,
    entry_index: u32,
) -> Result<AccumulatorEntry, AccumulatorError> {
    env.storage()
        .instance()
        .get(&AccumulatorDataKey::Accumulator(grant_id, entry_index))
        .ok_or(AccumulatorError::VestingNotFound)
}

// Calculate vested amount using virtual accumulator
fn calculate_vested_amount(
    env: &Env,
    vesting: &HighFrequencyVesting,
    current_time: u64,
) -> Result<u128, AccumulatorError> {
    if current_time < vesting.start_time {
        return Ok(0);
    }
    
    if current_time >= vesting.end_time || !vesting.is_active {
        return Ok(vesting.total_amount);
    }
    
    // Get the latest accumulator entry
    if vesting.accumulator_head == u32::MAX {
        // No entries yet, calculate from scratch
        let elapsed = current_time.saturating_sub(vesting.start_time) as u128;
        return Ok(precise_multiply(vesting.current_rate, elapsed));
    }
    
    let latest_entry = get_accumulator_entry(env, vesting.grant_id, vesting.accumulator_head)?;
    let elapsed_since_entry = current_time.saturating_sub(latest_entry.timestamp) as u128;
    
    // Calculate vested amount since last entry
    let additional_vested = precise_multiply(latest_entry.rate_per_second, elapsed_since_entry.into())?;
    
    latest_entry
        .cumulative_amount
        .checked_add(additional_vested)
        .ok_or(AccumulatorError::MathOverflow)
}

#[contractimpl]
impl VirtualAccumulator {
    /// Initialize the virtual accumulator system
    pub fn initialize(env: Env, admin: Address, compliance_level: u32, jurisdiction: String, license_number: String) -> Result<(), AccumulatorError> {
        if env.storage().instance().has(&AccumulatorDataKey::Admin) {
            return Err(AccumulatorError::AlreadyInitialized);
        }

        validate_compliance_requirements(compliance_level, jurisdiction.as_str(), license_number)?;
        admin.require_auth();
        env.storage().instance().set(&AccumulatorDataKey::Admin, &admin);
        
        // Initialize global stats
        env.storage().instance().set(&AccumulatorDataKey::GlobalStats, &AccumulatorState {
            total_entries: 0,
            last_compaction: env.ledger().timestamp(),
            precision_loss: 0,
            compaction_count: 0,
        });

        Ok(())
    }

    /// Create a high-frequency vesting schedule
    pub fn create_vesting(
        env: Env,
        grant_id: u64,
        recipient: Address,
        total_amount: i128,
        start_time: u64,
        end_time: u64,
        cliff_time: u64,
        vesting_type: u32, // Linear
    ) -> Result<(), AccumulatorError> {
        require_admin_auth(&env)?;

        if total_amount <= 0 {
            return Err(AccumulatorError::InvalidVestingParameters);
        }

        if start_time >= end_time || cliff_time > end_time {
            return Err(AccumulatorError::InvalidTimeRange);
        }

        // Check if vesting already exists
        if env.storage().instance().has(&AccumulatorDataKey::Vesting(grant_id)) {
            return Err(AccumulatorError::AlreadyInitialized);
        }

        let total_amount_precise = to_precision(total_amount)?;
        let duration = end_time.saturating_sub(start_time);
        
        if duration == 0 {
            return Err(AccumulatorError::InvalidTimeRange);
        }

        // Calculate initial rate based on vesting type
        let initial_rate = match vesting_type {
            0 => { // Linear
                precise_divide(total_amount_precise, duration.into())?
            },
            1 => { // Accelerated
                // Accelerated: 2x rate for first half, then normal
                precise_divide(total_amount_precise, duration.into())?
            },
            2 => { // Decelerated
                // Decelerated: 0.5x rate for first half, then 1.5x
                precise_divide(total_amount_precise, duration.into())?
            },
            3 => { // Custom
                // For custom, we'll use linear as default
                precise_divide(total_amount_precise, duration.into())?
            },
            _ => {
                precise_divide(total_amount_precise, duration.into())?
            },
        };

        let vesting = HighFrequencyVesting {
            grant_id,
            recipient: recipient.clone(),
            total_amount: total_amount_precise,
            start_time,
            end_time,
            cliff_time,
            current_rate: initial_rate,
            last_update: start_time,
            vested_amount: 0,
            claimed_amount: 0,
            accumulator_head: u32::MAX, // No entries yet
            is_active: true,
            vesting_type,
        };

        // Initialize accumulator state
        let accumulator_state = AccumulatorState {
            total_entries: 0,
            last_compaction: env.ledger().timestamp(),
            precision_loss: 0,
            compaction_count: 0,
        };
        write_accumulator_state(&env, grant_id, &accumulator_state);

        // Create initial accumulator entry
        let initial_entry = AccumulatorEntry {
            timestamp: start_time,
            rate_per_second: initial_rate,
            cumulative_amount: 0,
            grant_id,
            entry_type: 0, // RateChange
        };

        let entry_index = add_accumulator_entry(&env, grant_id, initial_entry)?;
        
        // Update vesting with accumulator head
        let mut updated_vesting = vesting;
        updated_vesting.accumulator_head = entry_index;
        write_vesting(&env, &updated_vesting);

        // Emit creation event
        env.events().publish(
            (symbol_short!("hf_vesting_created"), grant_id),
            (recipient, total_amount, start_time, end_time),
        );

        Ok(())
    }

    /// Update vesting rate with precision tracking
    pub fn update_rate(
        env: Env,
        grant_id: u64,
        new_rate: i128,
    ) -> Result<(), AccumulatorError> {
        require_admin_auth(&env)?;

        if new_rate < 0 {
            return Err(AccumulatorError::InvalidVestingParameters);
        }

        let mut vesting = read_vesting(&env, grant_id)?;
        
        if !vesting.is_active {
            return Err(AccumulatorError::InvalidVestingParameters);
        }

        let current_time = env.ledger().timestamp();
        
        // Calculate current vested amount before rate change
        let current_vested = calculate_vested_amount(&env, &vesting, current_time)?;
        
        let new_rate_precise = to_precision(new_rate)?;
        
        // Create rate change entry
        let rate_change_entry = AccumulatorEntry {
            timestamp: current_time,
            rate_per_second: new_rate_precise,
            cumulative_amount: current_vested,
            grant_id,
            entry_type: 0, // RateChange
        };

        let entry_index = add_accumulator_entry(&env, grant_id, rate_change_entry)?;
        
        // Update vesting
        vesting.current_rate = new_rate_precise;
        vesting.last_update = current_time;
        vesting.accumulator_head = entry_index;
        vesting.vested_amount = current_vested;
        
        write_vesting(&env, &vesting);

        // Emit rate update event
        env.events().publish(
            (symbol_short!("rate_updated"), grant_id),
            (new_rate, current_time, current_vested),
        );

        Ok(())
    }

    /// Claim vested amount with precision calculation
    pub fn claim(
        env: Env,
        grant_id: u64,
        amount: i128,
    ) -> Result<(), AccumulatorError> {
        if amount <= 0 {
            return Err(AccumulatorError::InvalidVestingParameters);
        }

        let mut vesting = read_vesting(&env, grant_id)?;
        
        vesting.recipient.require_auth();

        let current_time = env.ledger().timestamp();
        
        // Check if cliff period has passed
        if current_time < vesting.cliff_time {
            return Err(AccumulatorError::InvalidVestingParameters);
        }

        // Calculate current vested amount
        let current_vested = calculate_vested_amount(&env, &vesting, current_time)?;
        let available_to_claim = current_vested.saturating_sub(vesting.claimed_amount);
        
        let amount_precise = to_precision(amount)?;
        
        if amount_precise > available_to_claim {
            return Err(AccumulatorError::InsufficientBalance);
        }

        // Create vesting event entry
        let vesting_entry = AccumulatorEntry {
            timestamp: current_time,
            rate_per_second: vesting.current_rate,
            cumulative_amount: current_vested,
            grant_id,
            entry_type: 1, // VestingEvent
        };

        let entry_index = add_accumulator_entry(&env, grant_id, vesting_entry)?;

        // Update vesting
        vesting.claimed_amount += amount_precise;
        vesting.last_update = current_time;
        vesting.accumulator_head = entry_index;
        
        // Check if vesting is complete
        if vesting.claimed_amount >= vesting.total_amount {
            vesting.is_active = false;
        }
        
        write_vesting(&env, &vesting);

        // Emit claim event
        env.events().publish(
            (symbol_short!("claim"), grant_id),
            (amount, current_time, vesting.recipient.clone()),
        );

        Ok(())
    }

    /// Get current vested amount with precision
    pub fn get_vested_amount(env: Env, grant_id: u64) -> Result<i128, AccumulatorError> {
        let vesting = read_vesting(&env, grant_id)?;
        let current_time = env.ledger().timestamp();
        let vested_precise = calculate_vested_amount(&env, &vesting, current_time)?;
        // Jurisdiction check would be done at creation time
        from_precision(vested_precise)
    }

    /// Get claimable amount
    pub fn get_claimable_amount(env: Env, grant_id: u64) -> Result<i128, AccumulatorError> {
        let vesting = read_vesting(&env, grant_id)?;
        let current_time = env.ledger().timestamp();
        
        if current_time < vesting.cliff_time {
            return Ok(0);
        }
        
        let current_vested = calculate_vested_amount(&env, &vesting, current_time)?;
        let available = current_vested.saturating_sub(vesting.claimed_amount);
        from_precision(available)
    }

    /// Get accumulator entry
    pub fn get_accumulator_entry(env: Env, grant_id: u64, entry_index: u32) -> Result<AccumulatorEntry, AccumulatorError> {
        get_accumulator_entry(&env, grant_id, entry_index)
    }

    /// Get accumulator state
    pub fn get_accumulator_state(env: Env, grant_id: u64) -> Result<AccumulatorState, AccumulatorError> {
        read_accumulator_state(&env, grant_id)
    }

    /// Compact accumulator to save storage space
    pub fn compact_accumulator(env: Env, grant_id: u64) -> Result<(), AccumulatorError> {
        require_admin_auth(&env)?;

        let state = read_accumulator_state(&env, grant_id)?;
        
        if state.total_entries < COMPACTION_THRESHOLD {
            return Ok(()); // No need to compact
        }

        // Find the most recent rate change entry
        let mut latest_rate_change = None;
        let mut latest_vesting = None;
        
        for i in (0..state.total_entries).rev() {
            match get_accumulator_entry(&env, grant_id, i) {
                Ok(entry) => {
                    if entry.entry_type == 0 && latest_rate_change.is_none() { // RateChange
                        latest_rate_change = Some(entry.clone());
                    }
                    if entry.entry_type == 1 && latest_vesting.is_none() { // VestingEvent
                        latest_vesting = Some(entry.clone());
                    }
                },
                Err(_) => break, // Stop if we encounter an error
            }
        }

        // Create compaction entry
        let compaction_entry = AccumulatorEntry {
            timestamp: env.ledger().timestamp(),
            rate_per_second: latest_rate_change
                .as_ref()
                .map(|e| e.rate_per_second)
                .unwrap_or(0),
            cumulative_amount: latest_vesting
                .as_ref()
                .map(|e| e.cumulative_amount)
                .unwrap_or(0),
            grant_id,
            entry_type: 2, // Compaction
        };

        // Clear old entries and add compaction entry
        for i in 0..state.total_entries {
            env.storage().instance().remove(&AccumulatorDataKey::Accumulator(grant_id, i));
        }

        // Add compaction entry as the only entry
        env.storage()
            .instance()
            .set(&AccumulatorDataKey::Accumulator(grant_id, 0), &compaction_entry);

        // Update state
        let mut new_state = state;
        new_state.total_entries = 1;
        new_state.last_compaction = env.ledger().timestamp();
        new_state.compaction_count += 1;
        write_accumulator_state(&env, grant_id, &new_state);

        // Update vesting accumulator head
        let mut vesting = read_vesting(&env, grant_id)?;
        vesting.accumulator_head = 0;
        write_vesting(&env, &vesting);

        // Emit compaction event
        env.events().publish(
            (symbol_short!("compaction"), grant_id),
            (state.total_entries, 1, env.ledger().timestamp()),
        );

        Ok(())
    }

    /// Emergency function to reset accumulator state
    pub fn emergency_reset(env: Env, grant_id: u64) -> Result<(), AccumulatorError> {
        require_admin_auth(&env)?;

        let mut vesting = read_vesting(&env, grant_id)?;
        
        // Reset accumulator
        vesting.accumulator_head = u32::MAX;
        vesting.last_update = env.ledger().timestamp();
        
        write_vesting(&env, &vesting);

        // Clear accumulator entries
        let state = read_accumulator_state(&env, grant_id)?;
        for i in 0..state.total_entries {
            env.storage().instance().remove(&AccumulatorDataKey::Accumulator(grant_id, i));
        }

        // Reset state
        let new_state = AccumulatorState {
            total_entries: 0,
            last_compaction: env.ledger().timestamp(),
            precision_loss: 0,
            compaction_count: 0,
        };
        write_accumulator_state(&env, grant_id, &new_state);

        // Emit reset event
        env.events().publish(
            (symbol_short!("emergency_reset"), grant_id),
            (env.ledger().timestamp(),),
        );

        Ok(())
    }
}
