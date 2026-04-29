//! Integration tests for Protocol Sunset and State Migration Hooks (Issue #280)
//!
//! This test suite verifies the secure migration pathway for transitioning
//! from V2 to V3 architecture, ensuring no funds are trapped during major upgrades.

use soroban_sdk::{contract, contractimpl, Env, Address, BytesN, Vec, IntoVal, testutils::Address as _};
use crate::{VestingVault, VestingVaultClient};

#[test]
fn test_protocol_sunset_initiation() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let migration_target = Address::generate(&env);

    let contract_id = env.register_contract(None, VestingVault);
    let client = VestingVaultClient::new(&env, &contract_id);

    // Test: Initiate protocol sunset
    client.prepare_protocol_sunset(&admin, &migration_target);

    let sunset_status = client.get_protocol_sunset_status();
    assert!(sunset_status.is_some());
    let sunset = sunset_status.unwrap();
    assert!(sunset.is_initiated);
    assert!(!sunset.is_aborted);
    assert!(sunset.new_schedules_halted);
    assert_eq!(sunset.migration_target, migration_target);
}

#[test]
fn test_sunset_abort_before_timelock() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let migration_target = Address::generate(&env);

    let contract_id = env.register_contract(None, VestingVault);
    let client = VestingVaultClient::new(&env, &contract_id);

    // Initiate sunset
    client.prepare_protocol_sunset(&admin, &migration_target);

    // Abort before timelock expires
    client.abort_protocol_sunset(&admin);

    let sunset_status = client.get_protocol_sunset_status();
    assert!(sunset_status.is_some());
    let sunset = sunset_status.unwrap();
    assert!(sunset.is_aborted);
    assert!(!sunset.new_schedules_halted);
}

#[test]
fn test_state_payload_export() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let migration_target = Address::generate(&env);
    let vesting_id = 1u32;

    let contract_id = env.register_contract(None, VestingVault);
    let client = VestingVaultClient::new(&env, &contract_id);

    // Initiate sunset first
    client.prepare_protocol_sunset(&admin, &migration_target);

    // Export state payload
    let payload_hash = client.export_state_payload(&user, &vesting_id);

    let payload = client.get_migration_payload(&user, &vesting_id);
    assert!(payload.is_some());
    let payload_data = payload.unwrap();
    assert_eq!(payload_data.beneficiary, user);
    assert_eq!(payload_data.vesting_id, vesting_id);
    assert_eq!(payload_data.payload_hash, payload_hash);
}

#[test]
fn test_relayer_migration() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let relayer = Address::generate(&env);
    let migration_target = Address::generate(&env);
    let vesting_id = 1u32;

    let contract_id = env.register_contract(None, VestingVault);
    let client = VestingVaultClient::new(&env, &contract_id);

    // Initiate sunset
    client.prepare_protocol_sunset(&admin, &migration_target);

    // Export payload
    let payload_hash = client.export_state_payload(&user, &vesting_id);

    // Advance time past timelock (30 days)
    env.ledger().set_timestamp(3_000_000);

    // Relayer migrates account
    client.relayer_migrate_account(&relayer, &user, &vesting_id, &payload_hash);

    // Verify migration completed
    assert!(client.is_account_migrated(&user, &vesting_id));
}

#[test]
fn test_migration_security() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let migration_target = Address::generate(&env);
    let vesting_id = 1u32;

    let contract_id = env.register_contract(None, VestingVault);
    let client = VestingVaultClient::new(&env, &contract_id);

    // Initiate sunset
    client.prepare_protocol_sunset(&admin, &migration_target);

    // Export legitimate payload
    let legitimate_hash = client.export_state_payload(&user, &vesting_id);

    // Try to migrate with tampered hash
    let tampered_hash = BytesN::from_array(&env, &[1u8; 32]);
    let relayer = Address::generate(&env);

    // Advance time past timelock
    env.ledger().set_timestamp(3_000_000);

    // This should fail due to hash mismatch
    let result = client.relayer_migrate_account(&relayer, &user, &vesting_id, &tampered_hash);
    assert!(result.is_err());
}

#[test]
fn test_export_fails_without_sunset() {
    let env = Env::default();
    let user = Address::generate(&env);
    let vesting_id = 1u32;

    let contract_id = env.register_contract(None, VestingVault);
    let client = VestingVaultClient::new(&env, &contract_id);

    // Try to export without sunset - should fail
    let result = client.export_state_payload(&user, &vesting_id);
    assert!(result.is_err());
}