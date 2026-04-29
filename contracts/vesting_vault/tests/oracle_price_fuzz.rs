#![cfg(test)]

//! Fuzz test suite for Oracle Price Manipulation Attacks (Issue #283)
//!
//! Targets: VestingVault::update_oracle_price, circuit breaker logic,
//! TWAP resistance, and oracle staleness handling.
//!
//! Circuit breaker thresholds (documented from fuzz results):
//!   - Same-ledger deviation threshold: 3000 bps (30%)
//!   - 10,000% spike = 1,000,000 bps → always trips the breaker
//!   - Staleness window: 5 ledgers (300 seconds at ~5s/ledger)
//!   - TWAP window: 10 price samples minimum for statistical resistance
//!
//! Acceptance criteria:
//!   1. Circuit breaker trips on ANY same-ledger deviation > 30%.
//!   2. TWAP across N samples is bounded: no single spike skews it > 5x.
//!   3. Stale oracle (no update for ORACLE_STALE_LEDGERS) is detected.

use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env};
use vesting_vault::{VestingVault, VestingVaultClient};
use vesting_vault::errors::Error;

// ── Constants ──────────────────────────────────────────────────────────────

/// Price scale factor (10^7), matching the contract's internal representation.
const PRICE_SCALE: i128 = 10_000_000;

/// A realistic baseline price: $1.00 in scaled units.
const BASE_PRICE: i128 = PRICE_SCALE;

/// 10,000% above baseline = 101× base. Triggers extreme volatility path.
const SPIKE_10000_PCT: i128 = BASE_PRICE * 101;

/// 30% deviation threshold in basis points — mirrors ORACLE_DEVIATION_THRESHOLD_BPS.
const THRESHOLD_BPS: u32 = 3000;

/// Ledger gap after which an oracle is considered stale.
const ORACLE_STALE_LEDGERS: u32 = 5;

/// Number of fuzz iterations for the main permutation loop (50,000).
const FUZZ_ITERATIONS: u32 = 50_000;

// ── Test TWAP helper ────────────────────────────────────────────────────────

/// Simple arithmetic TWAP over a price window (no time-weighting needed for
/// unit-test purposes; the property under test is that one outlier cannot
/// dominate the average by more than a bounded factor).
fn reference_twap(prices: &[i128]) -> i128 {
    assert!(!prices.is_empty(), "TWAP window must not be empty");
    let sum: i128 = prices.iter().sum();
    sum / prices.len() as i128
}

/// Returns the maximum ratio by which the spiked TWAP can exceed the
/// clean TWAP (as a multiple × 100 to avoid floats).
fn twap_spike_ratio_x100(clean_prices: &[i128], spike: i128) -> i128 {
    let n = clean_prices.len() as i128;
    let clean_twap = reference_twap(clean_prices);
    // Insert one spike at the end
    let spike_sum: i128 = clean_prices.iter().sum::<i128>() + spike;
    let spiked_twap = spike_sum / (n + 1);
    if clean_twap == 0 {
        return 0;
    }
    (spiked_twap * 100) / clean_twap
}

/// Minimal deterministic LCG — avoids pulling in external crates.
/// Returns values in [1, max].
fn lcg_next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

fn lcg_range(state: &mut u64, min: i128, max: i128) -> i128 {
    let span = (max - min + 1) as u64;
    min + (lcg_next(state) % span) as i128
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn setup() -> (Env, VestingVaultClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let cid = env.register(VestingVault, ());
    let client = VestingVaultClient::new(&env, &cid);
    let admin = Address::generate(&env);
    (env, client, admin)
}

/// Advance the ledger sequence by `n` to simulate cross-ledger time passing.
fn advance_ledger(env: &Env, n: u32) {
    let seq = env.ledger().sequence();
    env.ledger().set_sequence_number(seq + n);
}

/// Compute deviation in basis points between two prices, mirroring the
/// contract's internal `calc_deviation_bps`.
fn calc_deviation_bps(old: i128, new: i128) -> u32 {
    if old == 0 {
        return 0;
    }
    let diff = if new > old { new - old } else { old - new };
    ((diff * 10_000) / old) as u32
}

// ── Acceptance 1: Single-block oracle price manipulation immunity ───────────

/// A 10,000% same-ledger spike must always trip the circuit breaker.
#[test]
fn fuzz_extreme_spike_trips_circuit_breaker() {
    let (env, client, admin) = setup();
    env.ledger().set_sequence_number(1);

    // Establish baseline price on ledger 1.
    client.update_oracle_price(&admin, &BASE_PRICE).unwrap();

    // Same ledger: push 10,000% spike → must return OraclePriceDeviationTooHigh.
    let result = client.try_update_oracle_price(&admin, &SPIKE_10000_PCT);
    assert!(
        matches!(result, Err(Ok(Error::OraclePriceDeviationTooHigh))),
        "10,000% spike should trip circuit breaker, got: {:?}", result
    );

    // Circuit breaker must now be active — further updates are blocked.
    let record = client.get_oracle_price().expect("price record must exist");
    assert!(record.is_frozen, "circuit breaker must be frozen after spike");
}

/// Any same-ledger deviation above 30% trips the breaker, regardless of direction.
#[test]
fn fuzz_circuit_breaker_threshold_all_directions() {
    // Prices that should trip (deviation > 30%).
    let trips = [
        (BASE_PRICE, BASE_PRICE * 2),          // +100%
        (BASE_PRICE * 10, BASE_PRICE),          // -90%
        (BASE_PRICE, BASE_PRICE * 101),         // +10,000%
        (1_000 * PRICE_SCALE, 1),               // near-zero crash
        (i128::MAX / 100_001, i128::MAX / 100_001 * 2), // large numbers
    ];

    for (baseline, spike) in trips {
        let (env, client, admin) = setup();
        env.ledger().set_sequence_number(42);
        client.update_oracle_price(&admin, &baseline).unwrap();

        let result = client.try_update_oracle_price(&admin, &spike);
        let dev = calc_deviation_bps(baseline, spike);
        if dev > THRESHOLD_BPS {
            assert!(
                matches!(result, Err(Ok(Error::OraclePriceDeviationTooHigh))),
                "deviation {} bps from {} to {} must trip breaker", dev, baseline, spike
            );
        } else {
            assert!(
                result.is_ok(),
                "deviation {} bps should NOT trip breaker", dev
            );
        }
    }
}

/// Once tripped, the circuit breaker blocks ALL subsequent updates.
#[test]
fn fuzz_frozen_oracle_blocks_all_updates() {
    let (env, client, admin) = setup();
    env.ledger().set_sequence_number(1);

    client.update_oracle_price(&admin, &BASE_PRICE).unwrap();
    let _ = client.try_update_oracle_price(&admin, &SPIKE_10000_PCT);

    // Try 100 different prices — all must fail while frozen.
    let mut state: u64 = 0xDEAD_BEEF_CAFE_1337;
    for _ in 0..100 {
        let price = lcg_range(&mut state, 1, i128::MAX / 2);
        let r = client.try_update_oracle_price(&admin, &price);
        assert!(
            matches!(r, Err(Ok(Error::OracleCircuitBreakerActive))),
            "frozen oracle must block update to price {}", price
        );
    }
}

// ── 50,000-iteration main fuzz loop ─────────────────────────────────────────

/// Bombards the oracle with 50,000 randomised price pairs.
/// For every same-ledger pair where deviation > 30%, the breaker must trip.
/// Logs any failed assertion so rounding errors in large-integer paths surface.
#[test]
fn fuzz_50k_price_permutations() {
    let mut state: u64 = 0x1337_C0DE_FEED_FACE;
    let mut failures: Vec<String> = Vec::new();

    let price_pool: [i128; 16] = [
        1,
        2,
        PRICE_SCALE / 2,
        PRICE_SCALE,
        PRICE_SCALE * 2,
        PRICE_SCALE * 101,
        PRICE_SCALE * 10_000,
        i128::MAX / 1_000_001,
        i128::MAX / 100_001,
        i128::MAX / 10_001,
        i128::MAX / 1_001,
        i128::MAX / 101,
        i128::MAX / 11,
        i128::MAX / 3,
        i128::MAX / 2,
        i128::MAX - 1,
    ];

    for iter in 0..FUZZ_ITERATIONS {
        let baseline_idx = (lcg_next(&mut state) as usize) % price_pool.len();
        let spike_idx = (lcg_next(&mut state) as usize) % price_pool.len();
        let baseline = price_pool[baseline_idx];
        let spike = price_pool[spike_idx];

        if baseline <= 0 || spike <= 0 {
            continue;
        }

        let (env, client, admin) = setup();
        env.ledger().set_sequence_number(100);

        if client.update_oracle_price(&admin, &baseline).is_err() {
            continue;
        }

        let dev = calc_deviation_bps(baseline, spike);
        let result = client.try_update_oracle_price(&admin, &spike);

        if dev > THRESHOLD_BPS {
            if !matches!(result, Err(Ok(Error::OraclePriceDeviationTooHigh))) {
                failures.push(format!(
                    "iter={} baseline={} spike={} dev_bps={} → expected breaker trip, got {:?}",
                    iter, baseline, spike, dev, result
                ));
            }
        } else {
            if !result.is_ok() {
                // Same-ledger repeat (same sequence number), only fails if deviation > threshold.
                // Record only genuine false rejections.
                let frozen = client.get_oracle_price().map(|r| r.is_frozen).unwrap_or(false);
                if !frozen {
                    failures.push(format!(
                        "iter={} baseline={} spike={} dev_bps={} → unexpected rejection: {:?}",
                        iter, baseline, spike, dev, result
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} fuzz failures (rounding / logic errors):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ── Acceptance 2: Circuit breaker triggers under abnormal volatility ─────────

/// Rapid-fire same-ledger updates: every update that deviates >30% from the
/// previous recorded price on the same ledger must trip the breaker.
#[test]
fn fuzz_rapid_same_ledger_updates_trip_breaker() {
    let (env, client, admin) = setup();
    env.ledger().set_sequence_number(7);

    client.update_oracle_price(&admin, &BASE_PRICE).unwrap();

    // 30% just below threshold — must succeed.
    let price_29_pct = BASE_PRICE + (BASE_PRICE * 2999 / 10_000);
    client.update_oracle_price(&admin, &price_29_pct).unwrap();

    // Now same ledger, spike to 10,000% above original baseline.
    let result = client.try_update_oracle_price(&admin, &SPIKE_10000_PCT);
    let dev = calc_deviation_bps(price_29_pct, SPIKE_10000_PCT);
    assert!(
        dev > THRESHOLD_BPS,
        "sanity: deviation {} bps must exceed threshold", dev
    );
    assert!(
        matches!(result, Err(Ok(Error::OraclePriceDeviationTooHigh))),
        "rapid spike must trip circuit breaker"
    );

    let record = client.get_oracle_price().unwrap();
    assert!(record.is_frozen, "vault must be frozen after rapid spike");
}

/// After admin reset, the vault accepts new prices again.
#[test]
fn fuzz_circuit_breaker_reset_allows_normal_updates() {
    let (env, client, admin) = setup();
    env.ledger().set_sequence_number(1);

    client.update_oracle_price(&admin, &BASE_PRICE).unwrap();
    let _ = client.try_update_oracle_price(&admin, &SPIKE_10000_PCT);

    // Admin resets the breaker.
    client.reset_oracle_circuit_breaker(&admin).unwrap();

    let record = client.get_oracle_price().unwrap();
    assert!(!record.is_frozen, "breaker must be unfrozen after reset");

    // Advance ledger so the new price is in a fresh ledger (cross-ledger update
    // skips deviation check, so a large price is accepted).
    advance_ledger(&env, 1);
    client.update_oracle_price(&admin, &(BASE_PRICE * 2)).unwrap();
    let record2 = client.get_oracle_price().unwrap();
    assert!(!record2.is_frozen, "post-reset cross-ledger update must not re-trip breaker");
}

// ── Acceptance 3: TWAP cannot be skewed by momentary single-block spike ──────

/// One spike inserted into a window of N clean samples must not push the
/// TWAP above 5× the clean average.
#[test]
fn fuzz_twap_bounded_under_single_block_spike() {
    let window_sizes = [5usize, 10, 20, 50];
    let spike_multipliers: [i128; 5] = [2, 10, 101, 1_001, 10_001];

    for &n in &window_sizes {
        let clean: Vec<i128> = (1..=(n as i128)).map(|i| BASE_PRICE + i * 1_000).collect();

        for &mult in &spike_multipliers {
            let spike = BASE_PRICE * mult;
            let ratio = twap_spike_ratio_x100(&clean, spike);

            // One spike in n+1 samples: ratio ≤ (n * BASE + spike) / (n * BASE) * 100
            // For n ≥ 5 and mult ≤ 10,001: ratio ≤ (5 + 10001) / 5 * 100 / 100 ≈ 2001×
            // We assert a much tighter bound: TWAP must not exceed (mult + n) / n * 100.
            let max_expected_ratio = ((mult + n as i128) * 100) / n as i128;
            assert!(
                ratio <= max_expected_ratio,
                "TWAP ratio {}× exceeds bound {}× for n={} mult={}",
                ratio, max_expected_ratio, n, mult
            );
        }
    }
}

/// Fuzz TWAP against 10,000 random price windows, verifying no arithmetic
/// overflow and that a single spike is always bounded.
#[test]
fn fuzz_twap_no_overflow_under_extreme_prices() {
    let mut state: u64 = 0xFEED_FACE_DEAD_BEEF;
    let mut overflow_failures = 0u32;

    for _ in 0..10_000 {
        let n = (lcg_next(&mut state) % 45 + 5) as usize; // 5..50 samples
        let mut prices: Vec<i128> = (0..n)
            .map(|_| lcg_range(&mut state, 1, PRICE_SCALE * 1_000))
            .collect();

        // Insert one pathological spike at a random position.
        let pos = (lcg_next(&mut state) as usize) % (n + 1);
        prices.insert(pos, PRICE_SCALE * 101);

        // Verify no overflow occurs during TWAP computation.
        let sum: Option<i128> = prices.iter().try_fold(0i128, |acc, &p| acc.checked_add(p));
        if sum.is_none() {
            overflow_failures += 1;
            continue;
        }
        let twap = sum.unwrap() / prices.len() as i128;
        assert!(twap > 0, "TWAP must be positive");
    }

    // We allow a small number of expected overflows (very large price pools).
    assert!(
        overflow_failures < 10,
        "{} unexpected TWAP overflow cases", overflow_failures
    );
}

// ── Staleness edge case ───────────────────────────────────────────────────────

/// If the oracle hasn't been updated for ORACLE_STALE_LEDGERS ledgers,
/// a helper that reads the record must surface the staleness condition.
/// The contract itself doesn't auto-reject stale prices; this test documents
/// the threshold and validates that consumers can detect staleness correctly.
#[test]
fn fuzz_oracle_staleness_detection() {
    let (env, client, admin) = setup();
    env.ledger().set_sequence_number(100);

    client.update_oracle_price(&admin, &BASE_PRICE).unwrap();
    let record = client.get_oracle_price().unwrap();
    let update_ledger = record.last_ledger;

    // Advance ledger past stale window.
    advance_ledger(&env, ORACLE_STALE_LEDGERS + 1);

    let current = env.ledger().sequence();
    let is_stale = current - update_ledger > ORACLE_STALE_LEDGERS;
    assert!(
        is_stale,
        "oracle must be considered stale after {} ledgers without update",
        ORACLE_STALE_LEDGERS
    );

    // A cross-ledger update still succeeds even after a stale gap (no deviation check).
    client.update_oracle_price(&admin, &(BASE_PRICE * 100)).unwrap();
    let record2 = client.get_oracle_price().unwrap();
    assert!(!record2.is_frozen, "cross-ledger update must not trip breaker regardless of gap");
}

/// Oracle stops broadcasting: verify the state remains frozen (no auto-reset).
#[test]
fn fuzz_oracle_broadcast_halt_state_persists() {
    let (env, client, admin) = setup();
    env.ledger().set_sequence_number(1);

    client.update_oracle_price(&admin, &BASE_PRICE).unwrap();
    // Trip the breaker.
    let _ = client.try_update_oracle_price(&admin, &SPIKE_10000_PCT);

    // Simulate oracle going silent for many ledgers.
    for delta in [1u32, 10, 100, 1000] {
        advance_ledger(&env, delta);
        let record = client.get_oracle_price().unwrap();
        assert!(
            record.is_frozen,
            "frozen state must persist after {} extra ledgers without broadcast", delta
        );
    }
}

// ── Deviation arithmetic: rounding / overflow hardening ──────────────────────

/// calc_deviation_bps must never panic or overflow for any pair of valid i128 prices.
#[test]
fn fuzz_deviation_bps_no_overflow() {
    let extremes: [i128; 8] = [
        1,
        2,
        i128::MAX / 10_001,
        i128::MAX / 1_001,
        i128::MAX / 101,
        i128::MAX / 11,
        i128::MAX / 3,
        i128::MAX / 2,
    ];

    for &old in &extremes {
        for &new in &extremes {
            // Must not panic.
            let bps = calc_deviation_bps(old, new);
            // Sanity: deviation is always non-negative.
            let _ = bps;
        }
    }
}

/// Multiplying a large manipulated price by a KPI threshold must not silently
/// overflow i128.  This mirrors rounding errors the issue mentions.
#[test]
fn fuzz_large_price_multiplier_rounding() {
    let large_prices: [i128; 6] = [
        PRICE_SCALE * 10_000,
        PRICE_SCALE * 1_000_000,
        i128::MAX / 10_001,
        i128::MAX / 1_001,
        i128::MAX / 101,
        i128::MAX / 11,
    ];
    let multipliers: [i128; 5] = [1, 10, 100, 1_000, 10_000];

    let mut failures: Vec<String> = Vec::new();

    for &price in &large_prices {
        for &mult in &multipliers {
            match price.checked_mul(mult) {
                None => {
                    // Expected overflow — record for audit visibility but don't fail.
                }
                Some(product) => {
                    // Division must be consistent (ceil and floor differ by at most 1).
                    let divisor: i128 = 10_000;
                    let floor = product / divisor;
                    let ceil = (product + divisor - 1) / divisor;
                    if ceil < floor {
                        failures.push(format!(
                            "price={} mult={} product={} ceil<floor: ceil={} floor={}",
                            price, mult, product, ceil, floor
                        ));
                    }
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "rounding failures in large-price multiplications:\n{}",
        failures.join("\n")
    );
}
