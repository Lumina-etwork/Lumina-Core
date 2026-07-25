use crate::types::*;
use soroban_sdk::{contracttype, symbol_short, Env, String, Symbol, Vec};

const ALL_KEYS: Symbol = symbol_short!("all_keys");

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    ConfigSchema(String),
    ConfigValue(String),
    ConfigVersion,
    IsInitialized,
}

pub fn set_admin(e: &Env, admin: &soroban_sdk::Address) {
    e.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(e: &Env) -> soroban_sdk::Address {
    e.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn require_admin(e: &Env, admin: &soroban_sdk::Address) {
    if admin != &get_admin(e) {
        panic!("Unauthorized");
    }
}

pub fn set_config_value(e: &Env, key: &String, value: &ConfigValue) {
    e.storage()
        .instance()
        .set(&DataKey::ConfigValue(key.clone()), value);
}

pub fn get_config_value(e: &Env, key: &String) -> Option<ConfigValue> {
    e.storage()
        .instance()
        .get(&DataKey::ConfigValue(key.clone()))
}

pub fn has_config(e: &Env, key: &String) -> bool {
    e.storage()
        .instance()
        .has(&DataKey::ConfigValue(key.clone()))
}

pub fn remove_config_value(e: &Env, key: &String) {
    e.storage()
        .instance()
        .remove(&DataKey::ConfigValue(key.clone()));
}

pub fn set_config_schema(e: &Env, key: &String, schema: &String) {
    e.storage()
        .instance()
        .set(&DataKey::ConfigSchema(key.clone()), schema);
}

pub fn get_config_schema(e: &Env, key: &String) -> Option<String> {
    e.storage()
        .instance()
        .get(&DataKey::ConfigSchema(key.clone()))
}

pub fn remove_config_schema(e: &Env, key: &String) {
    e.storage()
        .instance()
        .remove(&DataKey::ConfigSchema(key.clone()));
}

pub fn set_version(e: &Env, version: u32) {
    e.storage().instance().set(&DataKey::ConfigVersion, &version);
}

pub fn get_version(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::ConfigVersion)
        .unwrap_or(0)
}

pub fn set_initialized(e: &Env) {
    e.storage()
        .instance()
        .set(&DataKey::IsInitialized, &true);
}

pub fn is_initialized(e: &Env) -> bool {
    e.storage()
        .instance()
        .get(&DataKey::IsInitialized)
        .unwrap_or(false)
}

pub fn add_key_to_index(e: &Env, key: &String) {
    let mut keys: Vec<String> = e
        .storage()
        .instance()
        .get(&ALL_KEYS)
        .unwrap_or(Vec::new(e));
    if !keys.contains(key) {
        keys.push_back(key.clone());
        e.storage().instance().set(&ALL_KEYS, &keys);
    }
}

pub fn remove_key_from_index(e: &Env, key: &String) {
    let keys: Vec<String> = e
        .storage()
        .instance()
        .get(&ALL_KEYS)
        .unwrap_or(Vec::new(e));
    let mut filtered: Vec<String> = Vec::new(e);
    for i in 0..keys.len() {
        let k = keys.get(i).unwrap();
        if &k != key {
            filtered.push_back(k);
        }
    }
    e.storage().instance().set(&ALL_KEYS, &filtered);
}

pub fn get_all_config_keys(e: &Env) -> Vec<String> {
    e.storage()
        .instance()
        .get(&ALL_KEYS)
        .unwrap_or(Vec::new(e))
}
