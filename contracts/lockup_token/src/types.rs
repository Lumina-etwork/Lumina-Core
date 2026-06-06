use soroban_sdk::{contracttype, contractevent, Address};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockupConfig {
    pub vesting_id: u32,
    pub lockup_duration_seconds: u64,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockupInfo {
    pub vesting_id: u32,
    pub amount: i128,
    pub locked_at: u64,
    pub unlock_time: u64,
    pub is_unwrapped: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnwrapEvent {
    pub user: Address,
    pub vesting_id: u32,
    pub amount: i128,
    pub timestamp: u64,
}

// Events
#[event]
#[derive(Clone, Debug)]
pub struct Initialized {
    #[topic]
    pub admin: Address,
    #[topic]
    pub underlying_token: Address,
    pub timestamp: u64,
}

#[event]
#[derive(Clone, Debug)]
pub struct LockupConfigured {
    #[topic]
    pub vesting_id: u32,
    pub lockup_duration_seconds: u64,
    pub timestamp: u64,
}

#[event]
#[derive(Clone, Debug)]
pub struct WrappedTokensIssued {
    #[topic]
    pub to: Address,
    #[topic]
    pub vesting_id: u32,
    pub amount: i128,
    pub unlock_time: u64,
    pub timestamp: u64,
}

#[event]
#[derive(Clone, Debug)]
pub struct TokensUnwrapped {
    #[topic]
    pub user: Address,
    #[topic]
    pub vesting_id: u32,
    pub amount: i128,
    pub timestamp: u64,
}

#[event]
#[derive(Clone, Debug)]
pub struct AuthorizedMinterAdded {
    #[topic]
    pub minter: Address,
    pub timestamp: u64,
}

#[event]
#[derive(Clone, Debug)]
pub struct AuthorizedMinterRemoved {
    #[topic]
    pub minter: Address,
    pub timestamp: u64,
}
