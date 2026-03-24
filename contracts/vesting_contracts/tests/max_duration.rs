use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use vesting_contracts::{Allocation, BatchCreateData, VestingContract, VestingContractClient, MAX_DURATION};

fn setup(env: &Env) -> (VestingContractClient<'static>, Address) {
    env.mock_all_auths();

    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    client.initialize(&admin, &1_000_000i128);

    (client, admin)
}

fn create_allocation(env: &Env, amount: i128) -> Allocation {
    let dummy_asset = Address::generate(env);
    let mut allocation = Allocation::new(env);
    allocation.add_asset(env, dummy_asset, amount);
    allocation
}

#[test]
fn create_vault_full_allows_max_duration() {
    let env = Env::default();
    let (client, _admin) = setup(&env);

    let beneficiary = Address::generate(&env);
    let start = env.ledger().timestamp();
    let end = start + MAX_DURATION;

    client.create_vault_full(
        &beneficiary,
        &create_allocation(&env, 1_000i128),
        &start,
        &end,
        &0i128,
        &true,
        &false,
        &0u64,
    );
}

#[test]
#[should_panic(expected = "duration exceeds MAX_DURATION")]
fn create_vault_full_rejects_over_max_duration() {
    let env = Env::default();
    let (client, _admin) = setup(&env);

    let beneficiary = Address::generate(&env);
    let start = env.ledger().timestamp();
    let end = start + MAX_DURATION + 1;

    client.create_vault_full(
        &beneficiary,
        &create_allocation(&env, 1_000i128),
        &start,
        &end,
        &0i128,
        &true,
        &false,
        &0u64,
    );
}

#[test]
#[should_panic(expected = "duration exceeds MAX_DURATION")]
fn create_vault_lazy_rejects_over_max_duration() {
    let env = Env::default();
    let (client, _admin) = setup(&env);

    let beneficiary = Address::generate(&env);
    let start = env.ledger().timestamp();
    let end = start + MAX_DURATION + 1;

    client.create_vault_lazy(
        &beneficiary,
        &create_allocation(&env, 1_000i128),
        &start,
        &end,
        &0i128,
        &true,
        &false,
        &0u64,
    );
}

#[test]
#[should_panic(expected = "duration exceeds MAX_DURATION")]
fn batch_create_vaults_rejects_over_max_duration() {
    let env = Env::default();
    let (client, _admin) = setup(&env);

    let recipient = Address::generate(&env);
    let start = 100u64;
    let end = start + MAX_DURATION + 1;

    let batch = BatchCreateData {
        recipients: vec![&env, recipient],
        allocations: vec![&env, create_allocation(&env, 1_000i128)],
        start_times: vec![&env, start],
        end_times: vec![&env, end],
        keeper_fees: vec![&env, 0i128],
        step_durations: vec![&env, 0u64],
    };

    client.batch_create_vaults_lazy(&batch);
}
