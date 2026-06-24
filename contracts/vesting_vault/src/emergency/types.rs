use soroban_sdk::{contracttype, contractevent, Address, Vec, String};

#[contracttype]
pub struct EmergencyConfig {
    pub dao_members: Vec<Address>,
    pub cold_storage: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct AuditorPauseRequest {
    pub auditor: Address,
    pub timestamp: u64,
    pub reason: String,
}

#[contracttype]
#[derive(Clone)]
pub struct EmergencyPause {
    pub paused_by: Vec<Address>,
    pub paused_at: u64,
    pub expires_at: u64,
    pub reason: String,
    pub is_active: bool,
}

#[contractevent]
#[derive(Clone)]
pub struct EmergencyPauseTriggered {
    pub auditors: Vec<Address>,
    pub paused_at: u64,
    pub expires_at: u64,
    pub reason: String,
}

#[contractevent]
#[derive(Clone)]
#[allow(dead_code)]
pub struct EmergencyPauseLifted {
    pub lifted_at: u64,
}
