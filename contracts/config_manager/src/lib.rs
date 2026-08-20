#![no_std]

use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, Address, Env, String, Vec,
};

mod errors;
mod storage;
mod types;

use errors::Error;
use storage::*;
pub use types::*;

const CONFIG_SYMBOL: soroban_sdk::Symbol = symbol_short!("config");

#[contract]
pub struct ConfigManager;

#[contractimpl]
impl ConfigManager {
    pub fn initialize(e: Env, admin: Address) {
        if is_initialized(&e) {
            panic_with_error!(&e, Error::AlreadyInitialized);
        }
        admin.require_auth();
        set_admin(&e, &admin);
        set_initialized(&e);
        set_version(&e, 1);
    }

    pub fn set_config(
        e: Env,
        admin: Address,
        key: String,
        value: String,
        schema_type: String,
        _description: String,
    ) -> u32 {
        require_admin(&e, &admin);
        admin.require_auth();

        let current_version = get_version(&e);
        let new_version = current_version + 1;
        let now = e.ledger().timestamp();

        let config_value = ConfigValue {
            value: value.clone(),
            schema_type: schema_type.clone(),
            updated_at: now,
            version: new_version,
        };

        let is_new = !has_config(&e, &key);
        set_config_value(&e, &key, &config_value);
        set_config_schema(&e, &key, &schema_type);

        if is_new {
            add_key_to_index(&e, &key);
        }

        set_version(&e, new_version);

        #[allow(deprecated)] // reason: migrating to #[contractevent] is behavior-risky
        e.events().publish(
            (CONFIG_SYMBOL, symbol_short!("updated")),
            (key, value, new_version, now),
        );

        new_version
    }

    pub fn get_config(e: Env, key: String) -> Option<ConfigValue> {
        get_config_value(&e, &key)
    }

    pub fn get_config_string(e: Env, key: String) -> Option<String> {
        get_config_value(&e, &key).map(|cv| cv.value)
    }

    pub fn has_config(e: Env, key: String) -> bool {
        has_config(&e, &key)
    }

    pub fn remove_config(e: Env, admin: Address, key: String) {
        require_admin(&e, &admin);
        admin.require_auth();

        if !has_config(&e, &key) {
            panic_with_error!(&e, Error::ConfigNotFound);
        }

        let current_version = get_version(&e);
        let new_version = current_version + 1;

        remove_config_value(&e, &key);
        remove_config_schema(&e, &key);
        remove_key_from_index(&e, &key);
        set_version(&e, new_version);

        #[allow(deprecated)] // reason: migrating to #[contractevent] is behavior-risky
        e.events().publish(
            (CONFIG_SYMBOL, symbol_short!("deleted")),
            (key, new_version),
        );
    }

    pub fn get_all_configs(e: Env) -> Vec<ConfigEntry> {
        let keys = get_all_config_keys(&e);
        let mut entries: Vec<ConfigEntry> = Vec::new(&e);

        for i in 0..keys.len() {
            let key = keys.get(i).unwrap();
            if let Some(value) = get_config_value(&e, &key) {
                let desc = get_config_schema(&e, &key).unwrap_or(String::from_str(&e, ""));
                entries.push_back(ConfigEntry {
                    key,
                    value,
                    description: desc,
                });
            }
        }

        entries
    }

    pub fn get_config_count(e: Env) -> u32 {
        get_all_config_keys(&e).len()
    }

    pub fn get_version_number(e: Env) -> u32 {
        get_version(&e)
    }

    pub fn validate_config_value(e: Env, key: String, value: String) -> bool {
        let schema = get_config_schema(&e, &key);

        let u64_type = String::from_str(&e, "u64");
        let i128_type = String::from_str(&e, "i128");
        let bool_type = String::from_str(&e, "bool");

        match schema {
            Some(schema_str) => {
                if schema_str == u64_type {
                    let v = value.to_bytes();
                    if v.is_empty() {
                        return false;
                    }
                    for i in 0..v.len() {
                        let byte = v.get(i).unwrap();
                        if !(48..=57).contains(&byte) {
                            return false;
                        }
                    }
                    true
                } else if schema_str == i128_type {
                    let v = value.to_bytes();
                    if v.is_empty() {
                        return false;
                    }
                    for i in 0..v.len() {
                        let byte = v.get(i).unwrap();
                        if i == 0 && byte == 45 {
                            continue;
                        }
                        if !(48..=57).contains(&byte) {
                            return false;
                        }
                    }
                    true
                } else if schema_str == bool_type {
                    let _v = value.to_bytes();
                    let t = String::from_str(&e, "true");
                    let f = String::from_str(&e, "false");
                    let one = String::from_str(&e, "1");
                    let zero = String::from_str(&e, "0");
                    value == t || value == f || value == one || value == zero
                } else {
                    true
                }
            }
            None => true,
        }
    }

    pub fn batch_import_configs(
        e: Env,
        admin: Address,
        keys: Vec<String>,
        values: Vec<String>,
        schema_types: Vec<String>,
        _descriptions: Vec<String>,
    ) -> u32 {
        require_admin(&e, &admin);
        admin.require_auth();

        let count = keys.len();
        let mut last_version = get_version(&e);

        for i in 0..count {
            let key = keys.get(i).unwrap();
            let value = if i < values.len() {
                values.get(i).unwrap()
            } else {
                String::from_str(&e, "")
            };
            let schema = if i < schema_types.len() {
                schema_types.get(i).unwrap()
            } else {
                String::from_str(&e, "string")
            };
            let new_version = last_version + 1;
            let now = e.ledger().timestamp();

            let config_value = ConfigValue {
                value,
                schema_type: schema.clone(),
                updated_at: now,
                version: new_version,
            };

            let is_new = !has_config(&e, &key);
            set_config_value(&e, &key, &config_value);
            set_config_schema(&e, &key, &schema);

            if is_new {
                add_key_to_index(&e, &key);
            }

            last_version = new_version;
        }

        set_version(&e, last_version);

        #[allow(deprecated)] // reason: migrating to #[contractevent] is behavior-risky
        e.events().publish(
            (CONFIG_SYMBOL, symbol_short!("imported")),
            (count, last_version),
        );

        last_version
    }

    pub fn get_snapshot(e: Env) -> ConfigSnapshot {
        let keys = get_all_config_keys(&e);
        let mut values: Vec<ConfigValue> = Vec::new(&e);
        let mut key_strings: Vec<String> = Vec::new(&e);

        for i in 0..keys.len() {
            let key = keys.get(i).unwrap();
            key_strings.push_back(key.clone());
            if let Some(value) = get_config_value(&e, &key) {
                values.push_back(value);
            }
        }

        ConfigSnapshot {
            keys: key_strings,
            values,
            snapshot_at: e.ledger().timestamp(),
            total_entries: keys.len(),
        }
    }
}

#[cfg(test)]
mod test;
