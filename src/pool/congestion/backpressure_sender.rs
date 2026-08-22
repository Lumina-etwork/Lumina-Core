use crate::pool::congestion::types::CONTROL_MESSAGE_OVERHEAD_LIMIT;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};

pub struct BackpressureSender {
    tenants: BTreeMap<String, SenderState>,
    total_control_messages: u64,
    suppressed_acks: u64,
}

struct SenderState {
    consecutive_zero_acks: u8,
    last_window: u64,
    total_messages: u64,
    control_messages: u64,
}

impl BackpressureSender {
    pub fn new() -> Self {
        Self {
            tenants: BTreeMap::new(),
            total_control_messages: 0,
            suppressed_acks: 0,
        }
    }

    pub fn register_tenant(&mut self, tenant_id: &str) {
        self.tenants.insert(
            tenant_id.to_string(),
            SenderState {
                consecutive_zero_acks: 0,
                last_window: 0,
                total_messages: 0,
                control_messages: 0,
            },
        );
    }

    pub fn should_send_ack(&mut self, tenant_id: &str, window: u64) -> bool {
        let state = self.tenants.get_mut(tenant_id);

        let state = match state {
            Some(s) => s,
            None => return true,
        };

        state.total_messages += 1;

        if window == 0 && state.consecutive_zero_acks >= 3 {
            let ratio = if state.total_messages > 0 {
                state.control_messages as f64 / state.total_messages as f64
            } else {
                0.0
            };
            if ratio >= CONTROL_MESSAGE_OVERHEAD_LIMIT {
                self.suppressed_acks += 1;
                return false;
            }
        }

        if window == 0 {
            state.consecutive_zero_acks += 1;
            state.control_messages += 1;
        } else {
            state.consecutive_zero_acks = 0;
            state.control_messages += 1;
        }

        state.last_window = window;
        self.total_control_messages += 1;
        true
    }

    pub fn suppressed_ack_count(&self) -> u64 {
        self.suppressed_acks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suppress_zero_window_ack_after_threshold() {
        let mut bs = BackpressureSender::new();
        bs.register_tenant("tenant-a");

        assert!(bs.should_send_ack("tenant-a", 0));
        assert!(bs.should_send_ack("tenant-a", 0));
        assert!(bs.should_send_ack("tenant-a", 0));
        assert!(!bs.should_send_ack("tenant-a", 0));

        assert!(bs.suppressed_ack_count() >= 1);
    }

    #[test]
    fn test_non_zero_window_resets_suppression() {
        let mut bs = BackpressureSender::new();
        bs.register_tenant("tenant-b");

        bs.should_send_ack("tenant-b", 0);
        bs.should_send_ack("tenant-b", 0);
        bs.should_send_ack("tenant-b", 0);
        bs.should_send_ack("tenant-b", 1024);
        bs.should_send_ack("tenant-b", 0);

        assert!(bs.should_send_ack("tenant-b", 0));
    }

    #[test]
    fn test_unknown_tenant_sends_ack() {
        let mut bs = BackpressureSender::new();
        assert!(bs.should_send_ack("unknown", 0));
    }
}
