use super::storage::{get_poc_record, CONSENSUS_THRESHOLD};
use soroban_sdk::{Env, String};

pub fn check_consensus(env: &Env, node_id: String, peer_id: String) -> bool {
    let record = get_poc_record(env, node_id, peer_id);
    record.attestations.len() >= CONSENSUS_THRESHOLD
}
