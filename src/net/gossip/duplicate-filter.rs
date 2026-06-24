use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

const DUPLICATE_BUFFER_CAPACITY: usize = 10_000;
const DUPLICATE_THRESHOLD: usize = 50;
const DUPLICATE_WINDOW: Duration = Duration::from_secs(1);

#[derive(Clone, Debug)]
pub struct PeerDuplicateCount {
    pub duplicate_count: usize,
    pub window_start: Instant,
    pub flagged: bool,
}

pub struct DuplicateFilter {
    message_ids: VecDeque<[u8; 32]>,
    id_set: std::collections::HashSet<[u8; 32]>,
    peer_duplicates: HashMap<Vec<u8>, PeerDuplicateCount>,
    flagged_peers: std::collections::HashSet<Vec<u8>>,
    pub eviction_count: usize,
    pub total_insertions: usize,
}

impl DuplicateFilter {
    pub fn new() -> Self {
        Self {
            message_ids: VecDeque::with_capacity(DUPLICATE_BUFFER_CAPACITY),
            id_set: std::collections::HashSet::new(),
            peer_duplicates: HashMap::new(),
            flagged_peers: std::collections::HashSet::new(),
            eviction_count: 0,
            total_insertions: 0,
        }
    }

    pub fn is_duplicate(&mut self, message_id: [u8; 32], peer_id: &[u8]) -> bool {
        if self.id_set.contains(&message_id) {
            self.record_duplicate(peer_id);
            return true;
        }
        self.insert(message_id, peer_id);
        false
    }

    fn insert(&mut self, message_id: [u8; 32], _peer_id: &[u8]) {
        if self.message_ids.len() >= DUPLICATE_BUFFER_CAPACITY {
            if let Some(oldest) = self.message_ids.pop_front() {
                self.id_set.remove(&oldest);
                self.eviction_count += 1;
            }
        }

        self.message_ids.push_back(message_id);
        self.id_set.insert(message_id);
        self.total_insertions += 1;
    }

    fn record_duplicate(&mut self, peer_id: &[u8]) {
        let now = Instant::now();
        let entry = self
            .peer_duplicates
            .entry(peer_id.to_vec())
            .or_insert(PeerDuplicateCount {
                duplicate_count: 0,
                window_start: now,
                flagged: false,
            });

        if now.duration_since(entry.window_start) > DUPLICATE_WINDOW {
            entry.duplicate_count = 0;
            entry.window_start = now;
        }

        entry.duplicate_count += 1;

        if entry.duplicate_count > DUPLICATE_THRESHOLD && !entry.flagged {
            entry.flagged = true;
            self.flagged_peers.insert(peer_id.to_vec());
        }
    }

    pub fn is_flagged(&self, peer_id: &[u8]) -> bool {
        self.flagged_peers.contains(peer_id)
    }

    pub fn flagged_peers(&self) -> Vec<Vec<u8>> {
        self.flagged_peers.iter().cloned().collect()
    }

    pub fn eviction_rate(&self) -> f64 {
        if self.total_insertions == 0 {
            return 0.0;
        }
        self.eviction_count as f64 / self.total_insertions as f64
    }

    pub fn clear_flagged(&mut self, peer_id: &[u8]) {
        self.flagged_peers.remove(peer_id);
        if let Some(entry) = self.peer_duplicates.get_mut(peer_id) {
            entry.flagged = false;
            entry.duplicate_count = 0;
            entry.window_start = Instant::now();
        }
    }

    pub fn len(&self) -> usize {
        self.message_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.message_ids.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_id(n: u8) -> [u8; 32] {
        let mut id = [0u8; 32];
        id[0] = n;
        id
    }

    fn make_id_from(n: usize) -> [u8; 32] {
        let mut id = [0u8; 32];
        let bytes = n.to_le_bytes();
        id[..8].copy_from_slice(&bytes);
        id
    }

    #[test]
    fn test_first_seen_not_duplicate() {
        let mut filter = DuplicateFilter::new();
        assert!(!filter.is_duplicate(make_id(1), b"peer-a"));
    }

    #[test]
    fn test_same_id_from_same_peer_is_duplicate() {
        let mut filter = DuplicateFilter::new();
        filter.is_duplicate(make_id(1), b"peer-a");
        assert!(filter.is_duplicate(make_id(1), b"peer-a"));
    }

    #[test]
    fn test_same_id_from_different_peer_is_duplicate() {
        let mut filter = DuplicateFilter::new();
        filter.is_duplicate(make_id(1), b"peer-a");
        assert!(filter.is_duplicate(make_id(1), b"peer-b"));
    }

    #[test]
    fn test_peer_gets_flagged_after_threshold() {
        let mut filter = DuplicateFilter::new();
        let peer = b"amplifier";

        filter.is_duplicate(make_id(0), peer);

        for i in 1..=DUPLICATE_THRESHOLD + 1 {
            let id = make_id(i as u8);
            filter.is_duplicate(id, peer);
            filter.is_duplicate(id, peer);
        }

        assert!(filter.is_flagged(peer));
    }

    #[test]
    fn test_lru_eviction() {
        let mut filter = DuplicateFilter::new();

        // fill to capacity checking that first DUPLICATE_BUFFER_CAPACITY inserts do not evict
        for i in 0..DUPLICATE_BUFFER_CAPACITY {
            let id = make_id_from(i);
            assert!(!filter.is_duplicate(id, b"peer"));
        }

        assert_eq!(filter.len(), DUPLICATE_BUFFER_CAPACITY);
        assert_eq!(filter.eviction_count, 0);

        // one more triggers eviction
        let overflow_id = make_id_from(DUPLICATE_BUFFER_CAPACITY);
        assert!(!filter.is_duplicate(overflow_id, b"peer"));

        assert_eq!(filter.len(), DUPLICATE_BUFFER_CAPACITY);
        assert!(filter.eviction_count >= 1,
            "expected eviction, got evictions={}, total_insertions={}",
            filter.eviction_count, filter.total_insertions);
    }

    #[test]
    fn test_window_resets_after_one_second() {
        let mut filter = DuplicateFilter::new();

        filter.is_duplicate(make_id(1), b"peer");
        assert!(!filter.is_duplicate(make_id(2), b"peer"));

        if let Some(entry) = filter.peer_duplicates.get_mut(&b"peer".to_vec()) {
            entry.window_start = Instant::now() - DUPLICATE_WINDOW - Duration::from_millis(1);
            entry.duplicate_count = 45;
        }

        filter.is_duplicate(make_id(1), b"peer");
        assert!(!filter.is_flagged(b"peer"));
    }

    #[test]
    fn test_eviction_rate() {
        let mut filter = DuplicateFilter::new();
        assert!((filter.eviction_rate() - 0.0).abs() < 0.001);

        filter.eviction_count = 50;
        filter.total_insertions = 100;
        assert!((filter.eviction_rate() - 0.5).abs() < 0.001);
    }
}
