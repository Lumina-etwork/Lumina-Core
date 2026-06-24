use std::collections::HashMap;
use log::debug;

use super::types::{CongestionMetrics, TenantId, WindowSize, ACK_SUPPRESSION_THRESHOLD};

pub struct BackpressureSender {
    zero_window_heartbeat_count: HashMap<TenantId, u32>,
    total_suppressed: u64,
}

impl BackpressureSender {
    pub fn new() -> Self {
        Self {
            zero_window_heartbeat_count: HashMap::new(),
            total_suppressed: 0,
        }
    }

    pub fn should_suppress_ack(&mut self, tenant: &TenantId, window: WindowSize) -> bool {
        if !window.is_zero() {
            self.zero_window_heartbeat_count.remove(tenant);
            return false;
        }

        let count = self
            .zero_window_heartbeat_count
            .entry(tenant.clone())
            .or_insert(0);
        *count += 1;

        if *count > ACK_SUPPRESSION_THRESHOLD {
            self.total_suppressed += 1;
            debug!(
                "Suppressing zero-window ACK for {} (consecutive: {}) (idempotent)",
                tenant, *count,
            );
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self, tenant: &TenantId) {
        self.zero_window_heartbeat_count.remove(tenant);
    }

    pub fn metrics(&self) -> CongestionMetrics {
        CongestionMetrics {
            total_zero_window_ack_suppressed: self.total_suppressed,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ack_suppression_after_three_zero_window() {
        let mut sender = BackpressureSender::new();
        let tenant = "alice".to_string();

        for i in 1..=3 {
            let suppressed = sender.should_suppress_ack(&tenant, WindowSize::SUSPENDED);
            assert!(!suppressed, "ACK {i} should NOT be suppressed");
        }

        for i in 4..=10 {
            let suppressed = sender.should_suppress_ack(&tenant, WindowSize::SUSPENDED);
            assert!(suppressed, "ACK {i} SHOULD be suppressed");
        }
    }

    #[test]
    fn test_non_zero_window_resets_suppression() {
        let mut sender = BackpressureSender::new();
        let tenant = "bob".to_string();

        sender.should_suppress_ack(&tenant, WindowSize::SUSPENDED);
        sender.should_suppress_ack(&tenant, WindowSize::SUSPENDED);
        sender.should_suppress_ack(&tenant, WindowSize::SUSPENDED);
        assert!(sender.should_suppress_ack(&tenant, WindowSize::SUSPENDED));

        let suppressed = sender.should_suppress_ack(&tenant, WindowSize::new(4096));
        assert!(!suppressed, "Non-zero ACK should not be suppressed");

        let suppressed = sender.should_suppress_ack(&tenant, WindowSize::SUSPENDED);
        assert!(
            !suppressed,
            "After reset, first zero-window ACK should be allowed"
        );
    }

    #[test]
    fn test_ack_suppression_metrics() {
        let mut sender = BackpressureSender::new();
        let tenant = "carol".to_string();

        for _ in 0..3 {
            sender.should_suppress_ack(&tenant, WindowSize::SUSPENDED);
        }
        sender.should_suppress_ack(&tenant, WindowSize::SUSPENDED);
        sender.should_suppress_ack(&tenant, WindowSize::SUSPENDED);

        let metrics = sender.metrics();
        assert_eq!(metrics.total_zero_window_ack_suppressed, 2);
    }
}
