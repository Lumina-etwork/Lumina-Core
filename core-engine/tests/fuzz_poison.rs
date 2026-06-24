//! Fuzz test: submit 1,000 malicious binding updates with forged tickets,
//! verify zero cache-poisoning successes.
//!
//! Run with: cargo test --test fuzz_poison -- --nocapture

use core_engine::attestation::relay_ticket::{generate_relay_keypair, RelayTicket};
use core_engine::net::relay::endpoint_cache::{CacheConfig, EndpointCache};
use core_engine::net::relay::relay_registry::RelayRegistry;
use core_engine::net::relay::stun_bind::StunBindHandler;

use rand::Rng;

const FUZZ_ITERATIONS: usize = 1_000;

#[test]
fn fuzz_1000_malicious_bindings_no_poisoning() {
    let (_honest_sk, honest_vk) = generate_relay_keypair();
    let (attacker_sk, attacker_vk) = generate_relay_keypair();

    let mut registry = RelayRegistry::new();
    registry.register_relay("honest_relay", &honest_vk);
    registry.register_relay("attacker", &attacker_vk);

    let mut cache = EndpointCache::new(CacheConfig {
        poison_threshold: 5,
        penalty_window_secs: 60,
        entry_ttl_secs: 300,
        max_endpoints_per_peer: 16,
        max_cache_entries: 10_000,
    });
    registry.sync_to_cache(&mut cache);

    let _handler = StunBindHandler::new(300);
    let mut rng = rand::thread_rng();

    let mut successful_poisons = 0;
    let mut total_attempts = 0;

    for i in 0..FUZZ_ITERATIONS {
        let target_id = format!("victim_peer_{}", rng.gen_range(0..10));
        let fake_relay_id = format!("evil_relay_{}", rng.gen_range(0..5));

        // Generate a ticket with attacker's key (not registered)
        let mut ticket = RelayTicket::new(
            &attacker_sk,
            &fake_relay_id,
            &target_id,
            i as u64 + 1,
            300,
        );

        // 50% chance — tamper with target (mismatch)
        if rng.gen_bool(0.5) {
            ticket.target_id = format!("evil_target_{}", rng.gen_range(0..100));
        }

        // 30% chance — tamper with signature
        if rng.gen_bool(0.3) {
            ticket.signature = hex::encode(vec![0u8; 64]);
        }

        // 20% chance — use expired ticket
        if rng.gen_bool(0.2) {
            ticket.expiry = 0; // way in the past
        }

        total_attempts += 1;

        // Try to put in cache
        match cache.put_endpoint(&target_id, ticket) {
            Ok(()) => {
                successful_poisons += 1;
                eprintln!(
                    "DANGER: poison succeeded at iteration {} — target={} relay={}",
                    i, target_id, fake_relay_id
                );
            }
            Err(_) => {
                // Expected — ticket should be rejected
            }
        }
    }

    println!(
        "Fuzz results: {}/{} attempts — {} successful poisons",
        total_attempts - successful_poisons,
        total_attempts,
        successful_poisons
    );

    assert_eq!(
        successful_poisons, 0,
        "Fuzz test failed: {} cache-poisoning successes detected",
        successful_poisons
    );

    // Verify the cache is clean
    let stats = cache.stats();
    println!("Cache stats: {:?}", stats);
    assert_eq!(stats.total_entries, 0, "Cache should be empty after all fuzz attempts");
}

#[test]
fn fuzz_attacker_blacklisted_after_threshold() {
    let (_honest_sk, honest_vk) = generate_relay_keypair();
    let (attacker_sk, attacker_vk) = generate_relay_keypair();

    let mut registry = RelayRegistry::new();
    registry.register_relay("honest_relay", &honest_vk);
    registry.register_relay("attacker", &attacker_vk);

    let mut cache = EndpointCache::new(CacheConfig {
        poison_threshold: 5,
        penalty_window_secs: 60,
        ..Default::default()
    });
    registry.sync_to_cache(&mut cache);

    // Submit 5 bad tickets — should trigger blacklist
    for i in 0..5 {
        let mut ticket = RelayTicket::new(&attacker_sk, "attacker", "victim", i + 1, 300);
        ticket.target_id = "tampered".to_string();
        cache.report_poison_attempt("attacker");
    }

    assert!(cache.is_blacklisted("attacker"), "Attacker should be blacklisted after 5 poison attempts");

    // Even a valid ticket from attacker should now be rejected
    let valid_ticket = RelayTicket::new(&attacker_sk, "attacker", "victim", 6, 300);
    assert!(cache.put_endpoint("victim", valid_ticket).is_err(),
        "Blacklisted attacker should not be able to insert valid tickets");
}

#[test]
fn fuzz_honest_relay_works_fine() {
    let (_honest_sk, honest_vk) = generate_relay_keypair();

    let mut registry = RelayRegistry::new();
    registry.register_relay("honest_relay", &honest_vk);

    let mut cache = EndpointCache::new(CacheConfig::default());
    registry.sync_to_cache(&mut cache);

    // Submit 100 valid tickets from honest relay — all should succeed
    for i in 0..100 {
        let ticket = RelayTicket::new(&honest_sk, "honest_relay", "honest_peer", i + 1, 300);
        assert!(cache.put_endpoint("honest_peer", ticket).is_ok(),
            "Honest relay ticket #{} failed", i);
    }

    assert!(!cache.is_blacklisted("honest_relay"), "Honest relay should not be blacklisted");
    let stats = cache.stats();
    assert!(stats.blacklisted_relays == 0, "No relays should be blacklisted");
}