use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    tokens: u32,
    last_update: Instant,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            tokens: 100,
            last_update: Instant::now(),
        }
    }

    pub fn consume(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_millis();
        let add_tokens = (elapsed / 600) as u32;

        if add_tokens > 0 {
            self.tokens = std::cmp::min(100, self.tokens + add_tokens);
            self.last_update += Duration::from_millis((add_tokens * 600) as u64);
        }

        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            false
        }
    }
}

pub struct RateLimiterManager {
    limiters: HashMap<String, RateLimiter>,
}

impl RateLimiterManager {
    pub fn new() -> Self {
        Self {
            limiters: HashMap::new(),
        }
    }

    pub fn check(&mut self, sender: &str) -> bool {
        let limiter = self.limiters.entry(sender.to_string()).or_insert_with(RateLimiter::new);
        limiter.consume()
    }
}
