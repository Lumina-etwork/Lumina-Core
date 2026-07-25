use crate::pool::congestion::types::{
    WindowEvent, WindowState, WINDOW_MIN, WINDOW_SUSPENDED, MAX_ZERO_WINDOW_HEARTBEATS,
};
use std::collections::HashMap;

pub struct WindowController {
    tenants: HashMap<String, WindowState>,
    pub zero_window_stalls_prevented: u64,
}

impl WindowController {
    pub fn new() -> Self {
        Self {
            tenants: HashMap::new(),
            zero_window_stalls_prevented: 0,
        }
    }

    pub fn register_tenant(&mut self, tenant_id: &str, initial_window: u64) {
        self.tenants.insert(
            tenant_id.to_string(),
            WindowState::new(initial_window),
        );
    }

    pub fn get_window(&self, tenant_id: &str) -> Option<u64> {
        self.tenants.get(tenant_id).map(|s| s.current_window)
    }

    pub fn advertise_window(
        &mut self,
        tenant_id: &str,
        desired_window: u64,
    ) -> Vec<WindowEvent> {
        let mut events = Vec::new();
        let state = self.tenants.get_mut(tenant_id);

        if let Some(state) = state {
            let prev = state.current_window;
            state.set_window(desired_window);

            if state.current_window == WINDOW_SUSPENDED {
                state.zero_window_count += 1;
                state.consecutive_zero_heartbeats += 1;

                if state.zero_window_count >= MAX_ZERO_WINDOW_HEARTBEATS {
                    state.current_window = WINDOW_MIN;
                    state.zero_window_count = 0;
                    state.consecutive_zero_heartbeats = 0;
                    self.zero_window_stalls_prevented += 1;
                    events.push(WindowEvent::WindowForcedOpen {
                        tenant_id: tenant_id.to_string(),
                        previous_window: prev,
                    });
                }
            }

            events.push(WindowEvent::WindowUpdated {
                tenant_id: tenant_id.to_string(),
                new_window: state.current_window,
            });
        }

        events
    }

    pub fn forced_open_count(&self) -> u64 {
        self.zero_window_stalls_prevented
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_tenant() {
        let mut wc = WindowController::new();
        wc.register_tenant("tenant-a", 1024);
        assert_eq!(wc.get_window("tenant-a"), Some(1024));
    }

    #[test]
    fn test_advertise_zero_window_triggers_forced_open() {
        let mut wc = WindowController::new();
        wc.register_tenant("tenant-a", 65535);

        let mut forced = false;
        for i in 1..=10 {
            let events = wc.advertise_window("tenant-a", 0);
            for ev in &events {
                if let WindowEvent::WindowForcedOpen { .. } = ev {
                    forced = true;
                }
            }
        }

        assert!(forced, "Forced open should trigger within 6 zero-window heartbeats");
        assert_eq!(wc.forced_open_count(), 1);
    }

    #[test]
    fn test_non_zero_window_resets_counters() {
        let mut wc = WindowController::new();
        wc.register_tenant("tenant-b", 65535);

        for _ in 0..3 {
            wc.advertise_window("tenant-b", 0);
        }
        wc.advertise_window("tenant-b", 1024);

        let state = wc.tenants.get("tenant-b").unwrap();
        assert_eq!(state.consecutive_zero_heartbeats, 0);
        assert_eq!(state.zero_window_count, 0);
    }

    #[test]
    fn test_window_clamping() {
        let mut wc = WindowController::new();
        wc.register_tenant("tenant-c", 65535);
        wc.advertise_window("tenant-c", 100_000);
        assert_eq!(wc.get_window("tenant-c"), Some(65535));
    }

    #[test]
    fn test_unknown_tenant_returns_none() {
        let wc = WindowController::new();
        assert_eq!(wc.get_window("nonexistent"), None);
    }
}
