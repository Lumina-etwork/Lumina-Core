#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol, Vec, Map, U256};

#[contract]
pub struct GrantContract;

const TOTAL_AMOUNT: Symbol = symbol_short!("TOTAL");
const START_TIME: Symbol = symbol_short!("START");
const END_TIME: Symbol = symbol_short!("END");
const RECIPIENT: Symbol = symbol_short!("RECIPIENT");
const CLAIMED: Symbol = symbol_short!("CLAIMED");
const TAX_VAULT: Symbol = symbol_short!("TAX_VAULT");
const TAX_RATE: Symbol = symbol_short!("TAX_RATE");
const LAST_BALANCE_SYNC: Symbol = symbol_short!("BAL_SYNC");
const STREAM_DATA: Symbol = symbol_short!("STREAM");
const CLAWBACK_EVENT: Symbol = symbol_short!("CLAWBACK");

#[contracttype]
pub struct StreamData {
    pub recipient: Address,
    pub total_amount: U256,
    pub start_time: u64,
    pub end_time: u64,
    pub claimed: U256,
    pub tax_withheld: U256,
}

#[contracttype]
pub struct ClawbackEvent {
    pub clawback_amount: U256,
    pub previous_balance: U256,
    pub new_balance: U256,
    pub timestamp: u64,
}

#[contractimpl]
impl GrantContract {
    pub fn initialize_grant(
        env: Env,
        recipient: Address,
        total_amount: U256,
        duration_seconds: u64,
    ) -> u64 {
        let start_time = env.ledger().timestamp();
        let end_time = start_time + duration_seconds;
        
        env.storage().instance().set(&TOTAL_AMOUNT, &total_amount);
        env.storage().instance().set(&START_TIME, &start_time);
        env.storage().instance().set(&END_TIME, &end_time);
        env.storage().instance().set(&RECIPIENT, &recipient);
        env.storage().instance().set(&CLAIMED, &U256::from_u64(0));
        
        end_time
    }
    
    pub fn claimable_balance(env: Env) -> U256 {
        let current_time = env.ledger().timestamp();
        let start_time = env.storage().instance().get(&START_TIME).unwrap_or(0);
        let end_time = env.storage().instance().get(&END_TIME).unwrap_or(0);
        let total_amount = env.storage().instance().get(&TOTAL_AMOUNT).unwrap_or(U256::from_u64(0));
        let claimed = env.storage().instance().get(&CLAIMED).unwrap_or(U256::from_u64(0));
        
        if current_time <= start_time {
            return U256::from_u64(0);
        }
        
        let elapsed = if current_time >= end_time {
            end_time - start_time
        } else {
            current_time - start_time
        };
        
        let total_duration = end_time - start_time;
        let vested = if total_duration > 0 {
            total_amount * U256::from_u64(elapsed) / U256::from_u64(total_duration)
        } else {
            U256::from_u64(0)
        };
        
        if vested > claimed {
            vested - claimed
        } else {
            U256::from_u64(0)
        }
    }
    
    pub fn claim(env: Env, recipient: Address) -> U256 {
        recipient.require_auth();
        
        let stored_recipient = env.storage().instance().get(&RECIPIENT).unwrap();
        assert_eq!(recipient, stored_recipient, "Unauthorized recipient");
        
        let claimable = Self::claimable_balance(env.clone());
        assert!(claimable > U256::from_u64(0), "No tokens to claim");
        
        let claimed = env.storage().instance().get(&CLAIMED).unwrap_or(U256::from_u64(0));
        let new_claimed = claimed + claimable;
        env.storage().instance().set(&CLAIMED, &new_claimed);
        
        claimable
    }
    
    pub fn get_grant_info(env: Env) -> (U256, u64, u64, U256) {
        let total_amount = env.storage().instance().get(&TOTAL_AMOUNT).unwrap_or(U256::from_u64(0));
        let start_time = env.storage().instance().get(&START_TIME).unwrap_or(0);
        let end_time = env.storage().instance().get(&END_TIME).unwrap_or(0);
        let claimed = env.storage().instance().get(&CLAIMED).unwrap_or(U256::from_u64(0));
        
        (total_amount, start_time, end_time, claimed)
    }
    
    // Initialize grant with tax withholding
    pub fn initialize_grant_with_tax(
        env: Env,
        recipient: Address,
        total_amount: U256,
        duration_seconds: u64,
        tax_rate: u32, // Tax rate as percentage (e.g., 15 for 15%)
    ) -> u64 {
        let start_time = env.ledger().timestamp();
        let end_time = start_time + duration_seconds;
        
        require!(tax_rate <= 100, "Tax rate must be 0-100");
        
        env.storage().instance().set(&TOTAL_AMOUNT, &total_amount);
        env.storage().instance().set(&START_TIME, &start_time);
        env.storage().instance().set(&END_TIME, &end_time);
        env.storage().instance().set(&RECIPIENT, &recipient);
        env.storage().instance().set(&CLAIMED, &U256::from_u64(0));
        env.storage().instance().set(&TAX_RATE, &tax_rate);
        env.storage().instance().set(&TAX_VAULT, &U256::from_u64(0));
        
        // Create stream data
        let stream_data = StreamData {
            recipient: recipient.clone(),
            total_amount,
            start_time,
            end_time,
            claimed: U256::from_u64(0),
            tax_withheld: U256::from_u64(0),
        };
        env.storage().instance().set(&STREAM_DATA, &stream_data);
        
        end_time
    }
    
    // Claim with tax withholding
    pub fn claim_with_tax(env: Env, recipient: Address) -> (U256, U256) {
        recipient.require_auth();
        
        let stored_recipient = env.storage().instance().get(&RECIPIENT).unwrap();
        assert_eq!(recipient, stored_recipient, "Unauthorized recipient");
        
        let claimable = Self::claimable_balance(env.clone());
        assert!(claimable > U256::from_u64(0), "No tokens to claim");
        
        let tax_rate: u32 = env.storage().instance().get(&TAX_RATE).unwrap_or(0);
        let tax_amount = if tax_rate > 0 {
            (claimable * U256::from_u32(tax_rate)) / U256::from_u32(100)
        } else {
            U256::from_u64(0)
        };
        
        let net_amount = claimable - tax_amount;
        
        // Update claimed amount
        let claimed = env.storage().instance().get(&CLAIMED).unwrap_or(U256::from_u64(0));
        let new_claimed = claimed + claimable;
        env.storage().instance().set(&CLAIMED, &new_claimed);
        
        // Update tax vault
        let tax_vault = env.storage().instance().get(&TAX_VAULT).unwrap_or(U256::from_u64(0));
        let new_tax_vault = tax_vault + tax_amount;
        env.storage().instance().set(&TAX_VAULT, &new_tax_vault);
        
        // Update stream data
        let mut stream_data: StreamData = env.storage().instance().get(&STREAM_DATA).unwrap();
        stream_data.claimed = new_claimed;
        stream_data.tax_withheld = new_tax_vault;
        env.storage().instance().set(&STREAM_DATA, &stream_data);
        
        // Emit tax receipt event (simplified - in real implementation would use events)
        env.events().publish(
            (symbol_short!("TAX_RECEIPT"), recipient),
            (tax_amount, new_tax_vault, env.ledger().timestamp())
        );
        
        (net_amount, tax_amount)
    }
    
    // Withdraw from tax vault (grantor only)
    pub fn withdraw_tax_vault(env: Env, grantor: Address, amount: U256) -> U256 {
        grantor.require_auth();
        
        let tax_vault = env.storage().instance().get(&TAX_VAULT).unwrap_or(U256::from_u64(0));
        assert!(amount <= tax_vault, "Insufficient tax vault balance");
        
        let new_tax_vault = tax_vault - amount;
        env.storage().instance().set(&TAX_VAULT, &new_tax_vault);
        
        // Update stream data
        let mut stream_data: StreamData = env.storage().instance().get(&STREAM_DATA).unwrap();
        stream_data.tax_withheld = new_tax_vault;
        env.storage().instance().set(&STREAM_DATA, &stream_data);
        
        amount
    }
    
    // Sync balance for clawback compatibility
    pub fn balance_sync(env: Env, current_balance: U256) {
        let last_balance: U256 = env.storage().instance().get(&LAST_BALANCE_SYNC).unwrap_or_else(|| {
            // Initialize with current balance on first sync
            env.storage().instance().set(&LAST_BALANCE_SYNC, &current_balance);
            current_balance
        });
        
        if current_balance < last_balance {
            // Clawback detected
            let clawback_amount = last_balance - current_balance;
            
            // Create clawback event
            let clawback_event = ClawbackEvent {
                clawback_amount,
                previous_balance: last_balance,
                new_balance: current_balance,
                timestamp: env.ledger().timestamp(),
            };
            
            // Store clawback event
            env.storage().instance().set(&CLAWBACK_EVENT, &clawback_event);
            
            // Emit external clawback detected event
            env.events().publish(
                symbol_short!("EXTERNAL_CLAWBACK"),
                (clawback_amount, current_balance, env.ledger().timestamp())
            );
            
            // Pro-rata recalibration of active streams
            Self::recalibrate_streams_pro_rata(env, current_balance);
        }
        
        // Update last balance sync
        env.storage().instance().set(&LAST_BALANCE_SYNC, &current_balance);
    }
    
    // Recalibrate streams pro-rata after clawback
    fn recalibrate_streams_pro_rata(env: Env, new_total_balance: U256) {
        let mut stream_data: StreamData = env.storage().instance().get(&STREAM_DATA).unwrap();
        
        let original_total = stream_data.total_amount;
        let claimed = stream_data.claimed;
        let remaining_original = original_total - claimed;
        
        if remaining_original > U256::from_u64(0) && new_total_balance > claimed {
            // Calculate new total amount proportionally
            let new_total_amount = (new_total_balance * original_total) / remaining_original;
            
            // Update stream data with recalculated amounts
            stream_data.total_amount = new_total_amount;
            env.storage().instance().set(&STREAM_DATA, &stream_data);
            env.storage().instance().set(&TOTAL_AMOUNT, &new_total_amount);
        }
    }
    
    // Get tax vault balance
    pub fn get_tax_vault_balance(env: Env) -> U256 {
        env.storage().instance().get(&TAX_VAULT).unwrap_or(U256::from_u64(0))
    }
    
    // Get stream data
    pub fn get_stream_data(env: Env) -> StreamData {
        env.storage().instance().get(&STREAM_DATA).unwrap()
    }
    
    // Get last clawback event
    pub fn get_clawback_event(env: Env) -> Option<ClawbackEvent> {
        env.storage().instance().get(&CLAWBACK_EVENT)
    }
}

mod test;
