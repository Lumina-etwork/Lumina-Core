#![cfg(test)]
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, String};
use super::validator::validate_poc;
use super::consensus::check_consensus;
use super::storage::get_poc_record;

#[test]
fn test_concurrent_validator_submission() {
    let env = Env::default();
    env.mock_all_auths();
    
    let node_id = String::from_str(&env, "node1");
    let peer_id = String::from_str(&env, "peer1");
    
    env.ledger().with_mut(|l| l.sequence = 100);

    for i in 1..=5 {
        let validator = Address::generate(&env);
        validate_poc(
            &env,
            validator.clone(),
            node_id.clone(),
            peer_id.clone(),
            100, // rtt
            1000, // bandwidth
            12345, // timestamp
        );
        // Simulate next validator submitting in the same ledger
    }
    
    let record = get_poc_record(&env, node_id.clone(), peer_id.clone());
    assert_eq!(record.attestations.len(), 5);
    
    let is_consensus = check_consensus(&env, node_id, peer_id);
    assert!(is_consensus);
}
