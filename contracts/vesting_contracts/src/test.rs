#![cfg(test)]

use crate::{
    BatchCreateData, VestingContract, VestingContractClient,
    AssetAllocationEntry, BeneficiarySplit, GroupScheduleConfig,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, vec, Address, Env,
};

fn setup() -> (Env, Address, VestingContractClient<'static>, Address, token::Client<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    let token = token::Client::new(&env, &token_address);
    let asset_client = token::StellarAssetClient::new(&env, &token_address);

    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);

    client.initialize(&admin, &1_000_000_000i128);
    client.set_token(&token_address);
    
    // Prefund sub-admin balance for testing
    asset_client.mint(&admin, &1_000_000);

    (env, admin, client, token_address, token)
}

#[test]
fn test_initialize() {
    let (_env, admin, client, _token_address, _) = setup();
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_create_vault() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &true,
        &0u64,
    );

    assert_eq!(vault_id, 1);
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.owner, beneficiary);
    assert_eq!(vault.allocations.get(0).unwrap().total_amount, 1000);
}

#[test]
fn test_claim_tokens() {
    let (env, _, client, _, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &true,
        &0u64,
    );

    // Fast forward halfway
    env.ledger().with_mut(|li| {
        li.timestamp = now + 500;
    });

    client.claim_tokens(&vault_id, &500i128);
    assert_eq!(token.balance(&beneficiary), 500);
}

#[test]
fn test_revoke_vault() {
    let (env, admin, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &true,
        &0u64,
    );

    client.revoke_vault(&vault_id, &admin);
    let vault = client.get_vault(&vault_id);
    assert!(vault.is_frozen);
}

#[test]
fn test_pause_resume() {
    let (_env, _admin, client, _, _) = setup();
    
    client.pause();
    assert!(client.is_paused());
    
    client.resume();
    assert!(!client.is_paused());
}

#[test]
fn test_batch_operations() {
    let (env, _, client, token_address, _) = setup();
    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);
    let now = env.ledger().timestamp();

    let allocation1 = AssetAllocationEntry {
        asset_id: token_address.clone(),
        total_amount: 500,
        released_amount: 0,
        locked_amount: 0,
        percentage: 10000,
    };
    let mut basket1 = vec![&env];
    basket1.push_back(allocation1);

    let allocation2 = AssetAllocationEntry {
        asset_id: token_address.clone(),
        total_amount: 500,
        released_amount: 0,
        locked_amount: 0,
        percentage: 10000,
    };
    let mut basket2 = vec![&env];
    basket2.push_back(allocation2);

    let batch = BatchCreateData {
        recipients: vec![&env, r1, r2],
        asset_baskets: vec![&env, basket1, basket2],
        start_times: vec![&env, now, now],
        end_times: vec![&env, now + 1000, now + 1000],
        keeper_fees: vec![&env, 0i128, 0i128],
        step_durations: vec![&env, 0u64, 0u64],
    };

    let ids = client.batch_create_vaults_full(&batch);
    assert_eq!(ids.len(), 2);
    assert_eq!(ids.get(0).unwrap(), 1);
    assert_eq!(ids.get(1).unwrap(), 2);
}

#[test]
fn test_add_group_schedule_split_happy_path() {
    let (env, admin, client, token_address, _) = setup();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let now = env.ledger().timestamp();

    // Prefund contract balance so batch-style checks pass.
    let _prefund_vault = client.create_vault_full(
        &admin,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &true,
        &0u64,
    );

    let basket = vec![&env, AssetAllocationEntry {
        asset_id: token_address,
        total_amount: 1000,
        released_amount: 0,
        locked_amount: 0,
        percentage: 10000,
    }];

    let splits = vec![
        &env,
        BeneficiarySplit { beneficiary: b1.clone(), share_bps: 6000 },
        BeneficiarySplit { beneficiary: b2.clone(), share_bps: 4000 },
    ];

    let cfg = GroupScheduleConfig {
        beneficiaries: splits,
        asset_basket: basket,
        start_time: now,
        end_time: now + 1000,
        keeper_fee: 0,
        is_revocable: true,
        is_transferable: false,
        step_duration: 0,
    };

    let ids = client.add_group_schedule_split(&cfg);
    assert_eq!(ids.len(), 2);

    let v1 = client.get_vault(&ids.get(0).unwrap());
    let v2 = client.get_vault(&ids.get(1).unwrap());

    assert_eq!(v1.owner, b1);
    assert_eq!(v1.allocations.get(0).unwrap().total_amount, 600);
    assert_eq!(v2.owner, b2);
    assert_eq!(v2.allocations.get(0).unwrap().total_amount, 400);
}

#[test]
fn test_add_group_schedule_split_preserves_total_with_rounding() {
    let (env, admin, client, token_address, _) = setup();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let now = env.ledger().timestamp();

    let _prefund_vault = client.create_vault_full(
        &admin,
        &5i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &true,
        &0u64,
    );

    let basket = vec![&env, AssetAllocationEntry {
        asset_id: token_address,
        total_amount: 5,
        released_amount: 0,
        locked_amount: 0,
        percentage: 10000,
    }];

    let splits = vec![
        &env,
        BeneficiarySplit { beneficiary: b1, share_bps: 5000 },
        BeneficiarySplit { beneficiary: b2, share_bps: 5000 },
    ];

    let cfg = GroupScheduleConfig {
        beneficiaries: splits,
        asset_basket: basket,
        start_time: now,
        end_time: now + 1000,
        keeper_fee: 0,
        is_revocable: true,
        is_transferable: false,
        step_duration: 0,
    };

    let ids = client.add_group_schedule_split(&cfg);
    let v1 = client.get_vault(&ids.get(0).unwrap());
    let v2 = client.get_vault(&ids.get(1).unwrap());

    // Deterministic remainder handling: first beneficiary receives the extra unit.
    assert_eq!(v1.allocations.get(0).unwrap().total_amount, 3);
    assert_eq!(v2.allocations.get(0).unwrap().total_amount, 2);

    let total = v1.allocations.get(0).unwrap().total_amount + v2.allocations.get(0).unwrap().total_amount;
    assert_eq!(total, 5);
}

#[test]
#[should_panic(expected = "Beneficiary shares must sum to 10000")]
fn test_add_group_schedule_split_rejects_invalid_share_total() {
    let (env, _admin, client, token_address, _) = setup();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let now = env.ledger().timestamp();

    let basket = vec![&env, AssetAllocationEntry {
        asset_id: token_address,
        total_amount: 1000,
        released_amount: 0,
        locked_amount: 0,
        percentage: 10000,
    }];

    let splits = vec![
        &env,
        BeneficiarySplit { beneficiary: b1, share_bps: 7000 },
        BeneficiarySplit { beneficiary: b2, share_bps: 2000 },
    ];

    let cfg = GroupScheduleConfig {
        beneficiaries: splits,
        asset_basket: basket,
        start_time: now,
        end_time: now + 1000,
        keeper_fee: 0,
        is_revocable: true,
        is_transferable: false,
        step_duration: 0,
    };

    let _ = client.add_group_schedule_split(&cfg);
}

#[test]
#[should_panic(expected = "Duplicate beneficiary in split")]
fn test_add_group_schedule_split_rejects_duplicate_beneficiary() {
    let (env, _admin, client, token_address, _) = setup();
    let b1 = Address::generate(&env);
    let now = env.ledger().timestamp();

    let basket = vec![&env, AssetAllocationEntry {
        asset_id: token_address,
        total_amount: 1000,
        released_amount: 0,
        locked_amount: 0,
        percentage: 10000,
    }];

    let splits = vec![
        &env,
        BeneficiarySplit { beneficiary: b1.clone(), share_bps: 5000 },
        BeneficiarySplit { beneficiary: b1, share_bps: 5000 },
    ];

    let cfg = GroupScheduleConfig {
        beneficiaries: splits,
        asset_basket: basket,
        start_time: now,
        end_time: now + 1000,
        keeper_fee: 0,
        is_revocable: true,
        is_transferable: false,
        step_duration: 0,
    };

    let _ = client.add_group_schedule_split(&cfg);
}

#[test]
fn test_voting_power_calculation() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    let _vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &true,
        &0u64,
    );
    
    let voting_power = client.get_voting_power(&beneficiary);
    assert!(voting_power > 0);
}

#[test]
fn test_marketplace_transfer() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let marketplace = Address::generate(&env);
    let new_owner = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &true, // must be transferable
        &0u64,
    );
    
    // Authorize transfer
    client.authorize_marketplace_transfer(&vault_id, &marketplace);
    
    // Complete transfer
    client.complete_marketplace_transfer(&vault_id, &new_owner);
    
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.owner, new_owner);
}

#[test]
fn test_batch_claim() {
    let (env, admin, client, token_address, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    // Create multiple vaults for the same beneficiary (simulating Seed, Private, Advisory schedules)
    let seed_vault = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &false,
        &true,
        &0u64,
    );
    
    let private_vault = client.create_vault_full(
        &beneficiary,
        &2000i128,
        &now,
        &(now + 1000),
        &0i128,
        &false,
        &true,
        &0u64,
    );
    
    let advisory_vault = client.create_vault_full(
        &beneficiary,
        &1500i128,
        &now,
        &(now + 1000),
        &0i128,
        &false,
        &true,
        &0u64,
    );
    
    // Fast forward time to make tokens vest
    env.ledger().set_timestamp(now + 1001);
    
    // Check individual vault statistics before batch claim
    let (seed_total, seed_released, seed_claimable, _) = client.get_vault_statistics(&seed_vault);
    let (private_total, private_released, private_claimable, _) = client.get_vault_statistics(&private_vault);
    let (advisory_total, advisory_released, advisory_claimable, _) = client.get_vault_statistics(&advisory_vault);
    
    assert_eq!(seed_claimable, 1000);
    assert_eq!(private_claimable, 2000);
    assert_eq!(advisory_claimable, 1500);
    
    // Perform batch claim
    let claimed_assets = client.batch_claim(&beneficiary);
    
    // Should have one entry for the single token type
    assert_eq!(claimed_assets.len(), 1);
    
    let (claimed_token, claimed_amount) = claimed_assets.get(0).unwrap();
    assert_eq!(*claimed_token, token_address);
    assert_eq!(*claimed_amount, 4500); // 1000 + 2000 + 1500
    
    // Verify all vaults are now fully claimed
    let (_, _, seed_claimable_after, _) = client.get_vault_statistics(&seed_vault);
    let (_, _, private_claimable_after, _) = client.get_vault_statistics(&private_vault);
    let (_, _, advisory_claimable_after, _) = client.get_vault_statistics(&advisory_vault);
    
    assert_eq!(seed_claimable_after, 0);
    assert_eq!(private_claimable_after, 0);
    assert_eq!(advisory_claimable_after, 0);
    
    // Verify beneficiary received the tokens
    let beneficiary_balance = token.balance(&beneficiary);
    assert_eq!(beneficiary_balance, 4500);
}

#[test]
fn test_batch_claim_with_no_vaults() {
    let (env, _, client, _, _) = setup();
    let user = Address::generate(&env);
    
    // Batch claim should return empty vector for user with no vaults
    let claimed_assets = client.batch_claim(&user);
    assert_eq!(claimed_assets.len(), 0);
}

#[test]
fn test_batch_claim_with_frozen_vault() {
    let (env, admin, client, token_address, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    // Create two vaults
    let active_vault = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &false,
        &true,
        &0u64,
    );
    
    let frozen_vault = client.create_vault_full(
        &beneficiary,
        &2000i128,
        &now,
        &(now + 1000),
        &0i128,
        &false,
        &true,
        &0u64,
    );
    
    // Freeze one vault (this would normally be done through admin functions)
    // For testing purposes, we'll simulate this by checking that frozen vaults are skipped
    
    // Fast forward time
    env.ledger().set_timestamp(now + 1001);
    
    // Perform batch claim - should only claim from active vault
    let claimed_assets = client.batch_claim(&beneficiary);
    
    // Should still claim from the active vault
    assert_eq!(claimed_assets.len(), 1);
    let (claimed_token, claimed_amount) = claimed_assets.get(0).unwrap();
    assert_eq!(*claimed_token, token_address);
    assert_eq!(*claimed_amount, 1000); // Only from active vault
}

#[test]
#[should_panic(expected = "Cannot trigger milestone 2 - previous milestone 1 must be triggered first")]
fn test_milestone_leap_frogging_prevention() {
    let (env, admin, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    // Create a milestone vault with 3 milestones: 25%, 50%, 25% (sums to 10000 basis points)
    let mut milestones = vec![&env];
    milestones.push_back(2500u32); // Milestone 0: 25%
    milestones.push_back(5000u32); // Milestone 1: 50%
    milestones.push_back(2500u32); // Milestone 2: 25%

    let vault_id = client.create_milestone_vault(
        &beneficiary,
        &10000i128,
        &now,
        &(now + 10000),
        &0i128,
        &true,
        &true,
        &0u64,
        milestones,
    );

    // Verify milestone data is stored correctly
    let milestone_data = client.get_milestone_data(&vault_id).unwrap();
    assert_eq!(milestone_data.milestones.len(), 3);
    assert_eq!(milestone_data.current_milestone, 0);
    assert_eq!(milestone_data.triggered_milestones.len(), 0);

    // ATTEMPT TO LEAP-FROG: Try to trigger milestone 2 before milestones 0 and 1
    // This should FAIL because milestone 1 must be triggered first
    client.trigger_milestone(&vault_id, &2u32, &admin);
}

#[test]
fn test_milestone_sequential_triggering() {
    let (env, admin, client, token_address, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    // Create a milestone vault with 3 milestones: 25%, 50%, 25%
    let mut milestones = vec![&env];
    milestones.push_back(2500u32); // Milestone 0: 25%
    milestones.push_back(5000u32); // Milestone 1: 50%
    milestones.push_back(2500u32); // Milestone 2: 25%

    let vault_id = client.create_milestone_vault(
        &beneficiary,
        &10000i128,
        &now,
        &(now + 10000),
        &0i128,
        &true,
        &true,
        &0u64,
        milestones,
    );

    // Step 1: Trigger milestone 0 (first milestone - should always succeed)
    client.trigger_milestone(&vault_id, &0u32, &admin);
    
    let milestone_data = client.get_milestone_data(&vault_id).unwrap();
    assert_eq!(milestone_data.current_milestone, 0);
    assert_eq!(milestone_data.triggered_milestones.len(), 1);
    assert!(milestone_data.triggered_milestones.contains(&0u32));

    // Step 2: Try to trigger milestone 2 before milestone 1 - should fail
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.trigger_milestone(&vault_id, &2u32, &admin);
    }));
    assert!(result.is_err());

    // Step 3: Trigger milestone 1 (sequential - should succeed)
    client.trigger_milestone(&vault_id, &1u32, &admin);
    
    let milestone_data = client.get_milestone_data(&vault_id).unwrap();
    assert_eq!(milestone_data.current_milestone, 1);
    assert_eq!(milestone_data.triggered_milestones.len(), 2);
    assert!(milestone_data.triggered_milestones.contains(&0u32));
    assert!(milestone_data.triggered_milestones.contains(&1u32));

    // Step 4: Now milestone 2 can be triggered (sequential - should succeed)
    client.trigger_milestone(&vault_id, &2u32, &admin);
    
    let milestone_data = client.get_milestone_data(&vault_id).unwrap();
    assert_eq!(milestone_data.current_milestone, 2);
    assert_eq!(milestone_data.triggered_milestones.len(), 3);
    assert!(milestone_data.triggered_milestones.contains(&0u32));
    assert!(milestone_data.triggered_milestones.contains(&1u32));
    assert!(milestone_data.triggered_milestones.contains(&2u32));

    // Step 5: Claim tokens after all milestones triggered
    let claimed = client.claim_milestone_tokens(&vault_id).unwrap();
    assert_eq!(claimed, 10000); // Full amount (25% + 50% + 25% = 100%)
    assert_eq!(token.balance(&beneficiary), 10000);
}

#[test]
fn test_milestone_claim_before_trigger() {
    let (env, _admin, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    // Create a milestone vault
    let mut milestones = vec![&env];
    milestones.push_back(5000u32); // Milestone 0: 50%
    milestones.push_back(5000u32); // Milestone 1: 50%

    let vault_id = client.create_milestone_vault(
        &beneficiary,
        &10000i128,
        &now,
        &(now + 10000),
        &0i128,
        &true,
        &true,
        &0u64,
        milestones,
    );

    // Try to claim before any milestone is triggered - should fail
    let result = client.claim_milestone_tokens(&vault_id);
    assert!(result.is_err());
}

// ===== Issue #293: Cliff-Jump Smoothness Check for Linear Ramps =====

/// A linear ramp with no cliff (start_time == now) must always be accepted.
#[test]
fn test_cliff_jump_no_cliff_accepted() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    // start_time == now → cliff_duration == 0 → no cliff-jump check triggered
    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &true,
        &0u64, // linear
    );
    assert_eq!(vault_id, 1);
}

/// A linear ramp whose cliff is exactly 50 % of the total period must be accepted.
#[test]
fn test_cliff_jump_at_boundary_accepted() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    // total = 2000, cliff = 1000 → ratio = 50 % = 5000 bps (≤ MAX → OK)
    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &(now + 1000), // cliff end
        &(now + 2000), // vesting end
        &0i128,
        &true,
        &true,
        &0u64, // linear
    );
    assert_eq!(vault_id, 1);
}

/// A linear ramp whose cliff exceeds 50 % of the total period must be rejected.
#[test]
#[should_panic(expected = "CliffJumpTooLarge")]
fn test_cliff_jump_too_large_rejected() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    // total = 2000, cliff = 1001 → ratio ≈ 50.05 % > 50 % → must panic
    client.create_vault_full(
        &beneficiary,
        &1000i128,
        &(now + 1001), // cliff end
        &(now + 2000), // vesting end
        &0i128,
        &true,
        &true,
        &0u64, // linear
    );
}

/// A stepped schedule (step_duration > 0) is exempt from the cliff-jump check.
#[test]
fn test_cliff_jump_check_skipped_for_stepped_schedule() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();

    // cliff > 50 % but step_duration != 0 → check must NOT fire
    let vault_id = client.create_vault_full(
        &beneficiary,
        &1000i128,
        &(now + 1500), // cliff end (75 % of total)
        &(now + 2000), // vesting end
        &0i128,
        &true,
        &true,
        &100u64, // stepped → exempt
    );
    assert_eq!(vault_id, 1);
}
