use core::time::Duration;

pub const WINDOW_MIN: u64 = 1;
pub const WINDOW_MAX: u64 = 65535;
pub const WINDOW_SUSPENDED: u64 = 0;
pub const ZERO_WINDOW_PROBE_MIN_INTERVAL: Duration = Duration::from_secs(5);
pub const ZERO_WINDOW_PROBE_INITIAL_INTERVAL: Duration = Duration::from_secs(5);
pub const ZERO_WINDOW_PROBE_MAX_INTERVAL: Duration = Duration::from_secs(30);
pub const MAX_ZERO_WINDOW_HEARTBEATS: u8 = 6;
pub const CONTROL_MESSAGE_OVERHEAD_LIMIT: f64 = 0.05;

#[derive(Clone, Debug, PartialEq)]
pub enum WindowEvent {
    WindowUpdated { tenant_id: String, new_window: u64 },
    WindowForcedOpen { tenant_id: String, previous_window: u64 },
    ProbeScheduled { tenant_id: String, interval_ms: u64 },
    ZeroWindowAckSuppressed { tenant_id: String, consecutive_zero: u8 },
}

#[derive(Clone, Debug)]
pub struct WindowState {
    pub current_window: u64,
    pub initial_window: u64,
    pub zero_window_count: u8,
    pub consecutive_zero_heartbeats: u8,
    pub probe_interval: Duration,
    pub last_probe_time: Option<u64>,
    pub last_ack_time: Option<u64>,
}

impl WindowState {
    pub fn new(initial_window: u64) -> Self {
        Self {
            current_window: initial_window.clamp(WINDOW_MIN, WINDOW_MAX),
            initial_window,
            zero_window_count: 0,
            consecutive_zero_heartbeats: 0,
            probe_interval: ZERO_WINDOW_PROBE_INITIAL_INTERVAL,
            last_probe_time: None,
            last_ack_time: None,
        }
    }

    pub fn is_suspended(&self) -> bool {
        self.current_window == WINDOW_SUSPENDED
    }

    pub fn set_window(&mut self, new_window: u64) {
        self.current_window = new_window.clamp(WINDOW_SUSPENDED, WINDOW_MAX);
        if new_window > 0 {
            self.consecutive_zero_heartbeats = 0;
            self.zero_window_count = 0;
            self.probe_interval = ZERO_WINDOW_PROBE_INITIAL_INTERVAL;
        }
    }
}
