/// Integration test: Network partition → divergent QCs → convergence.
///
/// Scenario:
/// 1. Have 2 partitions produce conflicting QCs at the same view
/// 2. Heal the network
/// 3. Verify convergence within 3 view-change rounds
use std::collections::BTreeSet;

use lumina_consensus::commit::{compare_qcs, converge_conflicting};
use lumina_consensus::view_change::{QcValidator, QcValidatorConfig};
use lumina_consensus::QuorumCertificate;

/// Simulate a 7-replica cluster (f=2, quorum=5).
const N: usize = 7;
const F: usize = 2;
const QUORUM: usize = 5;

/// Create a QC for replica `replica_id` proposing `block_hash`.
fn create_qc(view: u64, epoch: u64, block_hash: [u8; 32], replica_id: u8) -> QuorumCertificate {
    let mut signer_set = BTreeSet::new();
    for i in 0..QUORUM {
        let mut pk = vec![replica_id; 32];
        pk[0] = i as u8; // unique prefix per signer
        signer_set.insert(pk);
    }
    QuorumCertificate::new(view, epoch, block_hash, signer_set, vec![replica_id; 64])
}

#[test]
fn test_partition_convergence_within_3_rounds() {
    let config = QcValidatorConfig {
        n: N,
        f: F,
        max_sigs_per_cert: 128,
        quarantine_rounds: 2,
    };
    let mut validator = QcValidator::new(config);

    let mut active_qcs: Vec<QuorumCertificate> = Vec::new();

    let block_a = [1u8; 32];
    let block_b = [2u8; 32];

    let qc_a = create_qc(1, 1, block_a, 0xAA);
    let qc_b = create_qc(1, 1, block_b, 0xBB);

    validator
        .validate_and_merge(qc_a.clone(), &mut active_qcs, 1)
        .expect("QC_A should be valid");
    assert_eq!(active_qcs.len(), 1, "QC_A should be active");

    let result = validator.validate_and_merge(qc_b.clone(), &mut active_qcs, 1);
    assert!(result.is_err(), "QC_B should be rejected as conflicting");
    assert_eq!(active_qcs.len(), 1, "Only QC_A should be active");
    assert_eq!(
        validator.quarantine().len(),
        1,
        "QC_B should be quarantined"
    );
    assert_eq!(
        validator.events.len(),
        1,
        "Conflict event should be emitted"
    );

    if let lumina_consensus::ConsensusEvent::QcConflictDetected { view, .. } = &validator.events[0]
    {
        assert_eq!(*view, 1);
    } else {
        panic!("Expected QcConflictDetected event");
    }

    let divergent = vec![qc_a.clone(), qc_b.clone()];
    let winner = converge_conflicting(&divergent).unwrap();

    // Winner should be one of the two blocks (deterministic tie-break)
    assert!(
        winner.block_hash == block_a || winner.block_hash == block_b,
        "Winner should be one of the two conflicting blocks"
    );

    // Simulate view-change rounds (garbage collection)
    validator.gc(2);
    assert_eq!(
        validator.quarantine().len(),
        1,
        "QC still quarantined after 1 round"
    );
    validator.gc(3);
    assert_eq!(
        validator.quarantine().len(),
        0,
        "QC garbage-collected after 2 rounds"
    );

    // Verify convergence within 3 rounds using the chosen QC
    let mut converged = false;
    for round in 1..=3 {
        let qc_new = create_qc(round + 1, round, winner.block_hash, 0xAA);
        match validator.validate_and_merge(qc_new, &mut active_qcs, round + 1) {
            Ok(()) => {
                converged = true;
                break;
            }
            Err(_) => continue,
        }
    }
    assert!(
        converged,
        "Convergence should happen within 3 view-change rounds"
    );
}

#[test]
fn test_epoch_tiebreaker_heals_partition() {
    let block_a = [1u8; 32];
    let block_b = [2u8; 32];

    let qc_a = create_qc(1, 2, block_a, 0xAA); // higher epoch
    let qc_b = create_qc(1, 1, block_b, 0xBB);

    let winner = converge_conflicting(&[qc_a.clone(), qc_b.clone()]).unwrap();
    assert_eq!(winner.block_hash, block_a, "Higher epoch should win");
    assert_eq!(compare_qcs(&qc_a, &qc_b), std::cmp::Ordering::Greater);
}

#[test]
fn test_quorum_threshold_enforced() {
    let config = QcValidatorConfig {
        n: N,
        f: F,
        max_sigs_per_cert: 128,
        quarantine_rounds: 2,
    };
    let validator = QcValidator::new(config);

    let mut signer_set = BTreeSet::new();
    for i in 0..3 {
        // only 3 signers, need 5
        signer_set.insert(vec![i; 32]);
    }
    let bad_qc = QuorumCertificate::new(1, 0, [0u8; 32], signer_set, vec![0u8; 64]);

    let result = validator.validate(&bad_qc, 0);
    assert!(
        result.is_err(),
        "Should reject QC with insufficient signers"
    );
}
