/// Adaptive token-bucket backpressure.
///
/// When the p90 lock hold time reported by `StripedPool` exceeds 10 μs,
/// new allocations are throttled by draining the bucket faster.
///
/// - Normal mode  (p90 ≤ 10 μs): refill at `max_rate` tokens/sec.
/// - Throttled mode (p90 > 10 μs): refill at `max_rate * throttle_factor`.
/// - `acquire()` returns immediately if a token is available, else returns
///   `Err(BackpressureError::Throttled)` so callers can back off.
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

const P90_THRESHOLD_NS: u64 = 10_000; // 10 μs
const THROTTLE_FACTOR: f64 = 0.20;    // 20% of normal rate when throttled

/// Shared inner state — kept in an Arc so the pool and the bucket
/// can both hold a reference.
struct Inner {
    /// Token count scaled by 2^16 to allow fractional tokens without floats.
    tokens_fp: AtomicU64,
    /// Capacity in full tokens.
    capacity: u64,
    /// Refill rate in full tokens per nanosecond × 2^16.
    refill_fp_per_ns: AtomicU64,
    /// Normal refill rate (pre-computed).
    normal_fp_per_ns: u64,
    /// Throttled refill rate (pre-computed).
    throttled_fp_per_ns: u64,
    /// Whether the bucket is currently in throttle mode.
    throttled: AtomicBool,
    /// Last refill timestamp in nanoseconds since an arbitrary epoch.
    last_refill_ns: AtomicU64,
    /// Reference epoch (used to convert Instant to u64).
    epoch: Instant,
}

pub struct TokenBucket {
    inner: Arc<Inner>,
}

#[derive(Debug, PartialEq)]
pub enum BackpressureError {
    /// No tokens available — caller should back off.
    Throttled,
}

impl TokenBucket {
    /// Create a new bucket with `capacity` tokens and `max_rate` tokens/sec.
    pub fn new(capacity: u64, max_rate_per_sec: u64) -> Self {
        // Convert tokens/sec → tokens/ns × 2^16 fixed-point
        let normal_fp = (max_rate_per_sec << 16) / 1_000_000_000;
        let throttled_fp = ((max_rate_per_sec as f64 * THROTTLE_FACTOR) as u64) << 16
            / 1_000_000_000;

        let epoch = Instant::now();
        Self {
            inner: Arc::new(Inner {
                tokens_fp: AtomicU64::new(capacity << 16),
                capacity,
                refill_fp_per_ns: AtomicU64::new(normal_fp),
                normal_fp_per_ns: normal_fp,
                throttled_fp_per_ns: throttled_fp,
                throttled: AtomicBool::new(false),
                last_refill_ns: AtomicU64::new(0),
                epoch,
            }),
        }
    }

    /// Try to consume one token.  Refills first based on elapsed time.
    /// Returns `Ok(())` if a token was available, `Err(Throttled)` otherwise.
    pub fn acquire(&self) -> Result<(), BackpressureError> {
        let inner = &self.inner;
        let now_ns = inner.epoch.elapsed().as_nanos() as u64;

        // Refill
        let last = inner.last_refill_ns.load(Ordering::Relaxed);
        let elapsed = now_ns.saturating_sub(last);
        if elapsed > 0 {
            // CAS to ensure only one thread does the refill window.
            if inner.last_refill_ns.compare_exchange(
                last, now_ns, Ordering::AcqRel, Ordering::Relaxed
            ).is_ok() {
                let rate = inner.refill_fp_per_ns.load(Ordering::Relaxed);
                let add = elapsed.saturating_mul(rate);
                let cap = inner.capacity << 16;
                let cur = inner.tokens_fp.load(Ordering::Relaxed);
                let new_tokens = (cur + add).min(cap);
                inner.tokens_fp.store(new_tokens, Ordering::Relaxed);
            }
        }

        // Try to consume one full token (1 << 16 in fixed-point).
        let one = 1u64 << 16;
        let cur = inner.tokens_fp.load(Ordering::Relaxed);
        if cur < one {
            return Err(BackpressureError::Throttled);
        }
        // Optimistic subtract — another thread may race; that's acceptable
        // as a minor over-admission rather than a blocking CAS loop.
        inner.tokens_fp.fetch_sub(one, Ordering::Relaxed);
        Ok(())
    }

    /// Update throttle mode based on the current p90 hold time.
    /// Call this periodically (e.g., every N operations) from any thread.
    pub fn update_throttle(&self, p90_ns: u64) {
        let inner = &self.inner;
        let should_throttle = p90_ns > P90_THRESHOLD_NS;
        let was_throttled = inner.throttled.swap(should_throttle, Ordering::Relaxed);
        if should_throttle != was_throttled {
            let rate = if should_throttle {
                inner.throttled_fp_per_ns
            } else {
                inner.normal_fp_per_ns
            };
            inner.refill_fp_per_ns.store(rate, Ordering::Relaxed);
        }
    }

    /// Whether the bucket is currently in throttle mode.
    pub fn is_throttled(&self) -> bool {
        self.inner.throttled.load(Ordering::Relaxed)
    }
}
