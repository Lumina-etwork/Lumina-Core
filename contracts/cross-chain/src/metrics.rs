use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::types::ChainId;

#[derive(Debug, Clone)]
pub struct Metrics {
    inner: Arc<Mutex<MetricsInner>>,
}

#[derive(Debug, Default)]
struct MetricsInner {
    chain_finality_lag_ms: HashMap<ChainId, i64>,
    chain_sync_backoff_ms: HashMap<ChainId, i64>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(MetricsInner::default())),
        }
    }

    pub fn record_chain_finality_lag_ms(&self, chain_id: ChainId, lag_ms: i64) {
        tracing::debug!(chain_id = chain_id, lag_ms = lag_ms, "chain finality lag");
        if let Ok(mut inner) = self.inner.lock() {
            inner.chain_finality_lag_ms.insert(chain_id, lag_ms);
        }
    }

    pub fn record_chain_sync_backoff_ms(&self, chain_id: ChainId, backoff_ms: i64) {
        tracing::debug!(chain_id = chain_id, backoff_ms = backoff_ms, "chain sync backoff");
        if let Ok(mut inner) = self.inner.lock() {
            inner.chain_sync_backoff_ms.insert(chain_id, backoff_ms);
        }
    }

    pub fn finality_lag_ms(&self, chain_id: ChainId) -> Option<i64> {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.chain_finality_lag_ms.get(&chain_id).copied())
    }

    pub fn sync_backoff_ms(&self, chain_id: ChainId) -> Option<i64> {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.chain_sync_backoff_ms.get(&chain_id).copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_record_and_read() {
        let m = Metrics::new();
        m.record_chain_finality_lag_ms(1, 4200);
        m.record_chain_sync_backoff_ms(1, 2000);
        assert_eq!(m.finality_lag_ms(1), Some(4200));
        assert_eq!(m.sync_backoff_ms(1), Some(2000));
        assert_eq!(m.finality_lag_ms(2), None);
    }
}
