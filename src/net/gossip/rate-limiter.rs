use std::collections::HashMap;
use std::time::{Duration, Instant};

const NORMAL_RATE_LIMIT: Duration = Duration::from_millis(100);
const PENALTY_RATE_LIMIT: Duration = Duration::from_secs(10);
const PENALTY_DURATION: Duration = Duration::from_secs(60);
const AMPLIFICATION_THRESHOLD: f64 = 100.0;

#[derive(Clone, Debug)]
pub struct PeerRateState {
    pub last_allowed: Instant,
    pub penalty_until: Option<Instant>,
    pub messages_in: u64,
    pub messages_out: u64,
    pub is_flagged: bool,
}

pub struct RateLimiter {
    peers: HashMap<Vec<u8>, PeerRateState>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    pub fn try_accept(&mut self, peer_id: &[u8]) -> bool {
        let now = Instant::now();
        let state = self
            .peers
            .entry(peer_id.to_vec())
            .or_insert(PeerRateState {
                last_allowed: now - NORMAL_RATE_LIMIT - Duration::from_secs(1),
                penalty_until: None,
                messages_in: 0,
                messages_out: 0,
                is_flagged: false,
            });

        if let Some(penalty_end) = state.penalty_until {
            if now < penalty_end {
                if now.duration_since(state.last_allowed) < PENALTY_RATE_LIMIT {
                    return false;
                }
            } else {
                state.penalty_until = None;
                state.is_flagged = false;
            }
        } else {
            if now.duration_since(state.last_allowed) < NORMAL_RATE_LIMIT {
                return false;
            }
        }

        state.last_allowed = now;
        true
    }

    pub fn record_incoming(&mut self, peer_id: &[u8]) {
        let now = Instant::now();
        let state = self
            .peers
            .entry(peer_id.to_vec())
            .or_insert(PeerRateState {
                last_allowed: now - NORMAL_RATE_LIMIT - Duration::from_secs(1),
                penalty_until: None,
                messages_in: 0,
                messages_out: 0,
                is_flagged: false,
            });
        state.messages_in += 1;
    }

    pub fn record_outgoing(&mut self, peer_id: &[u8]) {
        let now = Instant::now();
        let state = self
            .peers
            .entry(peer_id.to_vec())
            .or_insert(PeerRateState {
                last_allowed: now - NORMAL_RATE_LIMIT - Duration::from_secs(1),
                penalty_until: None,
                messages_in: 0,
                messages_out: 0,
                is_flagged: false,
            });
        state.messages_out += 1;
    }

    pub fn allow_message(&mut self, peer_id: &[u8]) -> bool {
        let ok = self.try_accept(peer_id);
        if ok {
            self.record_outgoing(peer_id);
        }
        ok
    }

    pub fn flag_peer(&mut self, peer_id: &[u8]) {
        let now = Instant::now();
        let state = self
            .peers
            .entry(peer_id.to_vec())
            .or_insert(PeerRateState {
                last_allowed: now - PENALTY_RATE_LIMIT - Duration::from_secs(1),
                penalty_until: None,
                messages_in: 0,
                messages_out: 0,
                is_flagged: false,
            });

        state.is_flagged = true;
        state.penalty_until = Some(now + PENALTY_DURATION);
    }

    pub fn check_amplification(&self, peer_id: &[u8]) -> bool {
        self.peers.get(peer_id).map_or(false, |state| {
            if state.messages_in == 0 {
                return false;
            }
            let ratio = state.messages_out as f64 / state.messages_in as f64;
            ratio > AMPLIFICATION_THRESHOLD
        })
    }

    pub fn amplification_ratio(&self, peer_id: &[u8]) -> f64 {
        self.peers
            .get(peer_id)
            .map(|s| {
                if s.messages_in == 0 {
                    0.0
                } else {
                    s.messages_out as f64 / s.messages_in as f64
                }
            })
            .unwrap_or(0.0)
    }

    pub fn is_penalized(&self, peer_id: &[u8]) -> bool {
        self.peers.get(peer_id).map_or(false, |s| {
            s.penalty_until.map_or(false, |end| Instant::now() < end)
        })
    }

    pub fn is_flagged(&self, peer_id: &[u8]) -> bool {
        self.peers.get(peer_id).map_or(false, |s| s.is_flagged)
    }

    pub fn clear_peer(&mut self, peer_id: &[u8]) {
        self.peers.remove(peer_id);
    }

    pub fn flagged_peers(&self) -> Vec<Vec<u8>> {
        self.peers
            .iter()
            .filter(|(_, s)| s.is_flagged)
            .map(|(k, _)| k.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_allow_message_normal_rate() {
        let mut limiter = RateLimiter::new();
        assert!(limiter.allow_message(b"peer-a"));
    }

    #[test]
    fn test_block_message_within_normal_interval() {
        let mut limiter = RateLimiter::new();
        assert!(limiter.allow_message(b"peer-a"));
        assert!(!limiter.allow_message(b"peer-a"));
    }

    #[test]
    fn test_allow_after_normal_interval() {
        let mut limiter = RateLimiter::new();
        assert!(limiter.allow_message(b"peer-a"));
        thread::sleep(NORMAL_RATE_LIMIT + Duration::from_millis(10));
        assert!(limiter.allow_message(b"peer-a"));
    }

    #[test]
    fn test_flagged_peer_penalty_rate() {
        let mut limiter = RateLimiter::new();
        limiter.flag_peer(b"bad-peer");
        assert!(limiter.is_flagged(b"bad-peer"));
        assert!(limiter.is_penalized(b"bad-peer"));

        assert!(limiter.allow_message(b"bad-peer"));
        assert!(!limiter.allow_message(b"bad-peer"));
    }

    #[test]
    fn test_amplification_detection() {
        let mut limiter = RateLimiter::new();

        limiter.record_incoming(b"amplifier");
        for _ in 0..200 {
            limiter.record_outgoing(b"amplifier");
        }

        assert!(limiter.check_amplification(b"amplifier"));
    }

    #[test]
    fn test_below_threshold_not_flagged() {
        let mut limiter = RateLimiter::new();

        limiter.record_incoming(b"normal");
        limiter.record_outgoing(b"normal");

        assert!(!limiter.check_amplification(b"normal"));
    }

    #[test]
    fn test_independent_peer_tracking() {
        let mut limiter = RateLimiter::new();

        limiter.flag_peer(b"bad");
        assert!(limiter.is_penalized(b"bad"));
        assert!(!limiter.is_penalized(b"good"));
    }

    #[test]
    fn test_amplification_ratio_calculation() {
        let mut limiter = RateLimiter::new();

        limiter.record_incoming(b"peer");
        for _ in 0..10 {
            limiter.record_outgoing(b"peer");
        }

        let ratio = limiter.amplification_ratio(b"peer");
        assert!((ratio - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_try_accept_respects_rate_limit() {
        let mut limiter = RateLimiter::new();
        assert!(limiter.try_accept(b"peer"));
        assert!(!limiter.try_accept(b"peer"));
    }

    #[test]
    fn test_allow_message_counts_outgoing() {
        let mut limiter = RateLimiter::new();
        assert!(limiter.allow_message(b"peer"));
        assert_eq!(limiter.amplification_ratio(b"peer"), 0.0);

        limiter.record_incoming(b"peer");
        assert!((limiter.amplification_ratio(b"peer") - 1.0).abs() < 0.001);
    }
}
