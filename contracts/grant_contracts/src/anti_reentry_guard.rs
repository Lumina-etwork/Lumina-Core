#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env,
};

use crate::optimized::{Grant, Error, DataKey, read_grant, write_grant, settle_grant, has_status, STATUS_ACTIVE};

#[contract]
pub struct AntiReentryGrantContract;

// Reentry protection status
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ReentryStatus {
    None = 0,
    ClaimInProgress = 1,
    ExternalTransferInProgress = 2,
}

// Reentry guard data structure
#[derive(Clone)]
#[contracttype]
pub struct ReentryGuard {
    pub status: u32,
    pub caller: Address,
    pub grant_id: u64,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub enum ReentryDataKey {
    Guard(Address, u64), // (caller, grant_id)
    GlobalLock,
}

#[contracterror]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum ReentryError {
    ReentryDetected = 1000,
    ExternalTransferInProgress = 1001,
    ClaimInProgress = 1002,
    InvalidReentryState = 1003,
    ReentryTimeout = 1004,
}

// Constants for reentry protection
pub const REENTRY_TIMEOUT_SECONDS: u64 = 300; // 5 minutes timeout for safety

// Helper functions for reentry guard management
fn set_reentry_guard(env: &Env, caller: &Address, grant_id: u64, status: ReentryStatus) -> Result<(), ReentryError> {
    let key = ReentryDataKey::Guard(caller.clone(), grant_id);
    let guard = ReentryGuard {
        status: status as u32,
        caller: caller.clone(),
        grant_id,
        timestamp: env.ledger().timestamp(),
    };
    
    env.storage().instance().set(&key, &guard);
    
    // Set temporary storage with TTL for safety
    env.storage().instance().extend_ttl(&key, REENTRY_TIMEOUT_SECONDS.try_into().unwrap(), REENTRY_TIMEOUT_SECONDS.try_into().unwrap());
    
    Ok(())
}

fn clear_reentry_guard(env: &Env, caller: &Address, grant_id: u64) {
    let key = ReentryDataKey::Guard(caller.clone(), grant_id);
    env.storage().instance().remove(&key);
}

fn check_reentry_guard(env: &Env, caller: &Address, grant_id: u64) -> Result<Option<ReentryGuard>, ReentryError> {
    let key = ReentryDataKey::Guard(caller.clone(), grant_id);
    
    if let Some(guard) = env.storage().instance().get::<_, ReentryGuard>(&key) {
        let now = env.ledger().timestamp();
        
        // Check for timeout
        if now > guard.timestamp.checked_add(REENTRY_TIMEOUT_SECONDS).unwrap_or(u64::MAX) {
            clear_reentry_guard(env, caller, grant_id);
            return Ok(None);
        }
        
        // Check if this is the same caller (allowed) or different caller (reentry)
        if guard.caller != *caller {
            return Err(ReentryError::ReentryDetected);
        }
        
        return Ok(Some(guard));
    }
    
    Ok(None)
}

// Global lock mechanism for critical sections
fn set_global_lock(env: &Env, lock_type: ReentryStatus) -> Result<(), ReentryError> {
    let key = ReentryDataKey::GlobalLock;
    let guard = ReentryGuard {
        status: lock_type as u32,
        caller: Address::from_string(&String::from_str(&env, "GD...")),
        grant_id: 0,
        timestamp: env.ledger().timestamp(),
    };
    
    if env.storage().instance().has(&key) {
        return Err(ReentryError::ReentryDetected);
    }
    
    env.storage().instance().set(&key, &guard);
    env.storage().instance().extend_ttl(&key, REENTRY_TIMEOUT_SECONDS.try_into().unwrap(), REENTRY_TIMEOUT_SECONDS.try_into().unwrap());
    
    Ok(())
}

fn clear_global_lock(env: &Env) {
    let key = ReentryDataKey::GlobalLock;
    env.storage().instance().remove(&key);
}

#[contractimpl]
impl AntiReentryGrantContract {
    /// Initialize the anti-reentry system
    pub fn initialize(env: Env) -> Result<(), ReentryError> {
        // No initialization needed for basic reentry guard
        Ok(())
    }
    
    /// Withdraw with anti-reentry protection
    pub fn withdraw_with_guard(
        env: Env,
        grant_id: u64,
        amount: i128,
        external_transfer: bool,
    ) -> Result<(), Error> {
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut grant = read_grant(&env, grant_id)?;

        // Can only withdraw from active grants
        if !has_status(grant.status_mask, STATUS_ACTIVE) {
            return Err(Error::InvalidState);
        }

        grant.recipient.require_auth();

        // Check for reentry
        let caller = grant.recipient.clone();
        match check_reentry_guard(&env, &caller, grant_id) {
            Ok(Some(guard)) => {
                // Same caller, check if we're in the correct state
                if external_transfer && guard.status == ReentryStatus::ClaimInProgress as u32 {
                    return Err(Error::InvalidState); // Can't do external transfer during claim
                }
                if !external_transfer && guard.status == ReentryStatus::ExternalTransferInProgress as u32 {
                    return Err(Error::InvalidState); // Can't claim during external transfer
                }
            },
            Ok(None) => {
                // No guard exists, proceed to set one
            },
            Err(_) => {
                return Err(Error::InvalidState); // Reentry detected
            }
        }

        // Set reentry guard
        let status = if external_transfer {
            ReentryStatus::ExternalTransferInProgress
        } else {
            ReentryStatus::ClaimInProgress
        };
        
        set_reentry_guard(&env, &caller, grant_id, status)
            .map_err(|_| Error::InvalidState)?;

        // Perform the actual withdrawal logic
        let result = Self::perform_withdrawal(&env, &mut grant, amount, external_transfer);

        // Clear reentry guard regardless of result
        clear_reentry_guard(&env, &caller, grant_id);

        result?;

        write_grant(&env, grant_id, &grant);

        // Emit withdrawal event
        env.events().publish(
            (symbol_short!("withdraw"), grant_id),
            (amount, external_transfer, caller),
        );

        Ok(())
    }

    /// Internal withdrawal logic
    fn perform_withdrawal(
        env: &Env,
        grant: &mut Grant,
        amount: i128,
        external_transfer: bool,
    ) -> Result<(), Error> {
        settle_grant(grant, env.ledger().timestamp())?;

        if amount > grant.claimable {
            return Err(Error::InvalidAmount);
        }

        grant.claimable = grant
            .claimable
            .checked_sub(amount)
            .ok_or(Error::MathOverflow)?;
        grant.withdrawn = grant
            .withdrawn
            .checked_add(amount)
            .ok_or(Error::MathOverflow)?;

        let accounted = grant
            .withdrawn
            .checked_add(grant.claimable)
            .ok_or(Error::MathOverflow)?;

        if accounted == grant.total_amount {
            // Mark as completed
            use crate::optimized::{STATUS_COMPLETED, set_status, clear_status};
            grant.status_mask = set_status(grant.status_mask, STATUS_COMPLETED);
            grant.status_mask = clear_status(grant.status_mask, STATUS_ACTIVE);
        }

        // If this is an external transfer, we would typically call an external contract here
        // For now, we just simulate the transfer logic
        if external_transfer {
            // In a real implementation, this would call the external transfer contract
            // and handle the callback securely
            Self::handle_external_transfer(env, grant, amount)?;
        }

        Ok(())
    }

    /// Handle external transfer logic
    fn handle_external_transfer(
        env: &Env,
        grant: &Grant,
        amount: i128,
    ) -> Result<(), Error> {
        // This is where you would implement the actual external transfer
        // For security, this should:
        // 1. Validate the external contract address
        // 2. Implement proper callback protection
        // 3. Handle transfer failures gracefully
        // 4. Ensure atomicity of the operation
        
        // For now, we'll just emit an event
        env.events().publish(
            (symbol_short!("ext_transfer"), grant.recipient.clone()),
            (amount, env.ledger().timestamp()),
        );

        Ok(())
    }

    /// Check if a grant is currently locked due to reentry protection
    pub fn is_grant_locked(env: Env, caller: Address, grant_id: u64) -> Result<bool, ReentryError> {
        match check_reentry_guard(&env, &caller, grant_id)? {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }

    /// Emergency clear function for administrators (use with caution)
    pub fn emergency_clear_guard(env: Env, caller: Address, grant_id: u64) -> Result<(), ReentryError> {
        // This should only be callable by admin in a real implementation
        clear_reentry_guard(&env, &caller, grant_id);
        Ok(())
    }

    /// Get reentry guard information
    pub fn get_guard_info(env: Env, caller: Address, grant_id: u64) -> Result<Option<ReentryGuard>, ReentryError> {
        check_reentry_guard(&env, &caller, grant_id)
    }
}
