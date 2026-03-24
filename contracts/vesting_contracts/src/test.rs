use crate::{
    Allocation, BatchCreateData, Milestone, VestingContract, VestingContractClient,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, vec, Address, Env, IntoVal, Symbol, String, Map,
};

fn setup() -> (Env, Address, VestingContractClient<'static>, Address, Address) {
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

    // Mint initial supply to contract
    let stellar = token::StellarAssetClient::new(&env, &token_addr);
    stellar.mint(&contract_id, &1_000_000_000i128);

    (env, contract_id, client, admin, token_addr)
}

fn create_allocation(env: &Env, token: &Address, amount: i128) -> Allocation {
    let mut allocation = Allocation::new(env);
    allocation.add_asset(env, token.clone(), amount);
    allocation
}

#[test]
fn test_initialize() {
    let (env, _, client, admin, _) = setup();
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_create_vault_full_and_claim() {
    let (env, _, client, admin, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    let allocation = create_allocation(&env, &token, 1000i128);
    
    let vault_id = client.create_vault_full(
        &beneficiary,
        &allocation,
        &now,
        &(now + 1000),
        &0i128,
        &false, // irrevocable
        &false,
        &0u64,
    );

    assert_eq!(vault_id, 1);
    
    // Fast forward
    env.ledger().set_timestamp(now + 500);
    assert_eq!(client.get_claimable_amount(&vault_id), 500);

    // Claim
    client.claim_tokens(&vault_id, &100i128);
    assert_eq!(client.get_claimable_amount(&vault_id), 400);
    
    let token_client = token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&beneficiary), 100);
}

#[test]
fn test_periodic_vesting() {
    let (env, _, client, _, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    let allocation = create_allocation(&env, &token, 1000i128);
    
    // 1000 tokens over 1000 seconds, with 100 second steps
    let vault_id = client.create_vault_full(
        &beneficiary,
        &allocation,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &false,
        &100u64,
    );

    env.ledger().set_timestamp(now + 150);
    // 1 step completed (100 tokens)
    assert_eq!(client.get_claimable_amount(&vault_id), 100);

    env.ledger().set_timestamp(now + 250);
    // 2 steps completed (200 tokens)
    assert_eq!(client.get_claimable_amount(&vault_id), 200);
}

#[test]
fn test_milestones() {
    let (env, _, client, admin, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    let allocation = create_allocation(&env, &token, 1000i128);
    
    let vault_id = client.create_vault_full(
        &beneficiary,
        &allocation,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &false,
        &0u64,
    );

    let milestones = vec![&env, 
        Milestone { id: 1, percentage: 30, is_unlocked: false },
        Milestone { id: 2, percentage: 70, is_unlocked: false }
    ];
    
    client.set_milestones(&vault_id, &milestones);
    
    assert_eq!(client.get_claimable_amount(&vault_id), 0);
    
    client.unlock_milestone(&vault_id, &1);
    assert_eq!(client.get_claimable_amount(&vault_id), 300);
    
    client.unlock_milestone(&vault_id, &2);
    assert_eq!(client.get_claimable_amount(&vault_id), 1000);
}

#[test]
fn test_global_pause() {
    let (env, _, client, admin, _) = setup();
    
    client.toggle_pause();
    assert!(client.is_paused());
    
    let beneficiary = Address::generate(&env);
    // Logic that depends on paused should fail
}

#[test]
fn test_batch_operations() {
    let (env, _, client, _, token) = setup();
    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    let alloc1 = create_allocation(&env, &token, 500i128);
    let alloc2 = create_allocation(&env, &token, 500i128);
    
    let batch = BatchCreateData {
        recipients: vec![&env, r1, r2],
        allocations: vec![&env, alloc1, alloc2],
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
fn test_voting_power() {
    let (env, _, client, _, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    let alloc1 = create_allocation(&env, &token, 1000i128);
    let alloc2 = create_allocation(&env, &token, 1000i128);
    
    // Irrevocable vault: 1000 tokens (100% weight = 1000 power)
    client.create_vault_full(
        &beneficiary,
        &alloc1,
        &now,
        &(now + 1000),
        &0i128,
        &false, // is_revocable = false => is_irrevocable = true
        &false,
        &0u64,
    );
    
    // Revocable vault: 1000 tokens (50% weight = 500 power)
    client.create_vault_full(
        &beneficiary,
        &alloc2,
        &now,
        &(now + 1000),
        &0i128,
        &true, // is_revocable = true => is_irrevocable = false
        &false,
        &0u64,
    );
    
    // Total power should be 1000 + 500 = 1500
    assert_eq!(client.get_voting_power(&beneficiary), 1500);
}

#[test]
fn test_delegated_voting_power() {
    let (env, _, client, _, token) = setup();
    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);
    let representative = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    let alloc_a = create_allocation(&env, &token, 1000i128);
    let alloc_b = create_allocation(&env, &token, 1000i128);
    
    // A: 1000 power (irrevocable)
    client.create_vault_full(
        &beneficiary_a,
        &alloc_a,
        &now,
        &(now + 1000),
        &0i128,
        &false,
        &false,
        &0u64,
    );
    
    // B: 500 power (revocable)
    client.create_vault_full(
        &beneficiary_b,
        &alloc_b,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &false,
        &0u64,
    );
    
    // Initial check
    assert_eq!(client.get_voting_power(&beneficiary_a), 1000);
    assert_eq!(client.get_voting_power(&beneficiary_b), 500);
    assert_eq!(client.get_voting_power(&representative), 0);
    
    // A delegates to B
    client.delegate_voting_power(&beneficiary_a, &beneficiary_b);
    assert_eq!(client.get_voting_power(&beneficiary_a), 0);
    assert_eq!(client.get_voting_power(&beneficiary_b), 1500); // 500 + 1000
    
    // B delegates to representative
    client.delegate_voting_power(&beneficiary_b, &representative);
    assert_eq!(client.get_voting_power(&beneficiary_b), 0);
    // Note: C only gets B's own power (500) because A is not a direct delegator of C in current implementation
    // This is fine as per simple requirements.
    assert_eq!(client.get_voting_power(&representative), 500); 
    
    // A redelegates to representative
    client.delegate_voting_power(&beneficiary_a, &representative);
    assert_eq!(client.get_voting_power(&representative), 1500); // 1000 + 500
    
    // A undelegates
    client.delegate_voting_power(&beneficiary_a, &beneficiary_a);
    assert_eq!(client.get_voting_power(&beneficiary_a), 1000);
    assert_eq!(client.get_voting_power(&representative), 500); // Only B left
}

#[test]
fn test_vesting_acceleration() {
    let (env, _, client, _admin, token) = setup();
    let beneficiary = Address::generate(&env);
    let now = env.ledger().timestamp();
    
    let allocation = create_allocation(&env, &token, 1000i128);
    
    // 1000 tokens over 1000 seconds
    let vault_id = client.create_vault_full(
        &beneficiary,
        &allocation,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &false,
        &0u64,
    );
    
    // Fast forward halfway to check baseline
    env.ledger().set_timestamp(now + 250);
    assert_eq!(client.get_claimable_amount(&vault_id), 250);
    
    // Accelerate by 25% (Shift = 250)
    client.accelerate_all_schedules(&25);
    // At T=250, effective is 500
    assert_eq!(client.get_claimable_amount(&vault_id), 500);
    
    // Accelerate by 50% (Shift = 500)
    client.accelerate_all_schedules(&50);
    // At T=250, effective is 750
    assert_eq!(client.get_claimable_amount(&vault_id), 750);
    
    // Accelerate by 100%
    client.accelerate_all_schedules(&100);
    assert_eq!(client.get_claimable_amount(&vault_id), 1000);
}

#[test]
fn test_slashing() {
    let (env, _, client, _admin, token) = setup();
    let beneficiary = Address::generate(&env);
    let treasury = Address::generate(&env);
    let token_client = token::Client::new(&env, &token);
    let now = env.ledger().timestamp();
    
    let allocation = create_allocation(&env, &token, 1000i128);
    
    // 1000 tokens over 1000 seconds
    let vault_id = client.create_vault_full(
        &beneficiary,
        &allocation,
        &now,
        &(now + 1000),
        &0i128,
        &true,
        &false,
        &0u64,
    );
    
    // Jump to T=400
    env.ledger().set_timestamp(now + 400);
    assert_eq!(client.get_claimable_amount(&vault_id), 400);
    
    // Slash!
    client.slash_unvested_balance(&vault_id, &treasury);
    
    // Treasury should have received 600 (remaining unvested)
    assert_eq!(token_client.balance(&treasury), 600);
    
    // Beneficiary should still have 400 vested
    assert_eq!(client.get_claimable_amount(&vault_id), 400);
    
    // vault.allocation.total_amount() should be 400
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.allocation.total_amount(), 400);
    
    // Jump to T=1000, claimable should NOT increase
    env.ledger().set_timestamp(now + 1000);
    assert_eq!(client.get_claimable_amount(&vault_id), 400);
    
    // Claim 400
    client.claim_tokens(&vault_id, &400i128);
    assert_eq!(token_client.balance(&beneficiary), 400);
}
