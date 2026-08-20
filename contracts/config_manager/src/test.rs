#![allow(deprecated)] // reason: soroban_sdk update requires macro rewrite which is behavior-risky

extern crate std;

use crate::{ConfigManager, ConfigManagerClient};
use soroban_sdk::{testutils::Address as _, vec, Address, Env, String};

fn setup_test() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, ConfigManager);
    ConfigManagerClient::new(&env, &contract_id).initialize(&admin);
    (env, contract_id, admin)
}

fn s(e: &Env, v: &str) -> String {
    String::from_str(e, v)
}

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, ConfigManager);
    let client = ConfigManagerClient::new(&env, &contract_id);

    client.initialize(&admin);
    assert_eq!(client.get_version_number(), 1);
}

#[test]
fn test_initialize_rejects_double_init() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, ConfigManager);
    let client = ConfigManagerClient::new(&env, &contract_id);

    client.initialize(&admin);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.initialize(&admin);
    }));
    assert!(result.is_err());
}

#[test]
fn test_set_and_get_config() {
    let (env, contract_id, admin) = setup_test();
    let client = ConfigManagerClient::new(&env, &contract_id);

    let version = client.set_config(
        &admin,
        &s(&env, "max_retries"),
        &s(&env, "5"),
        &s(&env, "u64"),
        &s(&env, "Maximum retry attempts"),
    );

    assert!(version > 0);

    let result = client.get_config(&s(&env, "max_retries"));
    assert!(result.is_some());
    let cv = result.unwrap();
    assert_eq!(cv.value, s(&env, "5"));
    assert_eq!(cv.schema_type, s(&env, "u64"));
}

#[test]
fn test_get_config_string() {
    let (env, contract_id, admin) = setup_test();
    let client = ConfigManagerClient::new(&env, &contract_id);

    client.set_config(
        &admin,
        &s(&env, "api_endpoint"),
        &s(&env, "https://api.example.com"),
        &s(&env, "string"),
        &s(&env, "API endpoint URL"),
    );

    let val = client.get_config_string(&s(&env, "api_endpoint"));
    assert_eq!(val, Some(s(&env, "https://api.example.com")));
}

#[test]
fn test_has_config() {
    let (env, contract_id, admin) = setup_test();
    let client = ConfigManagerClient::new(&env, &contract_id);

    assert!(!client.has_config(&s(&env, "nonexistent")));

    client.set_config(
        &admin,
        &s(&env, "exists"),
        &s(&env, "yes"),
        &s(&env, "string"),
        &s(&env, ""),
    );

    assert!(client.has_config(&s(&env, "exists")));
}

#[test]
fn test_remove_config() {
    let (env, contract_id, admin) = setup_test();
    let client = ConfigManagerClient::new(&env, &contract_id);

    client.set_config(
        &admin,
        &s(&env, "temp"),
        &s(&env, "value"),
        &s(&env, "string"),
        &s(&env, ""),
    );
    assert!(client.has_config(&s(&env, "temp")));

    client.remove_config(&admin, &s(&env, "temp"));
    assert!(!client.has_config(&s(&env, "temp")));
}

#[test]
fn test_get_all_configs() {
    let (env, contract_id, admin) = setup_test();
    let client = ConfigManagerClient::new(&env, &contract_id);

    let entries = client.get_all_configs();
    assert_eq!(entries.len(), 0);

    client.set_config(
        &admin,
        &s(&env, "a"),
        &s(&env, "1"),
        &s(&env, "u64"),
        &s(&env, "first"),
    );
    client.set_config(
        &admin,
        &s(&env, "b"),
        &s(&env, "2"),
        &s(&env, "u64"),
        &s(&env, "second"),
    );

    let entries = client.get_all_configs();
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_get_config_count() {
    let (env, contract_id, admin) = setup_test();
    let client = ConfigManagerClient::new(&env, &contract_id);

    assert_eq!(client.get_config_count(), 0);

    client.set_config(
        &admin,
        &s(&env, "x"),
        &s(&env, "10"),
        &s(&env, "u64"),
        &s(&env, ""),
    );
    assert_eq!(client.get_config_count(), 1);
}

#[test]
fn test_version_increments() {
    let (env, contract_id, admin) = setup_test();
    let client = ConfigManagerClient::new(&env, &contract_id);

    let v1 = client.get_version_number();
    client.set_config(
        &admin,
        &s(&env, "k"),
        &s(&env, "v"),
        &s(&env, "string"),
        &s(&env, ""),
    );
    let v2 = client.get_version_number();
    assert_eq!(v2, v1 + 1);

    client.set_config(
        &admin,
        &s(&env, "k2"),
        &s(&env, "v2"),
        &s(&env, "string"),
        &s(&env, ""),
    );
    let v3 = client.get_version_number();
    assert_eq!(v3, v2 + 1);
}

#[test]
fn test_validate_config_value() {
    let (env, contract_id, admin) = setup_test();
    let client = ConfigManagerClient::new(&env, &contract_id);

    client.set_config(
        &admin,
        &s(&env, "retries"),
        &s(&env, "3"),
        &s(&env, "u64"),
        &s(&env, ""),
    );

    assert!(client.validate_config_value(&s(&env, "retries"), &s(&env, "5")));
    assert!(!client.validate_config_value(&s(&env, "retries"), &s(&env, "not_a_number")));
}

#[test]
fn test_batch_import() {
    let (env, contract_id, admin) = setup_test();
    let client = ConfigManagerClient::new(&env, &contract_id);

    let keys = vec![&env, s(&env, "k1"), s(&env, "k2"), s(&env, "k3")];
    let values = vec![&env, s(&env, "v1"), s(&env, "v2"), s(&env, "v3")];
    let schema_types = vec![
        &env,
        s(&env, "string"),
        s(&env, "string"),
        s(&env, "string"),
    ];
    let descriptions = vec![&env, s(&env, "desc1"), s(&env, "desc2"), s(&env, "desc3")];

    let version = client.batch_import_configs(&admin, &keys, &values, &schema_types, &descriptions);
    assert_eq!(client.get_config_count(), 3);
    assert!(version > 0);
}

#[test]
fn test_get_snapshot() {
    let (env, contract_id, admin) = setup_test();
    let client = ConfigManagerClient::new(&env, &contract_id);

    client.set_config(
        &admin,
        &s(&env, "alpha"),
        &s(&env, "first"),
        &s(&env, "string"),
        &s(&env, ""),
    );

    let snapshot = client.get_snapshot();
    assert_eq!(snapshot.total_entries, 1);
    assert_eq!(snapshot.keys.len(), 1);
    assert_eq!(snapshot.values.len(), 1);
}
