use soroban_sdk::{contracttype, Address, Env, String, Vec};

pub const MAX_RTT: u32 = 500;
pub const CONSENSUS_THRESHOLD: u32 = 3;
pub const MAX_VALIDATORS_PER_PEER: u32 = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct ValidatorAttestation {
    pub validator: Address,
    pub rtt_ms: u32,
    pub bandwidth_kbps: u32,
    pub timestamp: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct PocRecord {
    pub node_id: String,
    pub peer_id: String,
    pub attestations: Vec<ValidatorAttestation>,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    PocRecord(String, String),
    PocLock(String, String, Address, u32),
}

pub fn get_poc_record(env: &Env, node_id: String, peer_id: String) -> PocRecord {
    let key = DataKey::PocRecord(node_id.clone(), peer_id.clone());
    env.storage().persistent().get(&key).unwrap_or(PocRecord {
        node_id,
        peer_id,
        attestations: Vec::new(env),
    })
}

pub fn set_poc_record(env: &Env, record: &PocRecord) {
    let key = DataKey::PocRecord(record.node_id.clone(), record.peer_id.clone());
    env.storage().persistent().set(&key, record);
}

pub fn check_and_set_lock(
    env: &Env,
    node_id: String,
    peer_id: String,
    validator: Address,
    ledger_seq: u32,
) -> bool {
    let key = DataKey::PocLock(node_id, peer_id, validator, ledger_seq);
    if env.storage().temporary().has(&key) {
        return false;
    }
    env.storage().temporary().set(&key, &true);
    true
}
