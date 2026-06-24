use core::sync::atomic::{AtomicI64, Ordering};

use crate::types::ChainId;

#[derive(Debug)]
pub struct Metrics {
    chain_finality_lag_ms: AtomicI64,
    chain_sync_backoff_ms: AtomicI64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            chain_finality_lag_ms: AtomicI64::new(-1),
            chain_sync_backoff_ms: AtomicI64::new(-1),
        }
    }

    pub fn record_chain_finality_lag_ms(&self, _chain_id: ChainId, lag_ms: i64) {
        self.chain_finality_lag_ms.store(lag_ms, Ordering::Release);
    }

    pub fn record_chain_sync_backoff_ms(&self, _chain_id: ChainId, backoff_ms: i64) {
        self.chain_sync_backoff_ms.store(backoff_ms, Ordering::Release);
    }

    pub fn finality_lag_ms(&self, _chain_id: ChainId) -> Option<i64> {
        let val = self.chain_finality_lag_ms.load(Ordering::Acquire);
        if val < 0 { None } else { Some(val) }
    }

    pub fn sync_backoff_ms(&self, _chain_id: ChainId) -> Option<i64> {
        let val = self.chain_sync_backoff_ms.load(Ordering::Acquire);
        if val < 0 { None } else { Some(val) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_record_and_read() {
        let m = Metrics::new();
        assert_eq!(m.finality_lag_ms(1), None);
        m.record_chain_finality_lag_ms(1, 4200);
        m.record_chain_sync_backoff_ms(1, 2000);
        assert_eq!(m.finality_lag_ms(1), Some(4200));
        assert_eq!(m.sync_backoff_ms(1), Some(2000));
        assert_eq!(m.finality_lag_ms(2), None);
    }
}
