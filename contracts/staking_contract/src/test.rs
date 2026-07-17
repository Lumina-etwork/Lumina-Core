use crate::{StakingContract, StakingContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (
    Env,
    Address,
    StakingContractClient<'static>,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(StakingContract, ());
    let client = StakingContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_addr = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    client.initialize(&admin, &token_addr);
    (env, contract_id, client, admin, token_addr)
}

// ---------------------------------------------------------------------------
// Initialisation
// ---------------------------------------------------------------------------

#[test]
fn test_initialize_sets_admin() {
    let (_, _, client, admin, _) = setup();
    assert_eq!(client.get_admin(), admin);
}

// ---------------------------------------------------------------------------
// stake_tokens
// ---------------------------------------------------------------------------

#[test]
fn test_stake_tokens_creates_record() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let vault_id = 1u64;

    client.stake_tokens(&beneficiary, &vault_id, &1000i128);

    let record = client.get_stake_record(&beneficiary, &vault_id);
    assert_eq!(record.amount, 1000);
    assert!(record.is_active);
    assert_eq!(record.pending_yield, 0);
}

#[test]
#[should_panic(expected = "AlreadyStaked")]
fn test_stake_tokens_double_stake_panics() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let vault_id = 1u64;

    client.stake_tokens(&beneficiary, &vault_id, &1000i128);
    // Second call should panic
    client.stake_tokens(&beneficiary, &vault_id, &500i128);
}

#[test]
#[should_panic(expected = "InsufficientBalance")]
fn test_stake_tokens_zero_amount_panics() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    client.stake_tokens(&beneficiary, &1u64, &0i128);
}

// ---------------------------------------------------------------------------
// unstake_tokens
// ---------------------------------------------------------------------------

#[test]
fn test_unstake_tokens_deactivates_record() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let vault_id = 1u64;

    client.stake_tokens(&beneficiary, &vault_id, &1000i128);
    client.unstake_tokens(&beneficiary, &vault_id);

    let record = client.get_stake_record(&beneficiary, &vault_id);
    assert!(!record.is_active);
}

#[test]
#[should_panic(expected = "NotStaked")]
fn test_unstake_tokens_not_staked_panics() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    client.unstake_tokens(&beneficiary, &1u64);
}

#[test]
#[should_panic(expected = "NotStaked")]
fn test_unstake_tokens_double_unstake_panics() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let vault_id = 1u64;

    client.stake_tokens(&beneficiary, &vault_id, &1000i128);
    client.unstake_tokens(&beneficiary, &vault_id);
    // Second unstake should panic
    client.unstake_tokens(&beneficiary, &vault_id);
}

// ---------------------------------------------------------------------------
// Yield
// ---------------------------------------------------------------------------

#[test]
fn test_accrue_and_get_yield() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let vault_id = 1u64;

    client.stake_tokens(&beneficiary, &vault_id, &1000i128);
    client.accrue_yield(&beneficiary, &vault_id, &50i128);

    assert_eq!(client.get_yield(&beneficiary, &vault_id), 50);
}

#[test]
fn test_claim_yield_for_resets_pending() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let vault_id = 1u64;

    client.stake_tokens(&beneficiary, &vault_id, &1000i128);
    client.accrue_yield(&beneficiary, &vault_id, &75i128);

    let claimed = client.claim_yield_for(&beneficiary, &vault_id);
    assert_eq!(claimed, 75);
    assert_eq!(client.get_yield(&beneficiary, &vault_id), 0);
}

#[test]
fn test_claim_yield_zero_when_none_accrued() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let vault_id = 1u64;

    client.stake_tokens(&beneficiary, &vault_id, &1000i128);
    let claimed = client.claim_yield_for(&beneficiary, &vault_id);
    assert_eq!(claimed, 0);
}

// ---------------------------------------------------------------------------
// Slashing
// ---------------------------------------------------------------------------

#[test]
fn test_slash_stake_reduces_amount() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let vault_id = 1u64;

    client.stake_tokens(&beneficiary, &vault_id, &1000i128);
    client.slash_stake(&beneficiary, &vault_id, &200i128);

    let record = client.get_stake_record(&beneficiary, &vault_id);
    assert_eq!(record.amount, 800);
}

#[test]
#[should_panic(expected = "SlashExceedsStake")]
fn test_slash_stake_exceeds_amount_panics() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let vault_id = 1u64;

    client.stake_tokens(&beneficiary, &vault_id, &1000i128);
    client.slash_stake(&beneficiary, &vault_id, &1001i128);
}

// ---------------------------------------------------------------------------
// Re-stake after unstake
// ---------------------------------------------------------------------------

#[test]
fn test_restake_after_unstake_succeeds() {
    let (env, _, client, _, _) = setup();
    let beneficiary = Address::generate(&env);
    let vault_id = 1u64;

    client.stake_tokens(&beneficiary, &vault_id, &1000i128);
    client.unstake_tokens(&beneficiary, &vault_id);
    // Re-stake should succeed
    client.stake_tokens(&beneficiary, &vault_id, &800i128);

    let record = client.get_stake_record(&beneficiary, &vault_id);
    assert_eq!(record.amount, 800);
    assert!(record.is_active);
}
