use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize(u16);

impl WindowSize {
    pub const SUSPENDED: Self = Self(0);
    pub const MIN: Self = Self(1);
    pub const MAX: Self = Self(65535);

    pub fn new(value: u16) -> Self {
        match value {
            0 => Self::SUSPENDED,
            _ => Self(value),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn value(&self) -> u16 {
        self.0
    }
}

impl Default for WindowSize {
    fn default() -> Self {
        Self::MAX
    }
}

pub type TenantId = String;

#[derive(Debug, Clone)]
pub enum CongestionEvent {
    WindowForcedOpen {
        tenant_id: TenantId,
        new_window: WindowSize,
    },
    ZeroWindowHeartbeat {
        tenant_id: TenantId,
        consecutive_count: u32,
    },
    ProbeSent {
        tenant_id: TenantId,
        backoff_ms: u64,
    },
    ProbeAckReceived {
        tenant_id: TenantId,
        window: WindowSize,
    },
}

#[derive(Debug, Default, Clone)]
pub struct CongestionMetrics {
    pub zero_window_stalls_prevented: u64,
    pub probe_backoff_interval_ms: u64,
    pub total_zero_window_ack_suppressed: u64,
}

pub const PROBE_BASE_INTERVAL: Duration = Duration::from_secs(5);
pub const PROBE_MAX_INTERVAL: Duration = Duration::from_secs(30);
pub const BACKOFF_MULTIPLIER: u32 = 2;
pub const MAX_ZERO_WINDOW_COUNT: u32 = 6;
pub const ACK_SUPPRESSION_THRESHOLD: u32 = 3;
