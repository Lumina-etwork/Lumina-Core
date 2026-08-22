use std::collections::{HashMap, HashSet};
use std::time::{Instant, Duration};

const MAX_DUPLICATES_PER_SEC: usize = 50;
const DUPLICATE_BUFFER_LIMIT: usize = 10000;

pub struct DuplicateFilter {
    seen_messages: HashSet<String>,
    message_order: Vec<String>, // simplified LRU
    peer_stats: HashMap<String, PeerDuplicateStats>,
    pub evictions: usize,
    pub total_inserts: usize,
}

struct PeerDuplicateStats {
    count: usize,
    window_start: Instant,
}

impl DuplicateFilter {
    pub fn new() -> Self {
        Self {
            seen_messages: HashSet::new(),
            message_order: Vec::new(),
            peer_stats: HashMap::new(),
            evictions: 0,
            total_inserts: 0,
        }
    }

    pub fn insert(&mut self, message_id: String) -> bool {
        self.total_inserts += 1;
        if self.seen_messages.insert(message_id.clone()) {
            self.message_order.push(message_id);
            if self.seen_messages.len() > DUPLICATE_BUFFER_LIMIT {
                let oldest = self.message_order.remove(0);
                self.seen_messages.remove(&oldest);
                self.evictions += 1;
            }
            true // new message
        } else {
            false // duplicate
        }
    }

    pub fn record_duplicate_from_peer(&mut self, peer_id: String) -> bool {
        let now = Instant::now();
        let stats = self.peer_stats.entry(peer_id.clone()).or_insert(PeerDuplicateStats {
            count: 0,
            window_start: now,
        });

        if now.duration_since(stats.window_start) > Duration::from_secs(1) {
            stats.count = 1;
            stats.window_start = now;
        } else {
            stats.count += 1;
        }

        stats.count > MAX_DUPLICATES_PER_SEC
    }

    pub fn eviction_rate(&self) -> f64 {
        if self.total_inserts == 0 {
            0.0
        } else {
            self.evictions as f64 / self.total_inserts as f64
        }
    }
}
