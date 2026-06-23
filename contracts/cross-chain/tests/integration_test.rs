use cross_chain::consensus::header_cache::{Header, HeaderCache};
use cross_chain::light_client::committee_sync::{CommitteeMember, CommitteeState};
use cross_chain::light_client::finality_verifier::FinalityVerifier;
use cross_chain::metrics::Metrics;
use cross_chain::types::*;

fn make_member(id: u64, weight: u64, drift_ms: i64) -> CommitteeMember {
    CommitteeMember {
        id,
        weight,
        last_seen_ms: 0,
        clock_drift_ms: drift_ms,
    }
}

fn make_header(height: u64, timestamp_ms: TimestampMs) -> Header {
    Header {
        height,
        timestamp_ms,
        hash: [height as u8; 32],
        parent_hash: if height > 0 {
            [(height - 1) as u8; 32]
        } else {
            [0u8; 32]
        },
        is_finalized: false,
    }
}

fn simulate_chain(
    chain_id: ChainId,
    block_time_ms: BlockTimeMs,
    num_blocks: u64,
    latency_ms: u64,
) -> (ChainConfig, CommitteeState, HeaderCache, Metrics) {
    let config = ChainConfig::new(chain_id, block_time_ms);
    let metrics = Metrics::new();

    let members = vec![
        make_member(1, 2, latency_ms as i64 / 2),
        make_member(2, 2, latency_ms as i64 / 2),
        make_member(3, 2, 0),
    ];
    let committee = CommitteeState::new(chain_id, members);

    let mut cache = HeaderCache::new();
    for height in 0..num_blocks {
        cache.push(make_header(height, height * block_time_ms));
    }

    (config, committee, cache, metrics)
}

#[test]
fn two_chain_latency_simulation() {
    let injected_latency_ms = 800;

    let (config_a, committee_a, mut cache_a, metrics_a) = simulate_chain(1, 2000, 10, injected_latency_ms);
    let (config_b, committee_b, mut cache_b, metrics_b) = simulate_chain(2, 15000, 10, injected_latency_ms);

    let drift_a = committee_a.detect_drift(&config_a);
    let drift_b = committee_b.detect_drift(&config_b);

    assert!(drift_a, "800ms latency should trigger drift on 2s chain");
    assert!(drift_b, "800ms latency should trigger drift on 15s chain");

    let now_a = 10 * config_a.block_time_ms;
    let now_b = 10 * config_b.block_time_ms;

    let mut finality_lag_a_ms = 0u64;
    let mut finality_lag_b_ms = 0u64;

    for height in 0..10 {
        if let Some(header) = cache_a.get(height).cloned() {
            if FinalityVerifier::is_header_finalized(
                header.timestamp_ms,
                now_a,
                &committee_a,
                &config_a,
                drift_a,
                &metrics_a,
            ) {
                cache_a.mark_finalized(height);
                finality_lag_a_ms = now_a.saturating_sub(header.timestamp_ms);
            }
        }
    }

    for height in 0..10 {
        if let Some(header) = cache_b.get(height).cloned() {
            if FinalityVerifier::is_header_finalized(
                header.timestamp_ms,
                now_b,
                &committee_b,
                &config_b,
                drift_b,
                &metrics_b,
            ) {
                cache_b.mark_finalized(height);
                finality_lag_b_ms = now_b.saturating_sub(header.timestamp_ms);
            }
        }
    }

    assert!(
        cache_a.finalized_count() > 0,
        "Chain A should have finalized headers"
    );
    assert!(
        cache_b.finalized_count() > 0,
        "Chain B should have finalized headers"
    );

    let recorded_lag_a = metrics_a.finality_lag_ms(1).unwrap_or(0) as u64;
    let recorded_lag_b = metrics_b.finality_lag_ms(2).unwrap_or(0) as u64;

    assert!(
        recorded_lag_a >= config_a.grace_timeout_ms() || recorded_lag_a >= config_a.sync_timeout_ms,
        "Finality lag on chain A should be at least the sync/grace timeout"
    );
    assert!(
        recorded_lag_b >= config_b.grace_timeout_ms() || recorded_lag_b >= config_b.sync_timeout_ms,
        "Finality lag on chain B should be at least the sync/grace timeout"
    );
}

#[test]
fn low_latency_no_drift() {
    let (config_a, committee_a, cache_a, metrics_a) = simulate_chain(1, 2000, 10, 50);
    let drift = committee_a.detect_drift(&config_a);
    assert!(!drift, "50ms latency should not trigger drift on 2s chain");

    let now = 10 * config_a.block_time_ms;
    let header = cache_a.latest().unwrap();
    let finalized = FinalityVerifier::is_header_finalized(
        header.timestamp_ms,
        now,
        &committee_a,
        &config_a,
        drift,
        &metrics_a,
    );

    assert!(finalized, "header should be finalized without drift after sync_timeout");
}

#[test]
fn sync_backoff_recovers() {
    let metrics = Metrics::new();
    let config = ChainConfig::new(1, 2000);
    let members = vec![make_member(1, 1, 0)];
    let mut committee = CommitteeState::new(1, members);

    committee.record_sync_attempt(1000, false, &metrics);
    assert_eq!(committee.sync_attempts, 1);
    assert_eq!(committee.retry_backoff_ms, 2000);

    committee.record_sync_attempt(3000, false, &metrics);
    assert_eq!(committee.sync_attempts, 2);
    assert_eq!(committee.retry_backoff_ms, 4000);

    committee.record_sync_attempt(7000, true, &metrics);
    assert_eq!(committee.sync_attempts, 0);
    assert_eq!(committee.retry_backoff_ms, 1000);
}

#[test]
fn header_cache_256_bound() {
    let mut cache = HeaderCache::new();
    for i in 0..300u64 {
        cache.push(make_header(i, i * 2000));
    }
    assert_eq!(cache.len(), 256);
    assert!(cache.get(43).is_some());
    assert!(cache.get(44).is_some());
}

#[test]
fn finality_lag_metric_recorded() {
    let metrics = Metrics::new();
    let config = ChainConfig::new(1, 2000);
    let members = vec![make_member(1, 3, 0), make_member(2, 3, 0), make_member(3, 3, 0)];
    let committee = CommitteeState::new(1, members);

    FinalityVerifier::is_header_finalized(0, config.sync_timeout_ms, &committee, &config, false, &metrics);

    let lag = metrics.finality_lag_ms(1);
    assert!(lag.is_some(), "finality lag metric should be recorded");
    assert!(lag.unwrap() >= config.sync_timeout_ms as i64);
}
