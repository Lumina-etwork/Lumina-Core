use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use super::state::{NodeLivenessState, NodeStatus};

/// Opaque node identifier.
pub type NodeId = u64;

/// Duration constants as per the issue specification.
pub const HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);
pub const HEARTBEAT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);
/// Grace period before a timed-out node is permanently marked offline.
pub const OFFLINE_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_secs(10);

/// Shared heartbeat table, keyed by [`NodeId`].
///
/// All per-node state lives inside [`NodeLivenessState`]; the `Arc<RwLock<…>>`
/// wrapper allows the table to be shared between the heartbeat receiver path
/// and the background scanner without data races.
pub type HeartbeatMap = Arc<RwLock<HashMap<NodeId, NodeLivenessState>>>;

/// Manages per-node heartbeat timestamps and liveness status.
#[derive(Clone)]
pub struct HeartbeatTracker {
    map: HeartbeatMap,
}

impl HeartbeatTracker {
    /// Creates a new, empty tracker.
    pub fn new() -> Self {
        Self {
            map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns a clone of the shared heartbeat map for use by the scanner.
    pub fn shared_map(&self) -> HeartbeatMap {
        Arc::clone(&self.map)
    }

    /// Records a heartbeat for `node_id` at the supplied instant.
    ///
    /// Line 45 (issue reference): this is the write path that races with the
    /// scanner.  Because we take a *write* lock on the individual node entry
    /// (via the whole-map write lock) and bump the generation counter atomically
    /// inside [`NodeLivenessState::record`], the scanner's double-check can
    /// detect that a heartbeat arrived between its two generation reads and
    /// suppress the false-positive offline transition.
    pub fn record_heartbeat(&self, node_id: NodeId, now: Instant) {
        let mut map = self.map.write().unwrap();
        map.entry(node_id)
            .and_modify(|s| s.record(now))
            .or_insert_with(|| NodeLivenessState::new(now));
    }

    /// Returns the current status of `node_id`, or `None` if it is unknown.
    pub fn status(&self, node_id: NodeId) -> Option<NodeStatus> {
        let map = self.map.read().unwrap();
        map.get(&node_id).map(|s| s.status)
    }

    /// Returns the number of tracked nodes.
    pub fn node_count(&self) -> usize {
        self.map.read().unwrap().len()
    }
}

impl Default for HeartbeatTracker {
    fn default() -> Self {
        Self::new()
    }
}
