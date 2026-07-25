use crate::rate_limiter::metrics::RateLimitMetrics;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

const FP_SCALE: u64 = 1 << 16;

struct Inner {
    tokens_fp: AtomicU64,
    capacity: u64,
    refill_rate_per_ns_fp: u64,
    last_refill_ns: AtomicU64,
    epoch: Instant,
}

pub struct TokenBucket {
    inner: Arc<Inner>,
}

pub struct TenantBucket {
    inner: Arc<Inner>,
    tenant_id: String,
}

pub struct PerTenantRateLimiter {
    buckets: HashMap<String, TenantBucket>,
    default_capacity: u64,
    default_refill_per_sec: u64,
    metrics: RateLimitMetrics,
}

impl TokenBucket {
    pub fn new(capacity: u64, refill_per_sec: u64) -> Self {
        let rate_fp = (refill_per_sec << 16) / 1_000_000_000;
        let epoch = Instant::now();
        Self {
            inner: Arc::new(Inner {
                tokens_fp: AtomicU64::new(capacity << 16),
                capacity,
                refill_rate_per_ns_fp: rate_fp,
                last_refill_ns: AtomicU64::new(0),
                epoch,
            }),
        }
    }

    pub fn acquire(&self) -> Result<(), RateLimitError> {
        let inner = &self.inner;
        let now_ns = inner.epoch.elapsed().as_nanos() as u64;

        let last = inner.last_refill_ns.load(Ordering::Relaxed);
        let elapsed = now_ns.saturating_sub(last);
        if elapsed > 0 {
            if inner
                .last_refill_ns
                .compare_exchange(last, now_ns, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                let add = elapsed.saturating_mul(inner.refill_rate_per_ns_fp);
                let cap = inner.capacity << 16;
                let cur = inner.tokens_fp.load(Ordering::Relaxed);
                let new_tokens = (cur + add).min(cap);
                inner.tokens_fp.store(new_tokens, Ordering::Relaxed);
            }
        }

        let one = 1u64 << 16;
        let cur = inner.tokens_fp.load(Ordering::Relaxed);
        if cur < one {
            return Err(RateLimitError::RateLimited);
        }
        inner.tokens_fp.fetch_sub(one, Ordering::Relaxed);
        Ok(())
    }

    pub fn available_tokens(&self) -> u64 {
        self.inner.tokens_fp.load(Ordering::Relaxed) >> 16
    }

    pub fn capacity(&self) -> u64 {
        self.inner.capacity
    }
}

impl TenantBucket {
    pub fn new(tenant_id: String, capacity: u64, refill_per_sec: u64) -> Self {
        let rate_fp = (refill_per_sec << 16) / 1_000_000_000;
        let epoch = Instant::now();
        Self {
            inner: Arc::new(Inner {
                tokens_fp: AtomicU64::new(capacity << 16),
                capacity,
                refill_rate_per_ns_fp: rate_fp,
                last_refill_ns: AtomicU64::new(0),
                epoch,
            }),
            tenant_id,
        }
    }

    pub fn acquire(&self) -> Result<(), RateLimitError> {
        let inner = &self.inner;
        let now_ns = inner.epoch.elapsed().as_nanos() as u64;

        let last = inner.last_refill_ns.load(Ordering::Relaxed);
        let elapsed = now_ns.saturating_sub(last);
        if elapsed > 0 {
            if inner
                .last_refill_ns
                .compare_exchange(last, now_ns, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                let add = elapsed.saturating_mul(inner.refill_rate_per_ns_fp);
                let cap = inner.capacity << 16;
                let cur = inner.tokens_fp.load(Ordering::Relaxed);
                let new_tokens = (cur + add).min(cap);
                inner.tokens_fp.store(new_tokens, Ordering::Relaxed);
            }
        }

        let one = 1u64 << 16;
        let cur = inner.tokens_fp.load(Ordering::Relaxed);
        if cur < one {
            return Err(RateLimitError::RateLimited);
        }
        inner.tokens_fp.fetch_sub(one, Ordering::Relaxed);
        Ok(())
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }
}

impl PerTenantRateLimiter {
    pub fn new(default_capacity: u64, default_refill_per_sec: u64) -> Self {
        Self {
            buckets: HashMap::new(),
            default_capacity,
            default_refill_per_sec,
            metrics: RateLimitMetrics::new(),
        }
    }

    pub fn register_tenant(&mut self, tenant_id: &str) {
        self.register_tenant_with_rate(
            tenant_id,
            self.default_capacity,
            self.default_refill_per_sec,
        );
    }

    pub fn register_tenant_with_rate(
        &mut self,
        tenant_id: &str,
        capacity: u64,
        refill_per_sec: u64,
    ) {
        self.buckets.insert(
            tenant_id.to_string(),
            TenantBucket::new(tenant_id.to_string(), capacity, refill_per_sec),
        );
    }

    pub fn try_acquire(&mut self, tenant_id: &str) -> Result<(), RateLimitError> {
        let bucket = self
            .buckets
            .get(tenant_id)
            .ok_or(RateLimitError::TenantNotFound)?;

        match bucket.acquire() {
            Ok(()) => {
                self.metrics.record_allowed(tenant_id);
                Ok(())
            }
            Err(e) => {
                self.metrics.record_blocked(tenant_id);
                Err(e)
            }
        }
    }

    pub fn remove_tenant(&mut self, tenant_id: &str) {
        self.buckets.remove(tenant_id);
    }

    pub fn get_tenant_bucket(&self, tenant_id: &str) -> Option<&TenantBucket> {
        self.buckets.get(tenant_id)
    }

    pub fn metrics(&self) -> &RateLimitMetrics {
        &self.metrics
    }

    pub fn active_tenant_count(&self) -> usize {
        self.buckets.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RateLimitError {
    RateLimited,
    TenantNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_basic_acquire() {
        let bucket = TokenBucket::new(10, 100);
        for _ in 0..10 {
            assert!(bucket.acquire().is_ok());
        }
        assert!(bucket.acquire().is_err());
    }

    #[test]
    fn test_token_refill_over_time() {
        let bucket = TokenBucket::new(1, 100);
        assert!(bucket.acquire().is_ok());
        assert!(bucket.acquire().is_err());

        thread::sleep(std::time::Duration::from_millis(20));
        assert!(bucket.acquire().is_ok());
    }

    #[test]
    fn test_register_and_acquire_tenant() {
        let mut limiter = PerTenantRateLimiter::new(5, 10);
        limiter.register_tenant("tenant-a");

        for _ in 0..5 {
            assert!(limiter.try_acquire("tenant-a").is_ok());
        }
        assert!(limiter.try_acquire("tenant-a").is_err());
    }

    #[test]
    fn test_unknown_tenant_returns_error() {
        let mut limiter = PerTenantRateLimiter::new(5, 10);
        assert_eq!(
            limiter.try_acquire("unknown"),
            Err(RateLimitError::TenantNotFound)
        );
    }

    #[test]
    fn test_per_tenant_independent_limits() {
        let mut limiter = PerTenantRateLimiter::new(2, 100);
        limiter.register_tenant("tenant-a");
        limiter.register_tenant("tenant-b");

        assert!(limiter.try_acquire("tenant-a").is_ok());
        assert!(limiter.try_acquire("tenant-a").is_ok());

        assert!(limiter.try_acquire("tenant-b").is_ok());
        assert!(limiter.try_acquire("tenant-b").is_ok());

        assert!(limiter.try_acquire("tenant-a").is_err());
        assert!(limiter.try_acquire("tenant-b").is_err());
    }

    #[test]
    fn test_custom_rates_per_tenant() {
        let mut limiter = PerTenantRateLimiter::new(5, 10);
        limiter.register_tenant_with_rate("premium", 100, 1000);
        limiter.register_tenant_with_rate("free", 1, 1);

        let premium_bucket = limiter.get_tenant_bucket("premium").unwrap();
        assert_eq!(premium_bucket.inner.capacity, 100);
    }

    #[test]
    fn test_metrics_tracking() {
        let mut limiter = PerTenantRateLimiter::new(2, 100);
        limiter.register_tenant("tenant-a");

        limiter.try_acquire("tenant-a").ok();
        limiter.try_acquire("tenant-a").ok();
        limiter.try_acquire("tenant-a").ok();

        let metrics = limiter.metrics();
        assert_eq!(metrics.total_allowed("tenant-a"), 2);
        assert_eq!(metrics.total_blocked("tenant-a"), 1);
    }

    #[test]
    fn test_remove_tenant() {
        let mut limiter = PerTenantRateLimiter::new(5, 10);
        limiter.register_tenant("tenant-a");
        assert_eq!(limiter.active_tenant_count(), 1);

        limiter.remove_tenant("tenant-a");
        assert_eq!(limiter.active_tenant_count(), 0);
        assert_eq!(
            limiter.try_acquire("tenant-a"),
            Err(RateLimitError::TenantNotFound)
        );
    }
}
