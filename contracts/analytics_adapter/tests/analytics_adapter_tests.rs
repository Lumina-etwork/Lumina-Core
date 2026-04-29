#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, Vec,
};

use analytics_adapter::{
    AnalyticsAdapter, AnalyticsAdapterClient,
    DataKey, VestingSchedule, MAX_PAGE_SIZE,
};

fn setup() -> (Env, Address) {
    let env = Env::default();
    let contract_id = env.register_contract(None, AnalyticsAdapter);
    (env, contract_id)
}

fn seed_schedules(env: &Env, contract_id: &Address, count: u32, start_time: u64) {
    env.as_contract(contract_id, || {
        let mut schedules: Vec<VestingSchedule> = Vec::new(env);
        for _ in 0..count {
            schedules.push_back(VestingSchedule {
                owner: Address::generate(env),
                total_amount: 1_000_000_000,
                released_amount: 0,
                start_time,
                duration_secs: 365 * 24 * 60 * 60,
                cliff_secs: 30 * 24 * 60 * 60,
            });
        }
        env.storage().persistent().set(&DataKey::VestingSchedules, &schedules);
    });
}

fn seed_mixed_schedules(env: &Env, contract_id: &Address, count: u32, start_time: u64) {
    env.as_contract(contract_id, || {
        let mut schedules: Vec<VestingSchedule> = Vec::new(env);
        for i in 0..count {
            let released = if i % 2 == 0 { 1_000_000_000i128 } else { 0i128 };
            schedules.push_back(VestingSchedule {
                owner: Address::generate(env),
                total_amount: 1_000_000_000,
                released_amount: released,
                start_time,
                duration_secs: 365 * 24 * 60 * 60,
                cliff_secs: 30 * 24 * 60 * 60,
            });
        }
        env.storage().persistent().set(&DataKey::VestingSchedules, &schedules);
    });
}

#[test]
fn test_tvl_empty_dataset() {
    let (env, id) = setup();
    let client = AnalyticsAdapterClient::new(&env, &id);
    let result = client.get_global_tvl(&0, &10);
    assert_eq!(result.tvl_raw, 0);
    assert_eq!(result.schedules_counted, 0);
    assert!(!result.has_next);
}

#[test]
fn test_tvl_single_page() {
    let (env, id) = setup();
    seed_schedules(&env, &id, 5, 1_000_000);
    let client = AnalyticsAdapterClient::new(&env, &id);
    let result = client.get_global_tvl(&0, &10);
    assert_eq!(result.tvl_raw, 5_000_000_000);
    assert_eq!(result.schedules_counted, 5);
    assert!(!result.has_next);
}

#[test]
fn test_tvl_pagination() {
    let (env, id) = setup();
    seed_schedules(&env, &id, 10, 1_000_000);
    let client = AnalyticsAdapterClient::new(&env, &id);
    let page1 = client.get_global_tvl(&0, &4);
    assert_eq!(page1.schedules_counted, 4);
    assert!(page1.has_next);
    assert_eq!(page1.next_cursor, 4);
    let page2 = client.get_global_tvl(&4, &4);
    assert_eq!(page2.schedules_counted, 4);
    assert!(page2.has_next);
    let page3 = client.get_global_tvl(&8, &4);
    assert_eq!(page3.schedules_counted, 2);
    assert!(!page3.has_next);
    let total = page1.tvl_raw + page2.tvl_raw + page3.tvl_raw;
    assert_eq!(total, 10 * 1_000_000_000i128);
}

#[test]
#[should_panic]
fn test_tvl_query_too_large() {
    let (env, id) = setup();
    let client = AnalyticsAdapterClient::new(&env, &id);
    client.get_global_tvl(&0, &(MAX_PAGE_SIZE + 1));
}

#[test]
#[should_panic]
fn test_tvl_zero_page_size() {
    let (env, id) = setup();
    let client = AnalyticsAdapterClient::new(&env, &id);
    client.get_global_tvl(&0, &0);
}

#[test]
#[should_panic]
fn test_tvl_invalid_cursor() {
    let (env, id) = setup();
    seed_schedules(&env, &id, 3, 1_000_000);
    let client = AnalyticsAdapterClient::new(&env, &id);
    client.get_global_tvl(&999, &10);
}

#[test]
fn test_historical_empty() {
    let (env, id) = setup();
    let client = AnalyticsAdapterClient::new(&env, &id);
    let result = client.get_total_vested_historically(&0, &10);
    assert_eq!(result.total_vested, 0);
    assert_eq!(result.completed_schedules, 0);
}

#[test]
fn test_historical_mixed_schedules() {
    let (env, id) = setup();
    seed_mixed_schedules(&env, &id, 10, 1_000_000);
    let client = AnalyticsAdapterClient::new(&env, &id);
    let result = client.get_total_vested_historically(&0, &50);
    assert_eq!(result.total_vested, 5_000_000_000);
    assert_eq!(result.completed_schedules, 5);
}

#[test]
fn test_historical_pagination_consistency() {
    let (env, id) = setup();
    seed_mixed_schedules(&env, &id, 20, 1_000_000);
    let client = AnalyticsAdapterClient::new(&env, &id);
    let mut total_vested = 0i128;
    let mut completed = 0u32;
    let mut cursor = 0u32;
    loop {
        let page = client.get_total_vested_historically(&cursor, &5);
        total_vested += page.total_vested;
        completed += page.completed_schedules;
        if !page.has_next { break; }
        cursor = page.next_cursor;
    }
    assert_eq!(total_vested, 10_000_000_000i128);
    assert_eq!(completed, 10);
}

#[test]
#[should_panic]
fn test_historical_query_too_large() {
    let (env, id) = setup();
    let client = AnalyticsAdapterClient::new(&env, &id);
    client.get_total_vested_historically(&0, &(MAX_PAGE_SIZE + 1));
}

#[test]
fn test_projected_empty() {
    let (env, id) = setup();
    let client = AnalyticsAdapterClient::new(&env, &id);
    let result = client.get_projected_emissions_30d(&0, &10);
    assert_eq!(result.projected_emissions, 0);
    assert_eq!(result.active_schedules, 0);
}

#[test]
fn test_projected_active_schedules() {
    let (env, id) = setup();
    let start_time: u64 = 1_000_000;
    env.ledger().set(LedgerInfo {
        timestamp: start_time + 60 * 24 * 60 * 60,
        protocol_version: 21,
        sequence_number: 100,
        network_id: Default::default(),
        base_reserve: 5_000_000,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 6_312_000,
    });
    seed_schedules(&env, &id, 4, start_time);
    let client = AnalyticsAdapterClient::new(&env, &id);
    let result = client.get_projected_emissions_30d(&0, &50);
    assert!(result.projected_emissions > 0);
    assert_eq!(result.active_schedules, 4);
}

#[test]
fn test_projected_expired_excluded() {
    let (env, id) = setup();
    let start_time: u64 = 100;
    env.ledger().set(LedgerInfo {
        timestamp: start_time + 999 * 24 * 60 * 60,
        protocol_version: 21,
        sequence_number: 300,
        network_id: Default::default(),
        base_reserve: 5_000_000,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 6_312_000,
    });
    seed_schedules(&env, &id, 5, start_time);
    let client = AnalyticsAdapterClient::new(&env, &id);
    let result = client.get_projected_emissions_30d(&0, &50);
    assert_eq!(result.active_schedules, 0);
    assert_eq!(result.projected_emissions, 0);
}

#[test]
#[should_panic]
fn test_projected_query_too_large() {
    let (env, id) = setup();
    let client = AnalyticsAdapterClient::new(&env, &id);
    client.get_projected_emissions_30d(&0, &(MAX_PAGE_SIZE + 1));
}

#[test]
fn test_tvl_max_page_does_not_panic() {
    let (env, id) = setup();
    seed_schedules(&env, &id, MAX_PAGE_SIZE, 1_000_000);
    let client = AnalyticsAdapterClient::new(&env, &id);
    let result = client.get_global_tvl(&0, &MAX_PAGE_SIZE);
    assert_eq!(result.schedules_counted, MAX_PAGE_SIZE);
}

#[test]
fn test_full_paginated_scan_consistency() {
    let (env, id) = setup();
    let n: u32 = MAX_PAGE_SIZE * 3;
    seed_schedules(&env, &id, n, 1_000_000);
    let client = AnalyticsAdapterClient::new(&env, &id);
    let mut total_tvl = 0i128;
    let mut cursor = 0u32;
    loop {
        let page = client.get_global_tvl(&cursor, &MAX_PAGE_SIZE);
        total_tvl += page.tvl_raw;
        if !page.has_next { break; }
        cursor = page.next_cursor;
    }
    assert_eq!(total_tvl, n as i128 * 1_000_000_000i128);
}
