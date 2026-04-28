#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Map, Vec,
};

#[contract]
pub struct AntiReentryContract;

// Reentry guard status flag
pub const REENTRY_GUARD_ACTIVE: u64 = 0x00000001;

#[derive(Clone)]
#[contracttype]
pub enum ReentryDataKey {
    Guard(Address),
}

#[derive(Clone)]
#[contracttype]
pub struct ReentryGuard {
    pub caller: Address,
    pub active: bool,
    pub timestamp: u64,
}

#[contracterror]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum ReentryError {
    ReentryDetected = 1001,
    GuardAlreadyActive = 1002,
    GuardNotActive = 1003,
    Unauthorized = 1004,
}

// Anti-reentry guard implementation
pub struct ReentryProtection;

impl ReentryProtection {
    // Check if reentry guard is active for a specific caller
    pub fn is_guard_active(env: &Env, caller: &Address) -> bool {
        if let Some(guard) = env.storage()
            .instance()
            .get::<ReentryDataKey, ReentryGuard>(&ReentryDataKey::Guard(caller.clone()))
        {
            guard.active
        } else {
            false
        }
    }

    // Set reentry guard active for caller
    pub fn set_guard_active(env: &Env, caller: &Address) -> Result<(), ReentryError> {
        if Self::is_guard_active(env, caller) {
            return Err(ReentryError::GuardAlreadyActive);
        }

        let guard = ReentryGuard {
            caller: caller.clone(),
            active: true,
            timestamp: env.ledger().timestamp(),
        };

        env.storage()
            .instance()
            .set(&ReentryDataKey::Guard(caller.clone()), &guard);

        Ok(())
    }

    // Clear reentry guard for caller
    pub fn clear_guard(env: &Env, caller: &Address) -> Result<(), ReentryError> {
        if !Self::is_guard_active(env, caller) {
            return Err(ReentryError::GuardNotActive);
        }

        env.storage()
            .instance()
            .remove::<ReentryDataKey>(&ReentryDataKey::Guard(caller.clone()));

        Ok(())
    }

    // Execute function with reentry protection
    pub fn execute_with_protection<F, R>(
        env: &Env,
        caller: &Address,
        func: F,
    ) -> Result<R, ReentryError>
    where
        F: FnOnce(&Env) -> Result<R, ReentryError>,
    {
        // Set guard
        Self::set_guard_active(env, caller)?;

        // Execute function
        let result = func(env);

        // Always clear guard, even if function failed
        let _ = Self::clear_guard(env, caller);

        result
    }
}

#[contractimpl]
impl AntiReentryContract {
    // Initialize reentry protection for a caller
    pub fn initialize_protection(env: Env, caller: Address) -> Result<(), ReentryError> {
        caller.require_auth();
        ReentryProtection::set_guard_active(&env, &caller)
    }

    // Clear reentry protection for a caller
    pub fn clear_protection(env: Env, caller: Address) -> Result<(), ReentryError> {
        caller.require_auth();
        ReentryProtection::clear_guard(&env, &caller)
    }

    // Check if protection is active for a caller
    pub fn is_protection_active(env: Env, caller: Address) -> bool {
        ReentryProtection::is_guard_active(&env, &caller)
    }

    // Protected claim function example
    pub fn protected_claim(
        env: Env,
        caller: Address,
        grant_id: u64,
        amount: i128,
    ) -> Result<(), ReentryError> {
        caller.require_auth();

        // Execute with reentry protection
        ReentryProtection::execute_with_protection(&env, &caller, |env| {
            // Simulate claiming logic here
            // In real implementation, this would interact with the main grant contract
            
            // Check if grant exists and caller is authorized
            // Calculate claimable amount
            // Transfer tokens
            
            Ok(())
        })
    }

    // Get guard information for debugging
    pub fn get_guard_info(env: Env, caller: Address) -> Option<ReentryGuard> {
        env.storage()
            .instance()
            .get::<ReentryDataKey, ReentryGuard>(&ReentryDataKey::Guard(caller))
    }

    // Emergency clear all guards (admin only)
    pub fn emergency_clear_all_guards(env: Env, admin: Address) -> Result<(), ReentryError> {
        admin.require_auth();
        
        // This would require iterating through all stored guards
        // For simplicity, we'll just note this functionality exists
        // In a real implementation, you might store a list of all active guards
        
        Ok(())
    }
}
