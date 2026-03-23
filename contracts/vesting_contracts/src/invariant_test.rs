use crate::{VestingContract, VestingContractClient, Milestone, BatchCreateData};
use soroban_sdk::{testutils::{Address as _, Ledger}, token, vec, Address, Env};
use proptest::prelude::*;

fn setup_env() -> (Env, Address, VestingContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &1_000_000_000i128);

    let token_admin = Address::generate(&env);
    let token_addr = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    client.set_token(&token_addr);
    client.add_to_whitelist(&token_addr);

    let stellar = token::StellarAssetClient::new(&env, &token_addr);
    stellar.mint(&contract_id, &1_000_000_000i128);

    (env, contract_id, client, admin, token_addr)
}

#[test]
fn test_math_invariant_linear() {
    let (env, _, client, _, _) = setup_env();
    let beneficiary = Address::generate(&env);
    let start = 1000u64;
    let end = 5000u64;
    let amount = 1_000_000i128;
    
    let vault_id = client.create_vault_full(
        &beneficiary,
        &amount,
        &start,
        &end,
        &0i128,
        &false,
        &false,
        &0u64,
    );

    // Test multiple timestamps
    for t in 0..6000 {
        env.ledger().set_timestamp(t);
        let claimable = client.get_claimable_amount(&vault_id);
        let vault = client.get_vault(&vault_id);
        
        // Invariant 1: claimable + released <= total
        assert!(claimable + vault.released_amount <= vault.total_amount, 
            "Invariant 1 failed at t={}: {} + {} > {}", t, claimable, vault.released_amount, vault.total_amount);
        
        // Invariant 2: claimable >= 0
        assert!(claimable >= 0, "Invariant 2 failed at t={}", t);
        
        // Invariant 3: at end_time, everything is claimable
        if t >= end {
            assert_eq!(claimable + vault.released_amount, vault.total_amount, "Invariant 3 failed at t={}", t);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]
    #[test]
    fn test_invariant_randomized(
        amount in 100..1_000_000_000i128,
        duration in 1000..315_360_000u64,
        step_duration in 0..2000u64,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(VestingContract, ());
        let client = VestingContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin, &amount);
        
        let token_admin = Address::generate(&env);
        let token_addr = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
        client.set_token(&token_addr);
        client.add_to_whitelist(&token_addr);
        
        token::StellarAssetClient::new(&env, &token_addr).mint(&contract_id, &amount);
        
        let beneficiary = Address::generate(&env);
        let start = 10000u64;
        let end = start + duration;
        
        let vault_id = client.create_vault_full(
            &beneficiary,
            &amount,
            &start,
            &end,
            &0i128,
            &false,
            &false,
            &step_duration,
        );
        
        // Sample at 10 random points including boundaries
        for i in 0..10 {
            let t = if i == 0 { 0 } 
                   else if i == 1 { start }
                   else if i == 2 { end }
                   else { (start + (duration * i as u64 / 10)) };
            
            env.ledger().set_timestamp(t);
            let claimable = client.get_claimable_amount(&vault_id);
            let vault = client.get_vault(&vault_id);
            
            assert!(claimable + vault.released_amount <= amount, "Invariant Violation! Released: {}, Claimable: {}, Total: {}", vault.released_amount, claimable, amount);
            if t >= end {
                 assert_eq!(claimable + vault.released_amount, amount, "Final unlock invariant failed!");
            }
        }
    }
}

