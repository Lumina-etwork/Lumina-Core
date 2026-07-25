use soroban_sdk::{contracttype, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigValue {
    pub value: String,
    pub schema_type: String,
    pub updated_at: u64,
    pub version: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigEntry {
    pub key: String,
    pub value: ConfigValue,
    pub description: String,
}

#[contracttype]
#[derive(Clone)]
pub struct ConfigSnapshot {
    pub keys: soroban_sdk::Vec<String>,
    pub values: soroban_sdk::Vec<ConfigValue>,
    pub snapshot_at: u64,
    pub total_entries: u32,
}
