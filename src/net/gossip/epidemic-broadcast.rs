use std::collections::HashMap;

use super::duplicate_filter::DuplicateFilter;
use super::fanout_controller::FanoutController;
use super::rate_limiter::RateLimiter;

const PUSHBACK_EVICTION_THRESHOLD: f64 = 0.5;
const PUSHBACK_TOP_N_SENDERS: usize = 5;
const MAX_MESSAGE_SIZE: usize = 1_048_576;

pub struct EpidemicBroadcast {
    pub fanout_controller: FanoutController,
    pub duplicate_filter: DuplicateFilter,
    pub rate_limiter: RateLimiter,
    pub peer_send_counts: HashMap<Vec<u8>, u64>,
    pub total_peers: usize,
    pub pushback_active: bool,
}

impl EpidemicBroadcast {
    pub fn new() -> Self {
        Self {
            fanout_controller: FanoutController::new(),
            duplicate_filter: DuplicateFilter::new(),
            rate_limiter: RateLimiter::new(),
            peer_send_counts: HashMap::new(),
            total_peers: 0,
            pushback_active: false,
        }
    }

    pub fn set_total_peers(&mut self, count: usize) {
        self.total_peers = count;
    }

    pub fn receive_message(
        &mut self,
        message_id: [u8; 32],
        sender: &[u8],
        message_size: usize,
    ) -> ReceiveResult<'_> {
        if message_size > MAX_MESSAGE_SIZE {
            return ReceiveResult::Rejected("message exceeds size limit");
        }

        if self.duplicate_filter.is_duplicate(message_id, sender) {
            if self.duplicate_filter.is_flagged(sender) {
                self.rate_limiter.flag_peer(sender);
                return ReceiveResult::FlaggedAmplifier;
            }
            return ReceiveResult::Duplicate;
        }

        self.rate_limiter.record_incoming(sender);

        if !self.rate_limiter.try_accept(sender) {
            return ReceiveResult::RateLimited;
        }

        let ratio = self.rate_limiter.amplification_ratio(sender);
        if ratio > 0.0 {
            self.fanout_controller
                .record_amplification_ratio(sender.to_vec(), ratio);
        }

        self.check_pushback();

        if self.pushback_active {
            let top_senders = self.top_prolific_senders(PUSHBACK_TOP_N_SENDERS);
            if top_senders.contains(&sender.to_vec()) {
                return ReceiveResult::PushbackRefused;
            }
        }

        ReceiveResult::Accepted
    }

    pub fn broadcast(&mut self, message_id: [u8; 32], peers: &[Vec<u8>]) -> Vec<Vec<u8>> {
        let fanout = self
            .fanout_controller
            .compute_fanout(self.total_peers.max(peers.len()));

        let selected = self.fanout_controller.select_peers(peers, fanout);

        for peer in &selected {
            self.fanout_controller
                .record_message(peer.clone(), 1, 1);
            *self.peer_send_counts.entry(peer.clone()).or_insert(0) += 1;

            self.duplicate_filter.is_duplicate(message_id, peer);
        }

        selected
    }

    pub fn broadcast_to_peers(
        &mut self,
        message_id: [u8; 32],
        peers: &[Vec<u8>],
        peer_map: &HashMap<Vec<u8>, Vec<Vec<u8>>>,
    ) -> BroadcastResult {
        let fanout = self.fanout_controller.compute_fanout(peers.len());
        let selected = self.fanout_controller.select_peers(peers, fanout);

        let mut total_forwarded = 0u64;
        let total_received = selected.len() as u64;

        for peer in &selected {
            let recipients = peer_map.get(peer).map(|r| r.as_slice()).unwrap_or(&[]);

            self.fanout_controller
                .record_message(peer.clone(), recipients.len() as u64, 1);
            *self.peer_send_counts.entry(peer.clone()).or_insert(0) += 1;

            self.duplicate_filter.is_duplicate(message_id, peer);

            for recipient in recipients {
                let result = self.receive_message(message_id, recipient, MAX_MESSAGE_SIZE);
                if matches!(result, ReceiveResult::Accepted | ReceiveResult::Duplicate) {
                    total_forwarded += 1;
                }
            }
        }

        self.check_pushback();

        BroadcastResult {
            forwarded_to: selected,
            fanout_used: fanout,
            total_forwarded,
            total_received,
        }
    }

    pub fn check_pushback(&mut self) -> bool {
        let eviction_rate = self.duplicate_filter.eviction_rate();

        if eviction_rate > PUSHBACK_EVICTION_THRESHOLD {
            self.pushback_active = true;
            true
        } else {
            self.pushback_active = false;
            false
        }
    }

    pub fn top_prolific_senders(&self, n: usize) -> Vec<Vec<u8>> {
        let mut senders: Vec<(u64, &Vec<u8>)> = self
            .peer_send_counts
            .iter()
            .map(|(k, v)| (*v, k))
            .collect();

        senders.sort_by(|a, b| b.0.cmp(&a.0));
        senders.into_iter().take(n).map(|(_, p)| p.clone()).collect()
    }

    pub fn amplification_factor(&self) -> f64 {
        let total_out: u64 = self
            .rate_limiter
            .flagged_peers()
            .iter()
            .filter_map(|p| {
                self.peer_send_counts.get(p)
            })
            .sum();

        let total_in: u64 = self
            .rate_limiter
            .flagged_peers()
            .iter()
            .filter_map(|p| {
                Some(
                    self.rate_limiter.amplification_ratio(p) as u64,
                )
            })
            .sum();

        if total_in == 0 {
            return 0.0;
        }
        total_out as f64 / total_in as f64
    }

    pub fn reset_pushback(&mut self) {
        self.pushback_active = false;
    }
}

#[derive(Debug, PartialEq)]
pub enum ReceiveResult<'a> {
    Accepted,
    Duplicate,
    RateLimited,
    FlaggedAmplifier,
    PushbackRefused,
    Rejected(&'a str),
}

#[derive(Debug)]
pub struct BroadcastResult {
    pub forwarded_to: Vec<Vec<u8>>,
    pub fanout_used: usize,
    pub total_forwarded: u64,
    pub total_received: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_id(n: u8) -> [u8; 32] {
        let mut id = [0u8; 32];
        id[0] = n;
        id
    }

    #[test]
    fn test_receive_new_message() {
        let mut broadcast = EpidemicBroadcast::new();
        let result = broadcast.receive_message(make_id(1), b"peer-a", 1024);
        assert_eq!(result, ReceiveResult::Accepted);
    }

    #[test]
    fn test_receive_duplicate_message() {
        let mut broadcast = EpidemicBroadcast::new();
        broadcast.receive_message(make_id(1), b"peer-a", 1024);
        let result = broadcast.receive_message(make_id(1), b"peer-b", 1024);
        assert_eq!(result, ReceiveResult::Duplicate);
    }

    #[test]
    fn test_reject_oversized_message() {
        let mut broadcast = EpidemicBroadcast::new();
        let result = broadcast.receive_message(make_id(1), b"peer-a", MAX_MESSAGE_SIZE + 1);
        assert_eq!(result, ReceiveResult::Rejected("message exceeds size limit"));
    }

    #[test]
    fn test_broadcast_selects_peers() {
        let mut broadcast = EpidemicBroadcast::new();
        broadcast.set_total_peers(100);

        let peers: Vec<Vec<u8>> = (0..50).map(|i| vec![i as u8]).collect();
        let selected = broadcast.broadcast(make_id(1), &peers);

        assert!(!selected.is_empty());
        assert!(selected.len() <= 20);
    }

    #[test]
    fn test_fanout_respects_limits() {
        let mut broadcast = EpidemicBroadcast::new();
        broadcast.set_total_peers(100);

        let peers: Vec<Vec<u8>> = (0..100).map(|i| vec![i as u8]).collect();
        let result = broadcast.broadcast_to_peers(make_id(1), &peers, &HashMap::new());

        assert!(result.fanout_used >= 3);
        assert!(result.fanout_used <= 20);
    }

    #[test]
    fn test_amplifier_gets_flagged() {
        let mut broadcast = EpidemicBroadcast::new();

        broadcast.receive_message(make_id(0), b"amplifier", 1024);

        for i in 1..=60 {
            let id = make_id(i as u8);
            broadcast.receive_message(id, b"amplifier", 1024);
            broadcast.receive_message(id, b"amplifier", 1024);
        }

        assert!(broadcast.rate_limiter.is_flagged(b"amplifier"));
    }

    #[test]
    fn test_pushback_activates_on_high_eviction() {
        let mut broadcast = EpidemicBroadcast::new();

        broadcast.duplicate_filter.eviction_count = 60;
        broadcast.duplicate_filter.total_insertions = 100;

        assert!(broadcast.check_pushback());
        assert!(broadcast.pushback_active);
    }

    #[test]
    fn test_pushback_refuses_top_senders() {
        let mut broadcast = EpidemicBroadcast::new();

        broadcast.duplicate_filter.eviction_count = 60;
        broadcast.duplicate_filter.total_insertions = 100;
        broadcast.check_pushback();

        for i in 0..10 {
            let peer = vec![i];
            *broadcast.peer_send_counts.entry(peer).or_insert(0) = 100 - i as u64;
        }

        let result = broadcast.receive_message(make_id(1), &[0], 1024);
        assert_eq!(result, ReceiveResult::PushbackRefused);
    }


}
