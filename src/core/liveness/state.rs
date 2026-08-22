use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Status of a node as seen by the liveness subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Online,
    /// Node has not been heard from within the timeout window but is in the
    /// grace period before being declared fully offline.
    PendingOffline,
    Offline,
}

/// All per-node liveness state, kept together so reads and writes can be done
/// under a single lock acquisition.
#[derive(Debug)]
pub struct NodeLivenessState {
    /// Wall-clock time of the most recent heartbeat received from this node.
    pub last_heartbeat: Instant,
    /// Monotonically increasing counter; incremented by every heartbeat.
    /// The scanner reads this before and after its timeout check; a change
    /// means a heartbeat arrived between the two reads.
    pub generation: AtomicU64,
    /// Current liveness status of the node.
    pub status: NodeStatus,
    /// When `Some`, the node entered the `PendingOffline` state at this
    /// instant. If a heartbeat arrives before the grace period elapses the
    /// entry is cleared and the node stays `Online`.
    pub pending_offline_since: Option<Instant>,
}

impl NodeLivenessState {
    /// Creates a new entry with `last_heartbeat = now` and `status = Online`.
    pub fn new(now: Instant) -> Self {
        Self {
            last_heartbeat: now,
            generation: AtomicU64::new(0),
            status: NodeStatus::Online,
            pending_offline_since: None,
        }
    }

    /// Records a fresh heartbeat, bumping the generation counter.
    pub fn record(&mut self, now: Instant) {
        self.last_heartbeat = now;
        self.generation.fetch_add(1, Ordering::Release);
        self.status = NodeStatus::Online;
        self.pending_offline_since = None;
    }

    /// Reads the current generation value for double-check use by the scanner.
    pub fn read_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}
