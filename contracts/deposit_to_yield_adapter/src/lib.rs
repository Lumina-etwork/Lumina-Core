#![no_std]
use soroban_sdk::{
    contract,
    contractimpl,
    contracttype,
    contractevent,
    token,
    Address,
    Env,
    IntoVal,
    Symbol,
    Vec,
    String,
    U256,
    BytesN,
};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct LendingProtocol {
    pub address: Address,
    pub name: String,
    pub is_active: bool,
    pub risk_rating: u32, // 1-5, where 1 is lowest risk
    pub supported_assets: Vec<Address>,
    pub minimum_deposit: i128,
    pub maximum_deposit: i128,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct YieldPosition {
    pub protocol_address: Address,
    pub asset_address: Address,
    pub deposited_amount: i128,
    pub shares: i128,
    pub deposited_at: u64,
    pub last_yield_claim: u64,
    pub accumulated_yield: i128,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct VaultYieldSummary {
    pub vault_id: u64,
    pub total_deposited: i128,
    pub total_yield_accumulated: i128,
    pub active_positions: Vec<YieldPosition>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum AdapterDataKey {
    Admin,
    VestingContract,
    WhitelistedProtocols(Address), // protocol_address -> LendingProtocol
    VaultPositions(u64), // vault_id -> Vec<YieldPosition>
    ProtocolCounter,
    YieldSummary(u64), // vault_id -> VaultYieldSummary
    IsPaused,
    YieldTreasury, // Address where yield is collected
    InsuranceTreasury, // Address of insurance treasury for 1% fee
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum AdapterError {
    Unauthorized,
    ProtocolNotWhitelisted,
    InsufficientBalance,
    InvalidAmount,
    ContractPaused,
    AssetNotSupported,
    RiskRatingTooHigh,
    PositionNotFound,
    InvalidProtocol,
}

#[contractevent]
pub struct ProtocolWhitelisted {
    #[topic]
    pub protocol_address: Address,
    #[topic]
    pub name: String,
    pub risk_rating: u32,
}

#[contractevent]
pub struct ProtocolDelisted {
    #[topic]
    pub protocol_address: Address,
}

#[contractevent]
pub struct DepositedToYield {
    #[topic]
    pub vault_id: u64,
    #[topic]
    pub protocol_address: Address,
    #[topic]
    pub asset_address: Address,
    pub amount: i128,
    pub shares_received: i128,
}

#[contractevent]
pub struct YieldClaimed {
    #[topic]
    pub vault_id: u64,
    #[topic]
    pub protocol_address: Address,
    #[topic]
    pub asset_address: Address,
    pub yield_amount: i128,
}

#[contractevent]
pub struct InsuranceFundCapitalized {
    #[topic]
    pub asset: Address,
    pub amount: i128,
    pub total_balance: i128, // But we don't have total_balance here
}

#[contractevent]
pub struct PositionWithdrawn {
    #[topic]
    pub vault_id: u64,
    #[topic]
    pub protocol_address: Address,
    #[topic]
    pub asset_address: Address,
    pub amount_withdrawn: i128,
    pub yield_withdrawn: i128,
}

#[contract]
pub struct DepositToYieldAdapter;

#[contractimpl]
impl DepositToYieldAdapter {
    /// Initialize the adapter with admin and vesting contract addresses
    pub fn initialize(env: Env, admin: Address, vesting_contract: Address, yield_treasury: Address, insurance_treasury: Address) {
        if env.storage().instance().has(&AdapterDataKey::Admin) {
            panic!("Already initialized");
        }
        
        admin.require_auth();
        
        env.storage().instance().set(&AdapterDataKey::Admin, &admin);
        env.storage().instance().set(&AdapterDataKey::VestingContract, &vesting_contract);
        env.storage().instance().set(&AdapterDataKey::YieldTreasury, &yield_treasury);
        env.storage().instance().set(&AdapterDataKey::InsuranceTreasury, &insurance_treasury);
        env.storage().instance().set(&AdapterDataKey::IsPaused, &false);
        env.storage().instance().set(&AdapterDataKey::ProtocolCounter, &0u64);
    }

    /// Whitelist a lending protocol for yield generation
    pub fn whitelist_protocol(env: Env, admin: Address, protocol: LendingProtocol) {
        Self::require_admin(&env, &admin);
        Self::require_not_paused(&env);
        
        // Validate risk rating (only allow low-risk protocols: rating 1-2)
        if protocol.risk_rating > 2 {
            panic!("Risk rating too high. Only low-risk protocols (rating 1-2) are allowed");
        }
        
        // Validate protocol has supported assets
        if protocol.supported_assets.is_empty() {
            panic!("Protocol must support at least one asset");
        }
        
        let protocol_address = protocol.address.clone();
        
        // Store protocol
        env.storage().instance().set(
            &AdapterDataKey::WhitelistedProtocols(protocol_address.clone()),
            &protocol,
        );
        
        // Increment protocol counter
        let mut counter: u64 = env.storage().instance()
            .get(&AdapterDataKey::ProtocolCounter)
            .unwrap_or(0u64);
        counter += 1;
        env.storage().instance().set(&AdapterDataKey::ProtocolCounter, &counter);
        
        // Emit event
        ProtocolWhitelisted {
            protocol_address: protocol_address.clone(),
            name: protocol.name.clone(),
            risk_rating: protocol.risk_rating,
        }.publish(&env);
    }

    /// Remove a protocol from the whitelist
    pub fn delist_protocol(env: Env, admin: Address, protocol_address: Address) {
        Self::require_admin(&env, &admin);
        
        // Check if protocol exists
        let protocol = Self::get_whitelisted_protocol(&env, &protocol_address);
        
        // Remove protocol
        env.storage().instance().remove(&AdapterDataKey::WhitelistedProtocols(protocol_address.clone()));
        
        // Emit event
        ProtocolDelisted {
            protocol_address: protocol_address.clone(),
        }.publish(&env);
    }

    /// Deposit unvested tokens from a vault to a whitelisted lending protocol
    pub fn deposit_to_yield(
        env: Env,
        admin: Address,
        vault_id: u64,
        protocol_address: Address,
        asset_address: Address,
        amount: i128,
    ) -> i128 {
        Self::require_admin(&env, &admin);
        Self::require_not_paused(&env);
        
        if amount <= 0 {
            panic!("Amount must be positive");
        }
        
        // Check if protocol is whitelisted
        let protocol = Self::get_whitelisted_protocol(&env, &protocol_address);
        
        // Check if asset is supported by protocol
        if !protocol.supported_assets.contains(&asset_address) {
            panic!("Asset not supported by protocol");
        }
        
        // Check deposit limits
        if amount < protocol.minimum_deposit || amount > protocol.maximum_deposit {
            panic!("Amount outside protocol deposit limits");
        }
        
        // Get unvested amount from vesting contract
        let unvested_amount = Self::get_unvested_amount(&env, vault_id, &asset_address);
        if unvested_amount < amount {
            panic!("Insufficient unvested tokens");
        }
        
        // Transfer tokens from vesting contract to this adapter
        let vesting_contract = Self::get_vesting_contract(&env);
        let token_client = token::Client::new(&env, &asset_address);
        token_client.transfer(&vesting_contract, &env.current_contract_address(), &amount);
        
        // Deposit to lending protocol and receive shares
        let shares_received = Self::deposit_to_protocol(&env, &protocol_address, &asset_address, amount);
        
        // Create or update yield position
        let position = YieldPosition {
            protocol_address: protocol_address.clone(),
            asset_address: asset_address.clone(),
            deposited_amount: amount,
            shares: shares_received,
            deposited_at: env.ledger().timestamp(),
            last_yield_claim: 0,
            accumulated_yield: 0,
        };
        
        Self::add_or_update_position(&env, vault_id, position);
        
        // Update vault yield summary
        Self::update_yield_summary(&env, vault_id, amount, 0);
        
        // Emit event
        DepositedToYield {
            vault_id,
            protocol_address: protocol_address.clone(),
            asset_address: asset_address.clone(),
            amount,
            shares_received,
        }.publish(&env);
        
        shares_received
    }

    /// Claim accumulated yield from a position
    pub fn claim_yield(
        env: Env,
        admin: Address,
        vault_id: u64,
        protocol_address: Address,
        asset_address: Address,
    ) -> i128 {
        Self::require_admin(&env, &admin);
        Self::require_not_paused(&env);
        
        // Get the position
        let mut positions = Self::get_vault_positions(&env, vault_id);
        let mut position_index: Option<u32> = None;
        
        for (i, pos) in positions.iter().enumerate() {
            if pos.protocol_address == protocol_address && pos.asset_address == asset_address {
                position_index = Some(i.try_into().unwrap());
                break;
            }
        }
        
        let position_index = position_index.expect("Position not found");
        let mut position = positions.get(position_index).unwrap();
        
        // Calculate yield
        let current_value = Self::get_position_value(&env, &position);
        let yield_amount = current_value - position.deposited_amount - position.accumulated_yield;
        
        if yield_amount <= 0 {
            return 0; // No yield to claim
        }
        
        // Claim yield from protocol
        let claimed_yield = Self::claim_yield_from_protocol(&env, &protocol_address, &asset_address, yield_amount);
        
        // Update position
        position.accumulated_yield += claimed_yield;
        position.last_yield_claim = env.ledger().timestamp();
        positions.set(position_index, position);
        
        // Update storage
        env.storage().instance().set(&AdapterDataKey::VaultPositions(vault_id), &positions);
        
        // Update yield summary
        Self::update_yield_summary(&env, vault_id, 0, claimed_yield);
        
        // Calculate insurance fee (1%)
        let insurance_fee = claimed_yield / 100;
        let yield_to_treasury = claimed_yield - insurance_fee;
        
        // Transfer yield to treasury
        let treasury = Self::get_yield_treasury(&env);
        let token_client = token::Client::new(&env, &asset_address);
        if yield_to_treasury > 0 {
            token_client.transfer(&env.current_contract_address(), &treasury, &yield_to_treasury);
        }
        
        // Transfer insurance fee to insurance treasury
        if insurance_fee > 0 {
            let insurance_treasury = Self::get_insurance_treasury(&env);
            token_client.transfer(&env.current_contract_address(), &insurance_treasury, &insurance_fee);
            
            // Record the deposit in insurance treasury
            let args = vec![
                &env,
                env.current_contract_address().into_val(),
                asset_address.into_val(),
                insurance_fee.into_val(),
            ];
            env.invoke_contract(&insurance_treasury, &Symbol::new(&env, "record_deposit"), args);
        }
        
        // Emit event
        YieldClaimed {
            vault_id,
            protocol_address,
            asset_address,
            yield_amount: claimed_yield,
        }.publish(&env);
        
        claimed_yield
    }

    /// Withdraw a position from a lending protocol
    pub fn withdraw_position(
        env: Env,
        admin: Address,
        vault_id: u64,
        protocol_address: Address,
        asset_address: Address,
    ) -> (i128, i128) {
        Self::require_admin(&env, &admin);
        Self::require_not_paused(&env);
        
        // Get the position
        let mut positions = Self::get_vault_positions(&env, vault_id);
        let mut position_index: Option<u32> = None;
        
        for (i, pos) in positions.iter().enumerate() {
            if pos.protocol_address == protocol_address && pos.asset_address == asset_address {
                position_index = Some(i.try_into().unwrap());
                break;
            }
        }
        
        let position_index = position_index.expect("Position not found");
        let position = positions.get(position_index).unwrap();
        
        // Withdraw from protocol
        let (principal_withdrawn, yield_withdrawn) = Self::withdraw_from_protocol(
            &env,
            &protocol_address,
            &asset_address,
            position.shares,
        );
        
        // Remove position from storage
        positions.remove(position_index);
        env.storage().instance().set(&AdapterDataKey::VaultPositions(vault_id), &positions);
        
        // Transfer principal back to vesting contract
        let vesting_contract = Self::get_vesting_contract(&env);
        let token_client = token::Client::new(&env, &asset_address);
        
        if principal_withdrawn > 0 {
            token_client.transfer(&env.current_contract_address(), &vesting_contract, &principal_withdrawn);
        }
        
        if yield_withdrawn > 0 {
            // Calculate insurance fee (1%)
            let insurance_fee = yield_withdrawn / 100;
            let yield_to_treasury = yield_withdrawn - insurance_fee;
            
            // Transfer yield to treasury
            let treasury = Self::get_yield_treasury(&env);
            if yield_to_treasury > 0 {
                token_client.transfer(&env.current_contract_address(), &treasury, &yield_to_treasury);
            }
            
            // Transfer insurance fee to insurance treasury
            if insurance_fee > 0 {
                let insurance_treasury = Self::get_insurance_treasury(&env);
                token_client.transfer(&env.current_contract_address(), &insurance_treasury, &insurance_fee);
                
                // Record the deposit in insurance treasury
                let args = vec![
                    &env,
                    env.current_contract_address().into_val(),
                    asset_address.into_val(),
                    insurance_fee.into_val(),
                ];
                env.invoke_contract(&insurance_treasury, &Symbol::new(&env, "record_deposit"), args);
            }
        }
        
        // Update yield summary (remove deposited amount, add yield)
        Self::update_yield_summary(&env, vault_id, -principal_withdrawn, yield_withdrawn);
        
        // Emit event
        PositionWithdrawn {
            vault_id,
            protocol_address,
            asset_address,
            amount_withdrawn: principal_withdrawn,
            yield_withdrawn,
        }.publish(&env);
        
        (principal_withdrawn, yield_withdrawn)
    }

    /// Get all whitelisted protocols
    pub fn get_whitelisted_protocols(env: Env) -> Vec<LendingProtocol> {
        let mut protocols = Vec::new(&env);
        let counter: u64 = env.storage().instance()
            .get(&AdapterDataKey::ProtocolCounter)
            .unwrap_or(0u64);
        
        // Note: In a real implementation, we'd need a way to iterate through all protocols
        // For now, this is a placeholder that would need to be implemented based on storage design
        protocols
    }

    /// Get vault positions
    pub fn get_vault_positions(env: Env, vault_id: u64) -> Vec<YieldPosition> {
        env.storage().instance()
            .get(&AdapterDataKey::VaultPositions(vault_id))
            .unwrap_or(Vec::new(&env))
    }

    /// Get vault yield summary
    pub fn get_vault_yield_summary(env: Env, vault_id: u64) -> VaultYieldSummary {
        env.storage().instance()
            .get(&AdapterDataKey::YieldSummary(vault_id))
            .unwrap_or(VaultYieldSummary {
                vault_id,
                total_deposited: 0,
                total_yield_accumulated: 0,
                active_positions: Vec::new(&env),
            })
    }

    /// Pause/unpause the adapter
    pub fn set_pause(env: Env, admin: Address, paused: bool) {
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&AdapterDataKey::IsPaused, &paused);
    }

    // --- Internal Helper Functions ---

    fn require_admin(env: &Env, admin: &Address) {
        let stored_admin: Address = env.storage().instance()
            .get(&AdapterDataKey::Admin)
            .expect("Admin not set");
        if stored_admin != *admin {
            admin.require_auth();
        }
    }

    fn require_not_paused(env: &Env) {
        if env.storage().instance().get(&AdapterDataKey::IsPaused).unwrap_or(false) {
            panic!("Contract is paused");
        }
    }

    fn get_vesting_contract(env: &Env) -> Address {
        env.storage().instance()
            .get(&AdapterDataKey::VestingContract)
            .expect("Vesting contract not set")
    }

    fn get_yield_treasury(env: &Env) -> Address {
        env.storage().instance()
            .get(&AdapterDataKey::YieldTreasury)
            .expect("Yield treasury not set")
    }

    fn get_insurance_treasury(env: &Env) -> Address {
        env.storage().instance()
            .get(&AdapterDataKey::InsuranceTreasury)
            .expect("Insurance treasury not set")
    }

    fn get_whitelisted_protocol(env: &Env, protocol_address: &Address) -> LendingProtocol {
        env.storage().instance()
            .get(&AdapterDataKey::WhitelistedProtocols(protocol_address.clone()))
            .expect("Protocol not whitelisted")
    }

    fn get_unvested_amount(env: &Env, vault_id: u64, asset_address: &Address) -> i128 {
        // This would call the vesting contract to get unvested amount
        // For now, return a placeholder - in real implementation, this would be:
        // let vesting_client = VestingContractClient::new(env, &Self::get_vesting_contract(env));
        // vesting_client.get_unvested_amount(vault_id, asset_address)
        1000000i128 // Placeholder
    }

    fn deposit_to_protocol(env: &Env, protocol_address: &Address, asset_address: &Address, amount: i128) -> i128 {
        // This would call the lending protocol's deposit function
        // For now, return a placeholder - in real implementation, this would be:
        // let protocol_client = LendingProtocolClient::new(env, protocol_address);
        // protocol_client.deposit(asset_address, amount)
        amount // 1:1 share ratio as placeholder
    }

    fn get_position_value(env: &Env, position: &YieldPosition) -> i128 {
        // This would call the lending protocol to get current value of shares
        // For now, return deposited amount + some yield as placeholder
        position.deposited_amount + (position.deposited_amount / 100) // 1% yield placeholder
    }

    fn claim_yield_from_protocol(env: &Env, protocol_address: &Address, asset_address: &Address, amount: i128) -> i128 {
        // This would call the lending protocol's claim yield function
        // For now, return the amount as placeholder
        amount
    }

    fn withdraw_from_protocol(env: &Env, protocol_address: &Address, asset_address: &Address, shares: i128) -> (i128, i128) {
        // This would call the lending protocol's withdraw function
        // For now, return placeholder values
        let principal = shares; // 1:1 ratio as placeholder
        let yield_amount = shares / 50; // 2% yield placeholder
        (principal, yield_amount)
    }

    fn add_or_update_position(env: &Env, vault_id: u64, new_position: YieldPosition) {
        let mut positions = Self::get_vault_positions(env.clone(), vault_id);
        
        // Check if position already exists
        let mut found = false;
        for (i, pos) in positions.iter().enumerate() {
            if pos.protocol_address == new_position.protocol_address && pos.asset_address == new_position.asset_address {
                // Update existing position
                let updated_position = YieldPosition {
                    protocol_address: new_position.protocol_address.clone(),
                    asset_address: new_position.asset_address.clone(),
                    deposited_amount: pos.deposited_amount + new_position.deposited_amount,
                    shares: pos.shares + new_position.shares,
                    deposited_at: pos.deposited_at, // Keep original deposit time
                    last_yield_claim: pos.last_yield_claim,
                    accumulated_yield: pos.accumulated_yield,
                };
                positions.set(i.try_into().unwrap(), updated_position);
                found = true;
                break;
            }
        }
        
        if !found {
            // Add new position
            positions.push_back(new_position);
        }
        
        env.storage().instance().set(&AdapterDataKey::VaultPositions(vault_id), &positions);
    }

    fn update_yield_summary(env: &Env, vault_id: u64, deposited_change: i128, yield_change: i128) {
        let mut summary = Self::get_vault_yield_summary(env.clone(), vault_id);
        
        summary.total_deposited += deposited_change;
        summary.total_yield_accumulated += yield_change;
        summary.active_positions = Self::get_vault_positions(env.clone(), vault_id);
        
        env.storage().instance().set(&AdapterDataKey::YieldSummary(vault_id), &summary);
    }
}

#[cfg(test)]
mod test;
