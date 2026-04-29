#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, vec, Address, Env, IntoVal, Symbol,
    Val, Vec,
};
use vesting_contracts::{kpi_engine, Error, VestingContract, VestingContractClient};

const ATTACK_CLAIM_TOKENS: u32 = 1;
const ATTACK_CLAIM_AND_SWAP: u32 = 2;

#[contracttype]
#[derive(Clone)]
struct CallbackAttack {
    target: Address,
    vault_id: u64,
    claim_amount: i128,
    min_destination_amount: i128,
    kind: u32,
    armed: bool,
    entered: bool,
    blocked: bool,
    callback_count: u32,
}

fn attack_key() -> Symbol {
    symbol_short!("attack")
}

fn read_attack(env: &Env) -> Option<CallbackAttack> {
    env.storage().instance().get(&attack_key())
}

fn write_attack(env: &Env, attack: &CallbackAttack) {
    env.storage().instance().set(&attack_key(), attack);
}

fn execute_attack(env: &Env) {
    let Some(mut attack) = read_attack(env) else {
        return;
    };

    if !attack.armed || attack.entered {
        return;
    }

    attack.entered = true;
    attack.callback_count += 1;
    write_attack(env, &attack);

    let result = match attack.kind {
        ATTACK_CLAIM_TOKENS => env.try_invoke_contract::<Val, Error>(
            &attack.target,
            &Symbol::new(env, "claim_tokens"),
            (attack.vault_id, attack.claim_amount).into_val(env),
        ),
        ATTACK_CLAIM_AND_SWAP => env.try_invoke_contract::<Val, Error>(
            &attack.target,
            &Symbol::new(env, "claim_and_swap"),
            (attack.vault_id, Some(attack.min_destination_amount)).into_val(env),
        ),
        _ => panic!("unsupported attack kind"),
    };

    attack.blocked = result.is_err();
    write_attack(env, &attack);
}

#[contract]
struct MaliciousToken;

#[contractimpl]
impl MaliciousToken {
    pub fn configure_claim(env: Env, target: Address, vault_id: u64, claim_amount: i128) {
        write_attack(
            &env,
            &CallbackAttack {
                target,
                vault_id,
                claim_amount,
                min_destination_amount: 0,
                kind: ATTACK_CLAIM_TOKENS,
                armed: true,
                entered: false,
                blocked: false,
                callback_count: 0,
            },
        );
    }

    pub fn configure_swap(env: Env, target: Address, vault_id: u64, min_destination_amount: i128) {
        write_attack(
            &env,
            &CallbackAttack {
                target,
                vault_id,
                claim_amount: 0,
                min_destination_amount,
                kind: ATTACK_CLAIM_AND_SWAP,
                armed: true,
                entered: false,
                blocked: false,
                callback_count: 0,
            },
        );
    }

    pub fn transfer(env: Env, _from: Address, _to: Address, _amount: i128) {
        execute_attack(&env);
    }

    pub fn balance(_env: Env, _id: Address) -> i128 {
        0
    }

    pub fn attack_blocked(env: Env) -> bool {
        read_attack(&env)
            .map(|attack| attack.blocked)
            .unwrap_or(false)
    }

    pub fn callback_count(env: Env) -> u32 {
        read_attack(&env)
            .map(|attack| attack.callback_count)
            .unwrap_or(0)
    }
}

#[contract]
struct MaliciousStaking;

#[contractimpl]
impl MaliciousStaking {
    pub fn configure_claim(env: Env, target: Address, vault_id: u64, claim_amount: i128) {
        write_attack(
            &env,
            &CallbackAttack {
                target,
                vault_id,
                claim_amount,
                min_destination_amount: 0,
                kind: ATTACK_CLAIM_TOKENS,
                armed: true,
                entered: false,
                blocked: false,
                callback_count: 0,
            },
        );
    }

    pub fn stake_tokens(_env: Env, _beneficiary: Address, _vault_id: u64, _amount: i128) {}

    pub fn unstake_tokens(_env: Env, _beneficiary: Address, _vault_id: u64) {}

    pub fn claim_yield_for(env: Env, _beneficiary: Address, _vault_id: u64) -> i128 {
        execute_attack(&env);
        125
    }

    pub fn attack_blocked(env: Env) -> bool {
        read_attack(&env)
            .map(|attack| attack.blocked)
            .unwrap_or(false)
    }
}

#[contract]
struct MaliciousDexHop;

#[contractimpl]
impl MaliciousDexHop {
    pub fn configure_swap(env: Env, target: Address, vault_id: u64, min_destination_amount: i128) {
        write_attack(
            &env,
            &CallbackAttack {
                target,
                vault_id,
                claim_amount: 0,
                min_destination_amount,
                kind: ATTACK_CLAIM_AND_SWAP,
                armed: true,
                entered: false,
                blocked: false,
                callback_count: 0,
            },
        );
    }

    pub fn path_payment_hop(
        env: Env,
        _source_contract: Address,
        _vault_id: u64,
        _source_amount: i128,
        _destination_asset: Address,
    ) {
        execute_attack(&env);
    }

    pub fn attack_blocked(env: Env) -> bool {
        read_attack(&env)
            .map(|attack| attack.blocked)
            .unwrap_or(false)
    }

    pub fn callback_count(env: Env) -> u32 {
        read_attack(&env)
            .map(|attack| attack.callback_count)
            .unwrap_or(0)
    }
}

fn setup_standard_vault(env: &Env) -> (Address, VestingContractClient<'_>, u64, Address, Address) {
    let admin = Address::generate(env);
    let token_admin = Address::generate(env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    let asset_admin = token::StellarAssetClient::new(env, &token_address);
    asset_admin.mint(&admin, &5_000);

    let contract_id = env.register_contract(None, VestingContract);
    let client = VestingContractClient::new(env, &contract_id);
    client.initialize(&admin, &1_000_000);
    client.set_token(&token_address);

    let beneficiary = Address::generate(env);
    let now = env.ledger().timestamp();
    let vault_id = client.create_vault_full(
        &beneficiary,
        &1_000i128,
        &now,
        &(now + 100),
        &0i128,
        &true,
        &true,
        &0u64,
    );

    env.ledger().set_timestamp(now + 101);
    (contract_id, client, vault_id, beneficiary, token_address)
}

fn setup_malicious_token_vault(
    env: &Env,
    token_address: &Address,
) -> (Address, VestingContractClient<'_>, u64) {
    let admin = Address::generate(env);
    let contract_id = env.register_contract(None, VestingContract);
    let client = VestingContractClient::new(env, &contract_id);
    client.initialize(&admin, &1_000_000);
    client.set_token(token_address);

    let beneficiary = Address::generate(env);
    let now = env.ledger().timestamp();
    let vault_id = client.create_vault_full(
        &beneficiary,
        &1_000i128,
        &now,
        &(now + 100),
        &0i128,
        &true,
        &true,
        &0u64,
    );

    env.ledger().set_timestamp(now + 101);
    (contract_id, client, vault_id)
}

fn mark_kpi_met(env: &Env, contract_id: &Address, vault_id: u64) {
    env.as_contract(contract_id, || {
        kpi_engine::set_kpi_met(env, vault_id, true);
    });
}

#[test]
fn formal_reentrancy_bmc_blocks_recursive_token_transfer_claims() {
    let env = Env::default();
    env.mock_all_auths();

    let malicious_token_id = env.register_contract(None, MaliciousToken);
    let malicious_token = MaliciousTokenClient::new(&env, &malicious_token_id);
    let (contract_id, client, vault_id) = setup_malicious_token_vault(&env, &malicious_token_id);

    malicious_token.configure_claim(&contract_id, &vault_id, &1i128);
    client.claim_tokens(&vault_id, &400i128);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.allocations.get(0).unwrap().released_amount, 400);
    assert!(malicious_token.attack_blocked());
    assert_eq!(malicious_token.callback_count(), 1);
}

#[test]
fn formal_reentrancy_bmc_blocks_recursive_staking_callbacks() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, client, vault_id, _beneficiary, _token_address) = setup_standard_vault(&env);
    let staking_id = env.register_contract(None, MaliciousStaking);
    let staking = MaliciousStakingClient::new(&env, &staking_id);

    client.add_staking_contract(&staking_id);
    client.auto_stake(&vault_id, &staking_id);

    staking.configure_claim(&contract_id, &vault_id, &1i128);
    client.claim_tokens(&vault_id, &500i128);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.allocations.get(0).unwrap().released_amount, 500);
    assert!(staking.attack_blocked());
}

#[test]
fn formal_reentrancy_bmc_covers_multi_hop_path_payment_callbacks() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, client, vault_id, _beneficiary, _token_address) = setup_standard_vault(&env);
    let destination_token_id = env.register_contract(None, MaliciousToken);
    let destination_token = MaliciousTokenClient::new(&env, &destination_token_id);
    let hop_a_id = env.register_contract(None, MaliciousDexHop);
    let hop_b_id = env.register_contract(None, MaliciousDexHop);
    let hop_a = MaliciousDexHopClient::new(&env, &hop_a_id);
    let hop_b = MaliciousDexHopClient::new(&env, &hop_b_id);

    let path: Vec<Address> = vec![&env, hop_a_id.clone(), hop_b_id.clone()];
    client.configure_path_payment(
        &Address::generate(&env),
        &destination_token_id,
        &900i128,
        &path,
    );
    mark_kpi_met(&env, &contract_id, vault_id);

    hop_a.configure_swap(&contract_id, &vault_id, &900i128);
    hop_b.configure_swap(&contract_id, &vault_id, &900i128);
    destination_token.configure_swap(&contract_id, &vault_id, &900i128);

    let event = client.claim_and_swap(&vault_id, &Some(900i128));
    assert_eq!(event.source_amount, 1_000);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.allocations.get(0).unwrap().released_amount, 1_000);
    assert!(hop_a.attack_blocked());
    assert!(hop_b.attack_blocked());
    assert!(destination_token.attack_blocked());
    assert_eq!(hop_a.callback_count(), 1);
    assert_eq!(hop_b.callback_count(), 1);
    assert_eq!(destination_token.callback_count(), 1);
    assert_eq!(client.get_path_payment_claim_history().len(), 1);
}
