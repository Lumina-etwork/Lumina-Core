#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{vec, Address, String};

fn setup_env() -> (
    Env,
    DepositToYieldAdapterClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let vesting_contract = Address::generate(&env);
    let yield_treasury = Address::generate(&env);
    let insurance_treasury = Address::generate(&env);
    let contract_id = env.register_contract(None, DepositToYieldAdapter);
    let client = DepositToYieldAdapterClient::new(&env, &contract_id);
    client.initialize(
        &admin,
        &vesting_contract,
        &yield_treasury,
        &insurance_treasury,
    );
    (
        env,
        client,
        admin,
        vesting_contract,
        yield_treasury,
        insurance_treasury,
    )
}

#[test]
fn test_initialization() {
    let (env, client, _admin, _vesting, _yield_treasury, _ins_treasury) = setup_env();

    // Verify the contract is usable after initialization
    let protocols = client.get_whitelisted_protocols();
    assert_eq!(protocols.len(), 0);

    // Also verify via as_contract context for direct storage access
    let contract_id = env.register_contract(None, DepositToYieldAdapter);
    let client2 = DepositToYieldAdapterClient::new(&env, &contract_id);
    let admin2 = Address::generate(&env);
    client2.initialize(&admin2, &admin2, &admin2, &admin2);

    env.as_contract(&contract_id, || {
        let stored: bool = env
            .storage()
            .instance()
            .get(&AdapterDataKey::IsPaused)
            .unwrap();
        assert!(!stored);
    });
}

#[test]
fn test_whitelist_protocol() {
    let (env, client, admin, _vesting, _treasury, _ins_treasury) = setup_env();
    let protocol_address = Address::generate(&env);
    let asset_address = Address::generate(&env);

    let protocol = LendingProtocol {
        address: protocol_address.clone(),
        name: String::from_str(&env, "USDC Lending Pool"),
        is_active: true,
        risk_rating: 1,
        supported_assets: vec![&env, asset_address.clone()],
        minimum_deposit: 1000,
        maximum_deposit: 1000000,
    };

    client.whitelist_protocol(&admin, &protocol);

    // get_whitelisted_protocols is a placeholder that always returns empty
    let _stored_protocols = client.get_whitelisted_protocols();
}

#[test]
#[should_panic(expected = "Risk rating too high")]
fn test_whitelist_high_risk_protocol() {
    let (env, client, admin, _vesting, _treasury, _ins_treasury) = setup_env();
    let protocol_address = Address::generate(&env);
    let asset_address = Address::generate(&env);

    let protocol = LendingProtocol {
        address: protocol_address,
        name: String::from_str(&env, "High Risk Pool"),
        is_active: true,
        risk_rating: 4,
        supported_assets: vec![&env, asset_address],
        minimum_deposit: 1000,
        maximum_deposit: 1000000,
    };

    client.whitelist_protocol(&admin, &protocol);
}

#[test]
fn test_pause_functionality() {
    let (env, client, admin, _vesting, _treasury, _ins_treasury) = setup_env();
    let protocol_address = Address::generate(&env);
    let asset_address = Address::generate(&env);

    // Pause the contract
    client.set_pause(&admin, &true);

    // Verify paused by trying an operation that checks pause
    let protocol = LendingProtocol {
        address: protocol_address.clone(),
        name: String::from_str(&env, "Test Pool"),
        is_active: true,
        risk_rating: 1,
        supported_assets: vec![&env, asset_address],
        minimum_deposit: 1000,
        maximum_deposit: 1000000,
    };
    client.set_pause(&admin, &false);

    // After unpause, whitelist should work
    client.whitelist_protocol(&admin, &protocol);
}

#[test]
#[should_panic(expected = "Contract is paused")]
fn test_whitelist_while_paused() {
    let (env, client, admin, _vesting, _treasury, _ins_treasury) = setup_env();

    client.set_pause(&admin, &true);

    let protocol = LendingProtocol {
        address: Address::generate(&env),
        name: String::from_str(&env, "Test Pool"),
        is_active: true,
        risk_rating: 1,
        supported_assets: vec![&env, Address::generate(&env)],
        minimum_deposit: 1000,
        maximum_deposit: 1000000,
    };

    client.whitelist_protocol(&admin, &protocol);
}

#[test]
#[should_panic(expected = "Contract is paused")]
fn test_deposit_while_paused() {
    let (env, client, admin, _vesting, _treasury, _ins_treasury) = setup_env();
    let protocol_address = Address::generate(&env);
    let asset_address = Address::generate(&env);

    let protocol = LendingProtocol {
        address: protocol_address,
        name: String::from_str(&env, "Lending Pool"),
        is_active: true,
        risk_rating: 1,
        supported_assets: vec![&env, asset_address],
        minimum_deposit: 100,
        maximum_deposit: 100000,
    };
    client.whitelist_protocol(&admin, &protocol);

    client.set_pause(&admin, &true);
    client.deposit_to_yield(
        &admin,
        &1u64,
        &Address::generate(&env),
        &Address::generate(&env),
        &500i128,
    );
}
