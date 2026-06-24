use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

const MAX_TOKENS: u64 = 60;
const REFILL_RATE: u64 = 1;

pub struct RateLimitedSigner {
    tokens: AtomicU64,
    last_refill: parking_lot::Mutex<Instant>,
}

pub struct SignResult {
    pub signature: Option<[u8; 64]>,
    pub rate_limited: bool,
    pub retry_after_secs: u64,
}

impl RateLimitedSigner {
    pub fn new() -> Self {
        Self {
            tokens: AtomicU64::new(MAX_TOKENS),
            last_refill: parking_lot::Mutex::new(Instant::now()),
        }
    }

    pub fn try_sign(&self, _message: &[u8]) -> SignResult {
        self.refill();
        let current = self.tokens.load(Ordering::Relaxed);
        if current == 0 {
            return SignResult {
                signature: None,
                rate_limited: true,
                retry_after_secs: 1,
            };
        }
        self.tokens.store(current - 1, Ordering::Relaxed);
        SignResult {
            signature: Some([0u8; 64]),
            rate_limited: false,
            retry_after_secs: 0,
        }
    }

    fn refill(&self) {
        let mut last = self.last_refill.lock();
        let now = Instant::now();
        let elapsed = now.duration_since(*last).as_secs();
        if elapsed > 0 {
            let new_tokens = self.tokens.load(Ordering::Relaxed) + elapsed * REFILL_RATE;
            self.tokens.store(new_tokens.min(MAX_TOKENS), Ordering::Relaxed);
            *last = now;
        }
    }

    pub fn available_tokens(&self) -> u64 {
        self.tokens.load(Ordering::Relaxed)
    }
}

pub struct RenewalMetrics {
    pub rate_limited_renewals: AtomicU64,
    pub renewal_queue_depth: AtomicU64,
}

impl RenewalMetrics {
    pub const fn new() -> Self {
        Self {
            rate_limited_renewals: AtomicU64::new(0),
            renewal_queue_depth: AtomicU64::new(0),
        }
    }

    pub fn record_rate_limited(&self) {
        self.rate_limited_renewals.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_queue_depth(&self, depth: u64) {
        self.renewal_queue_depth.store(depth, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> (u64, u64) {
        (
            self.rate_limited_renewals.load(Ordering::Relaxed),
            self.renewal_queue_depth.load(Ordering::Relaxed),
        )
    }
}

pub static RENEWAL_METRICS: RenewalMetrics = RenewalMetrics::new();
