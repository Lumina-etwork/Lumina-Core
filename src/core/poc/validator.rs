use soroban_sdk::{Address, Env, String};
use super::storage::{get_poc_record, set_poc_record, check_and_set_lock, ValidatorAttestation, MAX_RTT, MAX_VALIDATORS_PER_PEER};

pub fn validate_poc(
    env: &Env,
    validator: Address,
    node_id: String,
    peer_id: String,
    rtt_ms: u32,
    bandwidth_kbps: u32,
    timestamp: u64,
) {
    validator.require_auth();

    if rtt_ms > MAX_RTT {
        panic!("RTT exceeds maximum allowed");
    }

    let ledger_seq = env.ledger().sequence();
    if !check_and_set_lock(env, node_id.clone(), peer_id.clone(), validator.clone(), ledger_seq) {
        panic!("validator already attested in this ledger");
    }

    let mut record = get_poc_record(env, node_id, peer_id);
    
    // dedup check per validator
    for a in record.attestations.iter() {
        if a.validator == validator {
            panic!("validator already attested");
        }
    }

    if record.attestations.len() >= MAX_VALIDATORS_PER_PEER {
        panic!("max validators reached");
    }

    let attestation = ValidatorAttestation {
        validator,
        rtt_ms,
        bandwidth_kbps,
        timestamp,
    };
    
    record.attestations.push_back(attestation);

    set_poc_record(env, &record);
}
