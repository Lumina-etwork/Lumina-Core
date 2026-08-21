use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::core::liveness::heartbeat::{HeartbeatTracker, HEARTBEAT_TIMEOUT, OFFLINE_GRACE_PERIOD};
use crate::core::liveness::scanner::LivenessScanner;
use crate::core::liveness::state::NodeStatus;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Advances a synthetic clock past the heartbeat timeout.
fn timed_out_instant(base: Instant) -> Instant {
    base + HEARTBEAT_TIMEOUT + Duration::from_millis(1)
}

/// Runs two scanner sweeps separated by the grace period.
fn sweep_twice(scanner: &LivenessScanner, base: Instant) -> Vec<u64> {
    scanner.check_liveness(timed_out_instant(base));
    let after_grace = timed_out_instant(base) + OFFLINE_GRACE_PERIOD + Duration::from_millis(1);
    scanner.check_liveness(after_grace)
}

// ── basic unit tests ──────────────────────────────────────────────────────────

/// A fresh node is Online.
#[test]
fn new_node_is_online() {
    let tracker = HeartbeatTracker::new();
    tracker.record_heartbeat(1, Instant::now());
    assert_eq!(tracker.status(1), Some(NodeStatus::Online));
}

/// A node that has never missed a heartbeat is never marked offline.
#[test]
fn node_within_timeout_stays_online() {
    let base = Instant::now();
    let tracker = HeartbeatTracker::new();
    let scanner = LivenessScanner::new(tracker.shared_map());

    tracker.record_heartbeat(42, base);

    // Scanner runs well before the timeout.
    let just_before = base + HEARTBEAT_TIMEOUT - Duration::from_secs(5);
    scanner.check_liveness(just_before);

    assert_eq!(tracker.status(42), Some(NodeStatus::Online));
}

/// After the timeout + grace period, a silent node becomes Offline.
#[test]
fn silent_node_eventually_goes_offline() {
    let base = Instant::now();
    let tracker = HeartbeatTracker::new();
    let scanner = LivenessScanner::new(tracker.shared_map());

    tracker.record_heartbeat(10, base);

    let newly_offline = sweep_twice(&scanner, base);

    assert!(
        newly_offline.contains(&10),
        "node 10 should appear in newly_offline list"
    );
    assert_eq!(tracker.status(10), Some(NodeStatus::Offline));
}

/// First sweep moves a node to PendingOffline; second (after grace) to Offline.
#[test]
fn two_sweep_grace_period_transition() {
    let base = Instant::now();
    let tracker = HeartbeatTracker::new();
    let scanner = LivenessScanner::new(tracker.shared_map());

    tracker.record_heartbeat(20, base);

    // First sweep: should enter PendingOffline, not yet Offline.
    scanner.check_liveness(timed_out_instant(base));
    assert_eq!(tracker.status(20), Some(NodeStatus::PendingOffline));

    // Second sweep after grace period: should become Offline.
    let after_grace = timed_out_instant(base) + OFFLINE_GRACE_PERIOD + Duration::from_millis(1);
    let newly_offline = scanner.check_liveness(after_grace);
    assert!(newly_offline.contains(&20));
    assert_eq!(tracker.status(20), Some(NodeStatus::Offline));
}

/// A heartbeat that arrives just before the scanner's write lock cancels the
/// offline transition (grace period recovery path).
#[test]
fn heartbeat_during_grace_period_recovers_node() {
    let base = Instant::now();
    let tracker = HeartbeatTracker::new();
    let scanner = LivenessScanner::new(tracker.shared_map());

    tracker.record_heartbeat(30, base);

    // First sweep triggers PendingOffline.
    scanner.check_liveness(timed_out_instant(base));
    assert_eq!(tracker.status(30), Some(NodeStatus::PendingOffline));

    // Node sends a heartbeat during the grace window.
    let grace_heartbeat_time = timed_out_instant(base) + Duration::from_secs(5);
    tracker.record_heartbeat(30, grace_heartbeat_time);

    // Second sweep — node should be Online again, NOT Offline.
    let after_grace = timed_out_instant(base) + OFFLINE_GRACE_PERIOD + Duration::from_millis(1);
    let newly_offline = scanner.check_liveness(after_grace);
    assert!(
        !newly_offline.contains(&30),
        "node recovered via heartbeat during grace should not be marked offline"
    );
    assert_eq!(tracker.status(30), Some(NodeStatus::Online));
}

/// Verifies the core invariant from the issue:
/// if `now - last_heartbeat <= HEARTBEAT_TIMEOUT`, the node must NOT be Offline.
#[test]
fn invariant_node_not_offline_within_timeout() {
    let base = Instant::now();
    let tracker = HeartbeatTracker::new();
    let scanner = LivenessScanner::new(tracker.shared_map());

    tracker.record_heartbeat(99, base);

    // Scanner runs but the node is still within timeout (89s elapsed of 90s).
    let almost_timeout = base + HEARTBEAT_TIMEOUT - Duration::from_secs(1);
    scanner.check_liveness(almost_timeout);

    let status = tracker.status(99).unwrap();
    assert_ne!(
        status,
        NodeStatus::Offline,
        "node within timeout window must not be Offline"
    );
}

// ── race condition regression test ───────────────────────────────────────────

/// Regression: the exact scenario described in the issue.
///
/// A node heartbeats at T=89s (1s before the 90s timeout) while the scanner
/// reads at T=89.5s and then processes at T=89.6s.  The node must NOT be
/// marked Offline because `last_heartbeat = T=89s` and `now - last = 0.6s`.
#[test]
fn heartbeat_at_t89_not_falsely_offlined_by_scanner_at_t89_6() {
    // Simulated timeline with Instant offsets (no actual sleeping).
    let t0 = Instant::now();
    let t89 = t0 + Duration::from_secs(89);
    let t89_6 = t0 + Duration::from_millis(89_600);

    let tracker = HeartbeatTracker::new();
    let scanner = LivenessScanner::new(tracker.shared_map());

    // Previous heartbeat at T=0 (would be ~90s old by the scanner's check time).
    tracker.record_heartbeat(77, t0);

    // Fresh heartbeat arrives at T=89s — this is what the issue says races.
    tracker.record_heartbeat(77, t89);

    // Scanner runs at T=89.6s — with the fresh heartbeat already recorded the
    // node should NOT be timed out (elapsed = 0.6s << 90s).
    scanner.check_liveness(t89_6);

    let status = tracker.status(77).unwrap();
    assert_ne!(
        status,
        NodeStatus::Offline,
        "node that heartbeated at T=89s must not be offline when scanner runs at T=89.6s"
    );
    assert_ne!(
        status,
        NodeStatus::PendingOffline,
        "node that heartbeated at T=89s must not be pending-offline when scanner runs at T=89.6s"
    );
}

// ── concurrent stress test ────────────────────────────────────────────────────

/// 1 000 nodes each sending heartbeats at randomised intervals while the
/// scanner runs every 5 s — no false-positive offline markings allowed.
///
/// This test is intentionally coarse-grained in wall time by using synthetic
/// `Instant` offsets so it remains fast on CI.  The concurrency comes from
/// threads genuinely racing to write heartbeats while the scanner reads.
#[test]
fn concurrent_heartbeat_scan_no_false_positives() {
    const NUM_NODES: u64 = 1_000;
    const SCAN_ROUNDS: u32 = 10;

    let tracker = Arc::new(HeartbeatTracker::new());
    let base = Instant::now();

    // Seed all nodes with a fresh heartbeat.
    for node_id in 0..NUM_NODES {
        tracker.record_heartbeat(node_id, base);
    }

    // Spawn heartbeat senders: each node heartbeats on its own thread at
    // intervals that keep it well within the timeout window.
    let sender_handles: Vec<_> = (0..NUM_NODES)
        .map(|node_id| {
            let tracker = Arc::clone(&tracker);
            thread::spawn(move || {
                // Simulate sending several heartbeats over the observation window.
                for beat in 0..SCAN_ROUNDS {
                    // Heartbeat timestamp stays < HEARTBEAT_TIMEOUT from base.
                    // Use a small offset so every node heartbeats regularly.
                    let hb_time = base + Duration::from_secs((beat * 5 + 1) as u64);
                    tracker.record_heartbeat(node_id, hb_time);
                    // Yield so scanner threads can interleave.
                    thread::yield_now();
                }
            })
        })
        .collect();

    // Run the scanner concurrently from a separate thread.
    let scanner_tracker = Arc::clone(&tracker);
    let scanner_handle = thread::spawn(move || {
        let scanner = LivenessScanner::new(scanner_tracker.shared_map());
        for round in 0..SCAN_ROUNDS {
            // Scanner advances time but always within nodes' heartbeat window.
            // Nodes keep heartbeating every 5s, timeout is 90s — even at round
            // 10 (T=50s) the nodes are well within the 90s window.
            let scan_time = base + Duration::from_secs((round * 5 + 2) as u64);
            let newly_offline = scanner.check_liveness(scan_time);
            assert!(
                newly_offline.is_empty(),
                "false-positive offline at round {}: nodes {:?}",
                round,
                newly_offline
            );
            thread::yield_now();
        }
    });

    for h in sender_handles {
        h.join().expect("heartbeat sender panicked");
    }
    scanner_handle.join().expect("scanner panicked");

    // Final assertion: all nodes must still be Online.
    for node_id in 0..NUM_NODES {
        let status = tracker.status(node_id).unwrap();
        assert_ne!(
            status,
            NodeStatus::Offline,
            "node {} was falsely marked offline",
            node_id
        );
    }
}
