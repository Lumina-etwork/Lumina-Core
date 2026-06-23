use core::cmp;

pub type ChainId = u64;
pub type BlockTimeMs = u64;
pub type TimestampMs = u64;
pub type CommitteeWeight = u64;

pub struct ChainConfig {
    pub chain_id: ChainId,
    pub block_time_ms: BlockTimeMs,
    pub sync_timeout_ms: u64,
    pub max_clock_drift_ms: u64,
}

impl ChainConfig {
    pub fn new(chain_id: ChainId, block_time_ms: BlockTimeMs) -> Self {
        Self {
            chain_id,
            block_time_ms,
            sync_timeout_ms: cmp::max(3 * block_time_ms, 60_000),
            max_clock_drift_ms: 500,
        }
    }

    pub fn sync_interval_ms(&self) -> u64 {
        cmp::max(self.block_time_ms / 4, 1)
    }

    pub fn grace_timeout_ms(&self) -> u64 {
        (self.sync_timeout_ms as f64 * 1.5) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_config_fast_chain() {
        let cfg = ChainConfig::new(1, 2000);
        assert_eq!(cfg.sync_timeout_ms, 60_000);
        assert_eq!(cfg.sync_interval_ms(), 500);
        assert_eq!(cfg.grace_timeout_ms(), 90_000);
        assert_eq!(cfg.max_clock_drift_ms, 500);
    }

    #[test]
    fn chain_config_slow_chain() {
        let cfg = ChainConfig::new(2, 15_000);
        assert_eq!(cfg.sync_timeout_ms, 60_000);
        assert_eq!(cfg.sync_interval_ms(), 3_750);
        assert_eq!(cfg.grace_timeout_ms(), 90_000);
    }

    #[test]
    fn chain_config_very_slow_chain() {
        let cfg = ChainConfig::new(3, 30_000);
        assert_eq!(cfg.sync_timeout_ms, 90_000);
        assert_eq!(cfg.sync_interval_ms(), 7_500);
        assert_eq!(cfg.grace_timeout_ms(), 135_000);
    }
}
