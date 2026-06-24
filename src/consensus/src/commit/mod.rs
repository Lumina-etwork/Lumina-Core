/// Deterministic conflict resolution for equivocal quorum certificates.
///
/// Tie-breaking rules:
/// 1. Highest `qc_epoch` wins
/// 2. If equal, compare lexicographic hash of aggregated public-key set
/// 3. If still equal (astronomically unlikely), compare block_hash
use crate::QuorumCertificate;
use std::cmp::Ordering;

/// Result of comparing two conflicting QCs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConflictResolution {
    /// The first QC wins.
    FirstWins,
    /// The second QC wins.
    SecondWins,
    /// QCs are equivalent (same epoch, same pubkey set, same block hash).
    Equivalent,
}

/// Deterministic comparator for conflicting QCs.
///
/// Returns `Ordering::Greater` if QC `a` should take priority over QC `b`.
pub fn compare_qcs(a: &QuorumCertificate, b: &QuorumCertificate) -> Ordering {
    // Rule 1: Highest qc_epoch wins
    match a.qc_epoch.cmp(&b.qc_epoch) {
        Ordering::Greater => return Ordering::Greater,
        Ordering::Less => return Ordering::Less,
        Ordering::Equal => {}
    }

    // Rule 2: Compare lexicographic hash of aggregated public-key set
    let a_hash = a.pubkey_set_hash();
    let b_hash = b.pubkey_set_hash();
    match a_hash.cmp(&b_hash) {
        Ordering::Greater => return Ordering::Greater,
        Ordering::Less => return Ordering::Less,
        Ordering::Equal => {}
    }

    // Rule 3: Compare block_hash (effectively deterministic tie-break)
    a.block_hash.cmp(&b.block_hash)
}

/// Resolve a conflict between two QCs.
pub fn resolve_conflict(a: &QuorumCertificate, b: &QuorumCertificate) -> ConflictResolution {
    match compare_qcs(a, b) {
        Ordering::Greater => ConflictResolution::FirstWins,
        Ordering::Less => ConflictResolution::SecondWins,
        Ordering::Equal => ConflictResolution::Equivalent,
    }
}

/// Attempt to converge a set of conflicting QCs into a single canonical QC.
///
/// Returns `None` if the set is empty.
pub fn converge_conflicting(qcs: &[QuorumCertificate]) -> Option<QuorumCertificate> {
    qcs.iter().max_by(|a, b| compare_qcs(a, b)).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn make_qc(
        view: u64,
        epoch: u64,
        block_hash: [u8; 32],
        pubkeys: &[Vec<u8>],
    ) -> QuorumCertificate {
        let mut signer_set = BTreeSet::new();
        for pk in pubkeys {
            signer_set.insert(pk.clone());
        }
        QuorumCertificate::new(view, epoch, block_hash, signer_set, vec![0u8; 64])
    }

    #[test]
    fn test_epoch_tie_breaker() {
        let h1 = [1u8; 32];
        let pk = vec![1u8; 32];
        let a = make_qc(1, 5, h1, &[pk.clone()]);
        let b = make_qc(1, 3, h1, &[pk]);

        assert_eq!(compare_qcs(&a, &b), Ordering::Greater);
        assert_eq!(resolve_conflict(&a, &b), ConflictResolution::FirstWins);
    }

    #[test]
    fn test_pubkey_hash_tie_breaker() {
        let h1 = [1u8; 32];
        let pk_a = vec![0u8; 32];
        let pk_b = vec![0x42u8; 32];

        let a = make_qc(1, 5, h1, &[pk_a]);
        let b = make_qc(1, 5, h1, &[pk_b]);

        // When epochs are equal, comparison falls through to pubkey_set_hash
        // We just verify the comparison is NOT Equal
        assert_ne!(compare_qcs(&a, &b), Ordering::Equal);
    }

    #[test]
    fn test_convergence() {
        let h1 = [1u8; 32];
        let h2 = [2u8; 32];
        let pk = vec![1u8; 32];

        let qcs = vec![
            make_qc(1, 2, h1, &[pk.clone()]),
            make_qc(1, 5, h2, &[pk.clone()]), // higher epoch — should win
            make_qc(1, 1, h2, &[pk]),
        ];

        let winner = converge_conflicting(&qcs).unwrap();
        assert_eq!(winner.qc_epoch, 5);
        assert_eq!(winner.block_hash, h2);
    }

    #[test]
    fn test_equivalent_qcs() {
        let h = [42u8; 32];
        let pk = vec![7u8; 32];
        let a = make_qc(1, 5, h, &[pk.clone()]);
        let b = make_qc(1, 5, h, &[pk]);

        assert_eq!(resolve_conflict(&a, &b), ConflictResolution::Equivalent);
    }
}