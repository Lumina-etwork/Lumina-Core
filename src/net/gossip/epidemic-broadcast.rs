use std::collections::HashMap;
use crate::net::gossip::fanout_controller::FanoutController;
use crate::net::gossip::duplicate_filter::DuplicateFilter;
use crate::net::gossip::rate_limiter::RateLimiter;

pub struct EpidemicBroadcast {
    fanout_controller: FanoutController,
    duplicate_filter: DuplicateFilter,
    rate_limiter: RateLimiter,
    message_counts: HashMap<String, usize>, // messages sent per peer
}

impl EpidemicBroadcast {
    pub fn new(network_size: usize) -> Self {
        Self {
            fanout_controller: FanoutController::new(network_size),
            duplicate_filter: DuplicateFilter::new(),
            rate_limiter: RateLimiter::new(),
            message_counts: HashMap::new(),
        }
    }

    pub fn receive_message(&mut self, peer_id: String, message_id: String, amplification_ratio: u32) -> Option<u32> {
        if !self.rate_limiter.allow_message(peer_id.clone()) {
            return None; // Message rejected by rate limiter
        }

        // Pushback mechanism
        if self.duplicate_filter.eviction_rate() > 0.5 {
            let mut prolific_senders: Vec<_> = self.message_counts.iter().collect();
            prolific_senders.sort_by(|a, b| b.1.cmp(a.1));
            
            let top_5: Vec<String> = prolific_senders.into_iter().take(5).map(|(k, _)| k.clone()).collect();
            if top_5.contains(&peer_id) {
                return None; // Refuse new messages from top 5 prolific senders
            }
        }

        *self.message_counts.entry(peer_id.clone()).or_insert(0) += 1;

        if !self.duplicate_filter.insert(message_id.clone()) {
            let is_amplifier = self.duplicate_filter.record_duplicate_from_peer(peer_id.clone());
            if is_amplifier {
                self.rate_limiter.flag_peer(peer_id);
            }
            return None; // Message is a duplicate
        }

        // Calculate fanout for the current message
        let fanout = self.fanout_controller.compute_fanout(amplification_ratio);
        Some(fanout)
    }
}
