use std::collections::HashMap;
use std::time::{Duration, Instant};
use log::debug;

use super::types::{
    CongestionEvent, CongestionMetrics, TenantId, BACKOFF_MULTIPLIER, PROBE_BASE_INTERVAL,
    PROBE_MAX_INTERVAL,
};

struct ProbeState {
    current_interval: Duration,
    attempt: u32,
    last_probe_at: Option<Instant>,
}

impl ProbeState {
    fn new() -> Self {
        Self {
            current_interval: PROBE_BASE_INTERVAL,
            attempt: 0,
            last_probe_at: None,
        }
    }

    fn backoff(&mut self) -> Duration {
        self.attempt += 1;
        let next = self.current_interval * BACKOFF_MULTIPLIER;
        if next > PROBE_MAX_INTERVAL {
            self.current_interval = PROBE_MAX_INTERVAL;
        } else {
            self.current_interval = next;
        }
        self.current_interval
    }

    fn reset(&mut self) {
        self.current_interval = PROBE_BASE_INTERVAL;
        self.attempt = 0;
        self.last_probe_at = None;
    }

    fn should_send_probe(&self) -> bool {
        match self.last_probe_at {
            None => true,
            Some(last) => last.elapsed() >= self.current_interval,
        }
    }
}

pub struct ProbeScheduler {
    tenants: HashMap<TenantId, ProbeState>,
}

impl ProbeScheduler {
    pub fn new() -> Self {
        Self {
            tenants: HashMap::new(),
        }
    }

    pub fn on_zero_window(&mut self, tenant: &TenantId) -> Option<CongestionEvent> {
        let state = self
            .tenants
            .entry(tenant.clone())
            .or_insert_with(ProbeState::new);
        let interval = state.backoff();
        debug!(
            "Probe scheduled for {} with interval {}ms (attempt {})",
            tenant,
            interval.as_millis() as u64,
            state.attempt,
        );
        Some(CongestionEvent::ProbeSent {
            tenant_id: tenant.clone(),
            backoff_ms: interval.as_millis() as u64,
        })
    }

    pub fn on_non_zero_ack(&mut self, tenant: &TenantId) {
        if let Some(state) = self.tenants.get_mut(tenant) {
            let old = state.current_interval;
            state.reset();
            debug!(
                "Probe interval reset for {} (previous: {}ms) on non-zero ACK",
                tenant,
                old.as_millis() as u64,
            );
        }
    }

    pub fn should_probe(&self, tenant: &TenantId) -> bool {
        self.tenants
            .get(tenant)
            .map(|s| s.should_send_probe())
            .unwrap_or(false)
    }

    pub fn record_probe_sent(&mut self, tenant: &TenantId) {
        if let Some(state) = self.tenants.get_mut(tenant) {
            state.last_probe_at = Some(Instant::now());
        }
    }

    pub fn current_interval(&self, tenant: &TenantId) -> Duration {
        self.tenants
            .get(tenant)
            .map(|s| s.current_interval)
            .unwrap_or(PROBE_BASE_INTERVAL)
    }

    pub fn metrics(&self) -> CongestionMetrics {
        let max_interval = self
            .tenants
            .values()
            .map(|s| s.current_interval.as_millis() as u64)
            .max()
            .unwrap_or(PROBE_BASE_INTERVAL.as_millis() as u64);
        CongestionMetrics {
            probe_backoff_interval_ms: max_interval,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_probe_backoff() {
        let mut scheduler = ProbeScheduler::new();
        let tenant = "alice".to_string();

        let expected_intervals_ms = [5000u64, 10000, 20000, 30000, 30000];

        for (i, &expected_ms) in expected_intervals_ms.iter().enumerate() {
            let event = scheduler.on_zero_window(&tenant);
            match event {
                Some(CongestionEvent::ProbeSent { backoff_ms, .. }) => {
                    assert_eq!(
                        backoff_ms, expected_ms,
                        "Iteration {i}: expected {expected_ms}ms, got {backoff_ms}ms"
                    );
                }
                other => panic!("Expected ProbeSent at iteration {i}, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_backoff_resets_on_non_zero_ack() {
        let mut scheduler = ProbeScheduler::new();
        let tenant = "bob".to_string();

        scheduler.on_zero_window(&tenant);
        scheduler.on_zero_window(&tenant);
        scheduler.on_zero_window(&tenant);
        assert_eq!(scheduler.current_interval(&tenant).as_secs(), 20);

        scheduler.on_non_zero_ack(&tenant);
        assert_eq!(scheduler.current_interval(&tenant).as_secs(), 5);
    }

    #[test]
    fn test_should_probe() {
        let mut scheduler = ProbeScheduler::new();
        let tenant = "carol".to_string();

        assert!(!scheduler.should_probe(&tenant));

        scheduler.on_zero_window(&tenant);
        scheduler.record_probe_sent(&tenant);
        assert!(!scheduler.should_probe(&tenant));
    }
}
