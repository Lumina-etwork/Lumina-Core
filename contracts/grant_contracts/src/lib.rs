#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol, U256};

#[contract]
pub struct GrantContract;

const TOTAL_AMOUNT: Symbol = symbol_short!("TOTAL");
const START_TIME: Symbol = symbol_short!("START");
const END_TIME: Symbol = symbol_short!("END");
const RECIPIENT: Symbol = symbol_short!("RECIPIENT");
const CLAIMED: Symbol = symbol_short!("CLAIMED");
const VALIDATOR: Symbol = symbol_short!("VAL_ADDR");
const VAL_CLAIMED: Symbol = symbol_short!("VAL_CLMD");

// 10 years in seconds (Issue #44)
const MAX_DURATION: u64 = 315_360_000;

#[contractimpl]
impl GrantContract {
    pub fn initialize_grant(
        env: Env,
        recipient: Address,
        total_amount: U256,
        duration_seconds: u64,
        validator: Option<Address>,
    ) -> u64 {
        assert!(
            duration_seconds <= MAX_DURATION,
            "duration exceeds MAX_DURATION"
        );
        let start_time = env.ledger().timestamp();
        let end_time = start_time + duration_seconds;

        env.storage().instance().set(&TOTAL_AMOUNT, &total_amount);
        env.storage().instance().set(&START_TIME, &start_time);
        env.storage().instance().set(&END_TIME, &end_time);
        env.storage().instance().set(&RECIPIENT, &recipient);
        env.storage()
            .instance()
            .set(&CLAIMED, &U256::from_u32(&env, 0));
        
        if let Some(val_addr) = validator {
            env.storage().instance().set(&VALIDATOR, &val_addr);
            env.storage().instance().set(&VAL_CLAIMED, &U256::from_u32(&env, 0));
        }

        end_time
    }

    fn get_total_vested(env: &Env) -> U256 {
        let current_time = env.ledger().timestamp();
        let start_time = env.storage().instance().get(&START_TIME).unwrap_or(0);
        let end_time = env.storage().instance().get(&END_TIME).unwrap_or(0);
        let total_amount = env
            .storage()
            .instance()
            .get(&TOTAL_AMOUNT)
            .unwrap_or(U256::from_u32(&env, 0));

        if current_time <= start_time {
            return U256::from_u32(&env, 0);
        }

        let elapsed = if current_time >= end_time {
            end_time - start_time
        } else {
            current_time - start_time
        };

        let total_duration = end_time - start_time;
        if total_duration > 0 {
            let elapsed_u256 = U256::from_u32(&env, elapsed as u32);
            let duration_u256 = U256::from_u32(&env, total_duration as u32);
            total_amount.mul(&elapsed_u256).div(&duration_u256)
        } else {
            U256::from_u32(&env, 0)
        }
    }

    pub fn grantee_claimable(env: Env) -> U256 {
        let total_vested = Self::get_total_vested(&env);
        let claimed = env.storage().instance().get(&CLAIMED).unwrap_or(U256::from_u32(&env, 0));
        
        let share = if env.storage().instance().has(&VALIDATOR) {
            total_vested.mul(&U256::from_u32(&env, 95)).div(&U256::from_u32(&env, 100))
        } else {
            total_vested
        };

        if share > claimed {
            share.sub(&claimed)
        } else {
            U256::from_u32(&env, 0)
        }
    }

    pub fn validator_claimable(env: Env) -> U256 {
        if !env.storage().instance().has(&VALIDATOR) {
            return U256::from_u32(&env, 0);
        }
        let total_vested = Self::get_total_vested(&env);
        let claimed = env.storage().instance().get(&VAL_CLAIMED).unwrap_or(U256::from_u32(&env, 0));
        
        let share = total_vested.mul(&U256::from_u32(&env, 5)).div(&U256::from_u32(&env, 100));

        if share > claimed {
            share.sub(&claimed)
        } else {
            U256::from_u32(&env, 0)
        }
    }

    pub fn claimable_balance(env: Env) -> U256 {
        Self::grantee_claimable(env)
    }

    pub fn claim(env: Env, recipient: Address) -> U256 {
        recipient.require_auth();
        let stored_recipient: Address = env.storage().instance().get(&RECIPIENT).unwrap();
        assert_eq!(recipient, stored_recipient, "Unauthorized recipient");

        let claimable = Self::grantee_claimable(env.clone());
        assert!(claimable > U256::from_u32(&env, 0), "No tokens to claim");

        let claimed: U256 = env.storage().instance().get(&CLAIMED).unwrap();
        env.storage().instance().set(&CLAIMED, &claimed.add(&claimable));

        claimable
    }

    pub fn claim_validator(env: Env, validator: Address) -> U256 {
        validator.require_auth();
        let stored_validator: Address = env.storage().instance().get(&VALIDATOR).expect("No validator set");
        assert_eq!(validator, stored_validator, "Unauthorized validator");

        let claimable = Self::validator_claimable(env.clone());
        assert!(claimable > U256::from_u32(&env, 0), "No validator tokens to claim");

        let claimed: U256 = env.storage().instance().get(&VAL_CLAIMED).unwrap();
        env.storage().instance().set(&VAL_CLAIMED, &claimed.add(&claimable));

        claimable
    }

    pub fn get_grant_info(env: Env) -> (U256, u64, u64, U256, Option<Address>, U256) {
        let total_amount = env.storage().instance().get(&TOTAL_AMOUNT).unwrap_or(U256::from_u32(&env, 0));
        let start_time = env.storage().instance().get(&START_TIME).unwrap_or(0);
        let end_time = env.storage().instance().get(&END_TIME).unwrap_or(0);
        let claimed = env.storage().instance().get(&CLAIMED).unwrap_or(U256::from_u32(&env, 0));
        let validator = env.storage().instance().get(&VALIDATOR);
        let val_claimed = env.storage().instance().get(&VAL_CLAIMED).unwrap_or(U256::from_u32(&env, 0));
        
        (total_amount, start_time, end_time, claimed, validator, val_claimed)
    }
}

mod test;
