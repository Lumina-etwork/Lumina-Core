use tracing;

use crate::metrics::Metrics;
use crate::types::*;

const INITIAL_BACKOFF_MS: u64 = 1_000;
const MAX_BACKOFF_MS: u64 = 30_000;

pub struct CommitteeMember {
    pub id: u64,
    pub weight: CommitteeWeight,
    pub last_seen_ms: TimestampMs,
    pub clock_drift_ms: i64,
}

pub struct CommitteeState {
    pub chain_id: ChainId,
    pub members: Vec<CommitteeMember>,
    pub total_weight: CommitteeWeight,
    pub last_sync_ms: TimestampMs,
    pub sync_attempts: u32,
    pub retry_backoff_ms: u64,
}

impl CommitteeState {
    pub fn new(chain_id: ChainId, members: Vec<CommitteeMember>) -> Self {
        let total_weight = members.iter().map(|m| m.weight).sum();
        Self {
            chain_id,
            members,
            total_weight,
            last_sync_ms: 0,
            sync_attempts: 0,
            retry_backoff_ms: INITIAL_BACKOFF_MS,
        }
    }

    pub fn should_sync(&self, now_ms: TimestampMs, config: &ChainConfig) -> bool {
        let elapsed = now_ms.saturating_sub(self.last_sync_ms);
        if self.sync_attempts > 0 && elapsed < self.retry_backoff_ms {
            return false;
        }
        elapsed >= config.sync_interval_ms()
    }

    pub fn record_sync_attempt(&mut self, now_ms: TimestampMs, success: bool, metrics: &Metrics) {
        self.last_sync_ms = now_ms;
        if success {
            self.sync_attempts = 0;
            self.retry_backoff_ms = INITIAL_BACKOFF_MS;
            tracing::debug!(chain_id = self.chain_id, "committee sync succeeded");
        } else {
            self.sync_attempts += 1;
            self.retry_backoff_ms = std::cmp::min(self.retry_backoff_ms * 2, MAX_BACKOFF_MS);
            tracing::warn!(
                chain_id = self.chain_id,
                attempt = self.sync_attempts,
                backoff_ms = self.retry_backoff_ms,
                "committee sync failed, backing off",
            );
        }
        metrics.record_chain_sync_backoff_ms(self.chain_id, self.retry_backoff_ms as i64);
    }

    pub fn detect_drift(&self, config: &ChainConfig) -> bool {
        if self.members.is_empty() {
            return false;
        }
        let min_drift = self.members.iter().map(|m| m.clock_drift_ms).min().unwrap_or(0);
        let max_drift = self.members.iter().map(|m| m.clock_drift_ms).max().unwrap_or(0);
        let spread = max_drift.saturating_sub(min_drift) as u64;
        spread > config.max_clock_drift_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Metrics;

    fn make_member(id: u64, weight: u64, drift_ms: i64) -> CommitteeMember {
        CommitteeMember {
            id,
            weight,
            last_seen_ms: 0,
            clock_drift_ms: drift_ms,
        }
    }

    #[test]
    fn should_sync_respects_interval() {
        let cfg = ChainConfig::new(1, 2000);
        let members = vec![make_member(1, 1, 0)];
        let state = CommitteeState::new(1, members);
        assert!(state.should_sync(500, &cfg));
        assert!(state.should_sync(499, &cfg));
        assert!(state.should_sync(501, &cfg));
    }

    #[test]
    fn should_sync_respects_backoff() {
        let cfg = ChainConfig::new(1, 2000);
        let members = vec![make_member(1, 1, 0)];
        let mut state = CommitteeState::new(1, members);
        let metrics = Metrics::new();

        state.record_sync_attempt(0, false, &metrics);
        assert_eq!(state.retry_backoff_ms, 2000);
        assert!(!state.should_sync(500, &cfg));
        assert!(state.should_sync(2000, &cfg));
    }

    #[test]
    fn exponential_backoff_capped() {
        let members = vec![make_member(1, 1, 0)];
        let mut state = CommitteeState::new(1, members);
        let metrics = Metrics::new();

        for _ in 0..6 {
            state.record_sync_attempt(0, false, &metrics);
        }
        assert_eq!(state.retry_backoff_ms, MAX_BACKOFF_MS);
    }

    #[test]
    fn detect_drift_threshold() {
        let cfg = ChainConfig::new(1, 2000);
        let members = vec![make_member(1, 1, 0), make_member(2, 1, 600)];
        let state = CommitteeState::new(1, members);
        assert!(state.detect_drift(&cfg));

        let members2 = vec![make_member(1, 1, 100), make_member(2, 1, 300)];
        let state2 = CommitteeState::new(1, members2);
        assert!(!state2.detect_drift(&cfg));
    }

    #[test]
    fn supermajority_weight_totaled() {
        let members = vec![make_member(1, 5, 0), make_member(2, 3, 0), make_member(3, 2, 0)];
        let state = CommitteeState::new(1, members);
        assert_eq!(state.total_weight, 10);
    }
}
