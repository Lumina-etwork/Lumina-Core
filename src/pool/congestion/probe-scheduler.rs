use crate::pool::congestion::types::{
    WindowEvent, ZERO_WINDOW_PROBE_INITIAL_INTERVAL, ZERO_WINDOW_PROBE_MAX_INTERVAL,
};
use std::collections::HashMap;

pub struct ProbeScheduler {
    tenants: HashMap<String, ProbeState>,
    pub probe_backoff_interval_ms: u64,
}

struct ProbeState {
    current_interval_ms: u64,
    consecutive_probes: u64,
}

impl ProbeScheduler {
    pub fn new() -> Self {
        Self {
            tenants: HashMap::new(),
            probe_backoff_interval_ms: ZERO_WINDOW_PROBE_INITIAL_INTERVAL.as_millis() as u64,
        }
    }

    pub fn register_tenant(&mut self, tenant_id: &str) {
        self.tenants.insert(
            tenant_id.to_string(),
            ProbeState {
                current_interval_ms: ZERO_WINDOW_PROBE_INITIAL_INTERVAL.as_millis() as u64,
                consecutive_probes: 0,
            },
        );
    }

    pub fn schedule_probe(&mut self, tenant_id: &str) -> Option<WindowEvent> {
        let state = self.tenants.get_mut(tenant_id)?;

        state.consecutive_probes += 1;

        let interval = state.current_interval_ms;
        state.current_interval_ms = (state.current_interval_ms * 2)
            .min(ZERO_WINDOW_PROBE_MAX_INTERVAL.as_millis() as u64);

        self.probe_backoff_interval_ms = state.current_interval_ms;

        Some(WindowEvent::ProbeScheduled {
            tenant_id: tenant_id.to_string(),
            interval_ms: interval,
        })
    }

    pub fn on_non_zero_window_ack(&mut self, tenant_id: &str) {
        if let Some(state) = self.tenants.get_mut(tenant_id) {
            state.current_interval_ms = ZERO_WINDOW_PROBE_INITIAL_INTERVAL.as_millis() as u64;
            state.consecutive_probes = 0;
        }
    }

    pub fn get_probe_interval(&self, tenant_id: &str) -> Option<u64> {
        self.tenants.get(tenant_id).map(|s| s.current_interval_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        let mut ps = ProbeScheduler::new();
        ps.register_tenant("tenant-a");

        let i1 = ps.schedule_probe("tenant-a").unwrap();
        let i2 = ps.schedule_probe("tenant-a").unwrap();
        let i3 = ps.schedule_probe("tenant-a").unwrap();
        let i4 = ps.schedule_probe("tenant-a").unwrap();
        let i5 = ps.schedule_probe("tenant-a").unwrap();

        assert_eq!(extract_interval(&i1), 5000);
        assert_eq!(extract_interval(&i2), 10000);
        assert_eq!(extract_interval(&i3), 20000);
        assert_eq!(extract_interval(&i4), 30000);
        assert_eq!(extract_interval(&i5), 30000);
    }

    #[test]
    fn test_reset_on_non_zero_ack() {
        let mut ps = ProbeScheduler::new();
        ps.register_tenant("tenant-b");
        ps.schedule_probe("tenant-b");
        ps.schedule_probe("tenant-b");
        ps.on_non_zero_window_ack("tenant-b");
        let interval = ps.get_probe_interval("tenant-b").unwrap();
        assert_eq!(interval, 5000);
    }

    fn extract_interval(event: &WindowEvent) -> u64 {
        if let WindowEvent::ProbeScheduled { interval_ms, .. } = event {
            *interval_ms
        } else {
            0
        }
    }
}
