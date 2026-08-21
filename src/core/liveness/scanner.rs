use std::time::Instant;

use super::heartbeat::{HeartbeatMap, HEARTBEAT_TIMEOUT, OFFLINE_GRACE_PERIOD};
use super::state::NodeStatus;

/// Scans the heartbeat map and updates node liveness status.
///
/// # Race-free timeout check  (issue blueprint — resolution option 1 + 2 + 4)
///
/// The classic read-write race the issue describes:
///
/// ```text
///   T=89s   node sends heartbeat → record_heartbeat() queued
///   T=89.5s scanner reads last_heartbeat = T=0 (elapsed = 89.5s > 90s? no, but close)
///           … wait, elapsed = 89.5s which IS < 90s here, but the issue describes
///           it firing because last_heartbeat still shows the PREVIOUS heartbeat time
///           (e.g. T=0, so elapsed = 89.5s which is NOT yet > 90s —  the race fires
///           when the scanner runs at e.g. T=90.5s and sees the old T=0 value because
///           the fresh write at T=89s hasn't been incorporated yet under a stale read)
/// ```
///
/// Resolution implemented here:
///
/// 1. **Double-checked timeout with generation counter** (options 1 & 2):
///    Before promoting a node to `PendingOffline` the scanner reads
///    `(last_heartbeat, generation)` under a *read* lock.  If elapsed >
///    `HEARTBEAT_TIMEOUT` it *re-acquires a write lock* and re-reads the same
///    values.  If the generation has increased (heartbeat arrived between the
///    two reads) the offline transition is skipped.
///
/// 2. **Grace period** (option 4):
///    A node that fails the double-check is not immediately marked `Offline`.
///    Instead it is set to `PendingOffline` with a timestamp.  On the *next*
///    scanner sweep (or any heartbeat) the node either recovers or is finally
///    marked `Offline`.
pub struct LivenessScanner {
    map: HeartbeatMap,
}

impl LivenessScanner {
    /// Creates a scanner that operates on `map`.
    pub fn new(map: HeartbeatMap) -> Self {
        Self { map }
    }

    /// Checks liveness for every tracked node at the given instant.
    ///
    /// Lines 30–60 (issue reference): this is the scan-then-mark path.
    ///
    /// Returns the list of node IDs that were newly marked `Offline` during
    /// this sweep (useful for testing and observability).
    pub fn check_liveness(&self, now: Instant) -> Vec<u64> {
        // Phase 1: identify candidates under a read lock.
        //
        // We collect `(node_id, last_heartbeat, generation)` tuples for nodes
        // that appear to have timed out.  We deliberately release the read lock
        // before Phase 2 so that record_heartbeat() can make progress in the
        // meantime (important for fairness under high heartbeat rates).
        let candidates: Vec<(u64, Instant, u64)> = {
            let map = self.map.read().unwrap();
            map.iter()
                .filter_map(|(&node_id, state)| {
                    let elapsed = now.saturating_duration_since(state.last_heartbeat);
                    let gen = state.read_generation();

                    match state.status {
                        NodeStatus::Online if elapsed > HEARTBEAT_TIMEOUT => {
                            Some((node_id, state.last_heartbeat, gen))
                        }
                        NodeStatus::PendingOffline => {
                            // Also re-evaluate nodes already in the grace period.
                            Some((node_id, state.last_heartbeat, gen))
                        }
                        _ => None,
                    }
                })
                .collect()
        };

        if candidates.is_empty() {
            return Vec::new();
        }

        let mut newly_offline = Vec::new();

        // Phase 2: for each candidate, re-acquire a *write* lock and
        // double-check before mutating status.
        for (node_id, observed_hb, observed_gen) in candidates {
            let mut map = self.map.write().unwrap();

            let state = match map.get_mut(&node_id) {
                Some(s) => s,
                None => continue, // node was removed between phases
            };

            // Double-check 1: has the generation changed?
            // A higher generation means record_heartbeat() ran between our
            // Phase-1 read and now — the node is alive.
            let current_gen = state.read_generation();
            if current_gen != observed_gen {
                // Heartbeat arrived between the two reads.  Cancel any pending
                // offline transition and leave the node Online.
                state.status = NodeStatus::Online;
                state.pending_offline_since = None;
                continue;
            }

            // Double-check 2: re-read the timestamp under the write lock.
            let elapsed = now.saturating_duration_since(state.last_heartbeat);

            if elapsed <= HEARTBEAT_TIMEOUT {
                // Timestamp moved forward — heartbeat was recorded just before
                // we took the write lock.  Node is alive.
                state.status = NodeStatus::Online;
                state.pending_offline_since = None;
                continue;
            }

            // The node genuinely appears to have timed out.
            match state.status {
                NodeStatus::Online => {
                    // First detection: enter the grace period instead of
                    // immediately marking offline (option 4 of the blueprint).
                    state.status = NodeStatus::PendingOffline;
                    state.pending_offline_since = Some(now);
                    // Do NOT mark observed_hb as stale yet; preserve the
                    // reference for callers who track the map externally.
                    let _ = observed_hb; // suppress unused warning
                }
                NodeStatus::PendingOffline => {
                    // Check if the grace period has elapsed.
                    let grace_expired = state
                        .pending_offline_since
                        .map(|t| now.saturating_duration_since(t) >= OFFLINE_GRACE_PERIOD)
                        .unwrap_or(true);

                    if grace_expired {
                        state.status = NodeStatus::Offline;
                        newly_offline.push(node_id);
                    }
                    // else: still within grace period, do nothing.
                }
                NodeStatus::Offline => {
                    // Already offline — nothing to do.
                }
            }
        }

        newly_offline
    }
}
