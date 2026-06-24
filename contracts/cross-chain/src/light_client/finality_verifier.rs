use crate::light_client::committee_sync::CommitteeState;
use crate::metrics::Metrics;
use crate::types::*;

pub struct FinalityVerifier;

impl FinalityVerifier {
    pub fn is_header_finalized(
        header_timestamp_ms: TimestampMs,
        now_ms: TimestampMs,
        committee: &CommitteeState,
        config: &ChainConfig,
        sync_drift_detected: bool,
        metrics: &Metrics,
    ) -> bool {
        if !Self::has_supermajority(committee) {
            return false;
        }

        let confirmation_period = if sync_drift_detected {
            config.grace_timeout_ms()
        } else {
            config.sync_timeout_ms
        };

        let elapsed = now_ms.saturating_sub(header_timestamp_ms);
        if elapsed >= confirmation_period {
            metrics.record_chain_finality_lag_ms(config.chain_id, elapsed as i64);
            true
        } else {
            false
        }
    }

    pub fn has_supermajority(committee: &CommitteeState) -> bool {
        if committee.total_weight == 0 {
            return false;
        }
        let threshold = (2 * committee.total_weight) / 3 + 1;
        let active_weight: CommitteeWeight = committee.members.iter().map(|m| m.weight).sum();
        active_weight >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use crate::light_client::committee_sync::CommitteeMember;
    use crate::metrics::Metrics;
    use crate::types::ChainConfig;

    fn make_committee(weights: &[u64]) -> CommitteeState {
        let members: Vec<CommitteeMember> = weights
            .iter()
            .enumerate()
            .map(|(i, w)| CommitteeMember {
                id: i as u64,
                weight: *w,
                last_seen_ms: 0,
                clock_drift_ms: 0,
            })
            .collect();
        CommitteeState::new(1, members)
    }

    #[test]
    fn supermajority_met() {
        let committee = make_committee(&[3, 3, 3]);
        assert!(FinalityVerifier::has_supermajority(&committee));
    }

    #[test]
    fn supermajority_not_met() {
        let committee = make_committee(&[1, 1, 1]);
        assert!(!FinalityVerifier::has_supermajority(&committee));
    }

    #[test]
    fn supermajority_exact_threshold() {
        let committee = make_committee(&[2, 2, 2]);
        assert!(!FinalityVerifier::has_supermajority(&committee));
    }

    #[test]
    fn supermajority_empty() {
        let committee = make_committee(&[]);
        assert!(!FinalityVerifier::has_supermajority(&committee));
    }

    #[test]
    fn finalized_without_drift() {
        let metrics = Metrics::new();
        let config = ChainConfig::new(1, 2000);
        let committee = make_committee(&[3, 3, 3]);
        let header_time = 0;
        let now = config.sync_timeout_ms;

        assert!(FinalityVerifier::is_header_finalized(
            header_time,
            now,
            &committee,
            &config,
            false,
            &metrics,
        ));
    }

    #[test]
    fn not_yet_finalized() {
        let metrics = Metrics::new();
        let config = ChainConfig::new(1, 2000);
        let committee = make_committee(&[3, 3, 3]);
        let header_time = 0;
        let now = config.sync_timeout_ms - 1;

        assert!(!FinalityVerifier::is_header_finalized(
            header_time,
            now,
            &committee,
            &config,
            false,
            &metrics,
        ));
    }

    #[test]
    fn drift_extends_confirmation() {
        let metrics = Metrics::new();
        let config = ChainConfig::new(1, 2000);
        let committee = make_committee(&[3, 3, 3]);

        let header_time = 0;
        let normal_finalization = config.sync_timeout_ms;
        let grace_finalization = config.grace_timeout_ms();

        assert!(FinalityVerifier::is_header_finalized(
            header_time,
            normal_finalization,
            &committee,
            &config,
            false,
            &metrics,
        ));

        assert!(!FinalityVerifier::is_header_finalized(
            header_time,
            normal_finalization,
            &committee,
            &config,
            true,
            &metrics,
        ));

        assert!(FinalityVerifier::is_header_finalized(
            header_time,
            grace_finalization,
            &committee,
            &config,
            true,
            &metrics,
        ));
    }

    #[test]
    fn no_supermajority_prevents_finalization() {
        let metrics = Metrics::new();
        let config = ChainConfig::new(1, 2000);
        let committee = make_committee(&[1, 1, 1]);

        assert!(!FinalityVerifier::is_header_finalized(
            0,
            config.sync_timeout_ms + 1,
            &committee,
            &config,
            false,
            &metrics,
        ));
    }
}
