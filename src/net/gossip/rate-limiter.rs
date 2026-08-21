use std::collections::HashMap;
use std::time::{Instant, Duration};

pub struct RateLimiter {
    peer_limits: HashMap<String, PeerLimit>,
}

struct PeerLimit {
    flagged_until: Option<Instant>,
    last_message_time: Option<Instant>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            peer_limits: HashMap::new(),
        }
    }

    pub fn flag_peer(&mut self, peer_id: String) {
        let entry = self.peer_limits.entry(peer_id).or_insert(PeerLimit {
            flagged_until: None,
            last_message_time: None,
        });
        entry.flagged_until = Some(Instant::now() + Duration::from_secs(60));
    }

    pub fn allow_message(&mut self, peer_id: String) -> bool {
        let now = Instant::now();
        let entry = self.peer_limits.entry(peer_id).or_insert(PeerLimit {
            flagged_until: None,
            last_message_time: None,
        });

        if let Some(flagged_until) = entry.flagged_until {
            if now < flagged_until {
                // Rate limit: 1 message per 10s
                if let Some(last_time) = entry.last_message_time {
                    if now.duration_since(last_time) < Duration::from_secs(10) {
                        return false;
                    }
                }
            } else {
                entry.flagged_until = None;
            }
        }

        entry.last_message_time = Some(now);
        true
    }
}
