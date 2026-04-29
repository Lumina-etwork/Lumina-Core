use soroban_sdk::{contracttype, contractevent, Address, Vec, Map, String, BytesN};

#[contracttype]
#[derive(Clone)]
pub struct BailoutRequest {
    pub id: u64,
    pub beneficiary: Address,
    pub asset: Address,
    pub amount: i128,
    pub requested_at: u64,
    pub signatures: Vec<Address>,
    pub executed: bool,
}

#[contractevent]
pub struct InsuranceFundCapitalized {
    #[topic]
    pub asset: Address,
    pub amount: i128,
    pub total_balance: i128,
}

#[contractevent]
pub struct BailoutRequested {
    #[topic]
    pub request_id: u64,
    #[topic]
    pub beneficiary: Address,
    #[topic]
    pub asset: Address,
    pub amount: i128,
    pub requested_at: u64,
}

#[contractevent]
pub struct BailoutExecuted {
    #[topic]
    pub request_id: u64,
    #[topic]
    pub beneficiary: Address,
    #[topic]
    pub asset: Address,
    pub amount: i128,
    pub executed_at: u64,
}