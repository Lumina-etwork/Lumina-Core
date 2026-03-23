#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, U256};

#[test]
fn test_basic_grant() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(GrantContract, ());
    let client = GrantContractClient::new(&env, &contract_id);

    let recipient = Address::generate(&env);
    let total_amount = U256::from_u32(&env, 1000);
    let duration = 100u64;

    client.initialize_grant(&recipient, &total_amount, &duration, &None);

    let claimable = client.claimable_balance();
    assert_eq!(claimable, U256::from_u32(&env, 0));
}

#[test]
fn test_grant_with_validator_split() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(GrantContract, ());
    let client = GrantContractClient::new(&env, &contract_id);

    let recipient = Address::generate(&env);
    let validator = Address::generate(&env);
    let total_amount = U256::from_u32(&env, 1000);
    let duration = 100u64;

    client.initialize_grant(&recipient, &total_amount, &duration, &Some(validator.clone()));

    // Advance time to 50%
    env.ledger().with_mut(|li| li.timestamp = 50);

    // Total vested should be 500
    // Grantee share (95%) = 475
    // Validator share (5%) = 25
    
    assert_eq!(client.grantee_claimable(), U256::from_u32(&env, 475));
    assert_eq!(client.validator_claimable(), U256::from_u32(&env, 25));

    // Grantee claims partial
    let claimed_grantee = client.claim(&recipient);
    assert_eq!(claimed_grantee, U256::from_u32(&env, 475));

    // Validator claims
    let claimed_validator = client.claim_validator(&validator);
    assert_eq!(claimed_validator, U256::from_u32(&env, 25));

    // Advance to 100%
    env.ledger().with_mut(|li| li.timestamp = 100);

    // Total vested 1000
    // Grantee total share 950, remaining 475
    // Validator total share 50, remaining 25

    assert_eq!(client.grantee_claimable(), U256::from_u32(&env, 475));
    assert_eq!(client.validator_claimable(), U256::from_u32(&env, 25));
}

#[test]
#[should_panic(expected = "duration exceeds MAX_DURATION")]
fn test_initialize_rejects_duration_over_max() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(GrantContract, ());
    let client = GrantContractClient::new(&env, &contract_id);

    let recipient = Address::generate(&env);
    let total_amount = U256::from_u32(&env, 1000);
    let duration = super::MAX_DURATION + 1;

    client.initialize_grant(&recipient, &total_amount, &duration, &None);
}
