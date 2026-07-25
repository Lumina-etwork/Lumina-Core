use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct RateLimitMetrics {
    allowed: HashMap<String, AtomicU64>,
    blocked: HashMap<String, AtomicU64>,
    total_allowed: AtomicU64,
    total_blocked: AtomicU64,
}

impl RateLimitMetrics {
    pub fn new() -> Self {
        Self {
            allowed: HashMap::new(),
            blocked: HashMap::new(),
            total_allowed: AtomicU64::new(0),
            total_blocked: AtomicU64::new(0),
        }
    }

    pub fn record_allowed(&mut self, tenant_id: &str) {
        self.total_allowed.fetch_add(1, Ordering::Relaxed);
        self.allowed
            .entry(tenant_id.to_string())
            .or_insert(AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_blocked(&mut self, tenant_id: &str) {
        self.total_blocked.fetch_add(1, Ordering::Relaxed);
        self.blocked
            .entry(tenant_id.to_string())
            .or_insert(AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn total_allowed(&self, tenant_id: &str) -> u64 {
        self.allowed
            .get(tenant_id)
            .map(|a| a.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    pub fn total_blocked(&self, tenant_id: &str) -> u64 {
        self.blocked
            .get(tenant_id)
            .map(|b| b.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    pub fn global_allowed(&self) -> u64 {
        self.total_allowed.load(Ordering::Relaxed)
    }

    pub fn global_blocked(&self) -> u64 {
        self.total_blocked.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_tracking() {
        let mut m = RateLimitMetrics::new();
        m.record_allowed("tenant-a");
        m.record_allowed("tenant-a");
        m.record_blocked("tenant-a");
        m.record_allowed("tenant-b");

        assert_eq!(m.total_allowed("tenant-a"), 2);
        assert_eq!(m.total_blocked("tenant-a"), 1);
        assert_eq!(m.total_allowed("tenant-b"), 1);
        assert_eq!(m.global_allowed(), 3);
        assert_eq!(m.global_blocked(), 1);
    }

    #[test]
    fn test_unknown_tenant_returns_zero() {
        let m = RateLimitMetrics::new();
        assert_eq!(m.total_allowed("unknown"), 0);
        assert_eq!(m.total_blocked("unknown"), 0);
    }
}
