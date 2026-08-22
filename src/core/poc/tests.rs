#![cfg(test)]
use super::consensus::check_consensus;
use super::storage::get_poc_record;
use super::validator::validate_poc;
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger, MockAuth},
    Address, Env, String,
};

#[contract]
struct PocMockContract;

#[contractimpl]
impl PocMockContract {
    pub fn validate(
        env: Env,
        validator: Address,
        node_id: String,
        peer_id: String,
        rtt_ms: u32,
        bandwidth_kbps: u32,
        timestamp: u64,
    ) {
        validate_poc(&env, validator, node_id, peer_id, rtt_ms, bandwidth_kbps, timestamp)
    }

    pub fn check(env: Env, node_id: String, peer_id: String) -> bool {
        check_consensus(&env, node_id, peer_id)
    }

    pub fn attest_count(env: Env, node_id: String, peer_id: String) -> u32 {
        let record = get_poc_record(&env, node_id, peer_id);
        record.attestations.len()
    }
}

#[test]
fn test_concurrent_validator_submission() {
    let env = Env::default();
    env.mock_all_auths();

    let node_id = String::from_str(&env, "node1");
    let peer_id = String::from_str(&env, "peer1");

    env.ledger().with_mut(|l| l.sequence_number = 100);

    let contract_id = env.register_contract(None, PocMockContract);
    let client = PocMockContractClient::new(&env, &contract_id);

    for _ in 1..=5 {
        let validator = Address::generate(&env);
        client.validate(
            &validator,
            &node_id,
            &peer_id,
            &100,   // rtt
            &1000,  // bandwidth
            &12345, // timestamp
        );
    }

    let count = client.attest_count(&node_id, &peer_id);
    assert_eq!(count, 5);

    let is_consensus = client.check(&node_id, &peer_id);
    assert!(is_consensus);
}
