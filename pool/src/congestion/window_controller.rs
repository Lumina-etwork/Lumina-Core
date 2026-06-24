use std::collections::HashMap;
use log::warn;

use super::types::{
    CongestionEvent, CongestionMetrics, TenantId, WindowSize, MAX_ZERO_WINDOW_COUNT,
};

pub struct WindowController {
    zero_window_count: HashMap<TenantId, u32>,
    stalls_prevented: u64,
}

impl WindowController {
    pub fn new() -> Self {
        Self {
            zero_window_count: HashMap::new(),
            stalls_prevented: 0,
        }
    }

    pub fn process_heartbeat(
        &mut self,
        tenant: &TenantId,
        advertised_window: WindowSize,
    ) -> Option<CongestionEvent> {
        if !advertised_window.is_zero() {
            self.zero_window_count.remove(tenant);
            return None;
        }

        let count = self.zero_window_count.entry(tenant.clone()).or_insert(0);
        *count += 1;

        if *count >= MAX_ZERO_WINDOW_COUNT {
            let event = CongestionEvent::WindowForcedOpen {
                tenant_id: tenant.clone(),
                new_window: WindowSize::MIN,
            };
            warn!(
                "Forcing window open for {} after {} consecutive zero-window heartbeats (count: {})",
                tenant, MAX_ZERO_WINDOW_COUNT, *count,
            );
            self.zero_window_count.remove(tenant);
            self.stalls_prevented += 1;
            Some(event)
        } else {
            Some(CongestionEvent::ZeroWindowHeartbeat {
                tenant_id: tenant.clone(),
                consecutive_count: *count,
            })
        }
    }

    pub fn metrics(&self) -> CongestionMetrics {
        CongestionMetrics {
            zero_window_stalls_prevented: self.stalls_prevented,
            ..Default::default()
        }
    }

    pub fn reset(&mut self, tenant: &TenantId) {
        self.zero_window_count.remove(tenant);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_zero_heartbeat_resets_counter() {
        let mut controller = WindowController::new();
        let tenant = "alice".to_string();

        for _ in 0..3 {
            controller.process_heartbeat(&tenant, WindowSize::SUSPENDED);
        }

        let result = controller.process_heartbeat(&tenant, WindowSize::new(1024));
        assert!(result.is_none());

        let result = controller.process_heartbeat(&tenant, WindowSize::SUSPENDED);
        match result {
            Some(CongestionEvent::ZeroWindowHeartbeat { consecutive_count, .. }) => {
                assert_eq!(consecutive_count, 1);
            }
            other => panic!("Expected ZeroWindowHeartbeat, got {other:?}"),
        }
    }

    #[test]
    fn test_forced_recovery_after_six_zero_window_heartbeats() {
        let mut controller = WindowController::new();
        let tenant = "bob".to_string();

        for i in 1..=10 {
            let result = controller.process_heartbeat(&tenant, WindowSize::SUSPENDED);

            if i < 6 {
                match result {
                    Some(CongestionEvent::ZeroWindowHeartbeat { consecutive_count, .. }) => {
                        assert_eq!(consecutive_count, i);
                    }
                    other => panic!("Expected ZeroWindowHeartbeat at iteration {i}, got {other:?}"),
                }
            } else if i == 6 {
                match result {
                    Some(CongestionEvent::WindowForcedOpen { new_window, .. }) => {
                        assert_eq!(new_window, WindowSize::MIN);
                    }
                    other => panic!("Expected WindowForcedOpen at iteration {i}, got {other:?}"),
                }
            } else {
                match result {
                    Some(CongestionEvent::ZeroWindowHeartbeat { consecutive_count, .. }) => {
                        assert_eq!(consecutive_count, i - 6);
                    }
                    other => panic!("Expected ZeroWindowHeartbeat at iteration {i}, got {other:?}"),
                }
            }
        }

        assert_eq!(controller.metrics().zero_window_stalls_prevented, 1);
    }
}
