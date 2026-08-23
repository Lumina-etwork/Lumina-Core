pub mod coordinator;
#[path="prepare-phase.rs"]
pub mod prepare_phase;
#[path="conflict-detector.rs"]
pub mod conflict_detector;
#[path="commit-phase.rs"]
pub mod commit_phase;

#[cfg(test)]
mod tests {
    use super::*;
    use coordinator::MigrationCoordinator;
    use prepare_phase::{PreparePhase, PrepareMessage};
    use conflict_detector::ConflictDetector;
    use commit_phase::CommitPhase;
    use std::time::Duration;

    #[test]
    fn test_concurrent_migrations() {
        let replicas_count = 5; // 4f + 1 where f = 1
        let mut prepare_phases: Vec<_> = (0..replicas_count).map(|_| PreparePhase::new()).collect();
        let mut commit_phases: Vec<_> = (0..replicas_count).map(|_| CommitPhase::new()).collect();
        
        let coordinator = MigrationCoordinator::new();

        // 5 concurrent migrations with 20% range overlap
        let mut migrations = vec![
            (coordinator.next_epoch(), vec![(0, 100)]),
            (coordinator.next_epoch(), vec![(80, 180)]), // 20% overlap
            (coordinator.next_epoch(), vec![(200, 300)]),
            (coordinator.next_epoch(), vec![(280, 380)]), // 20% overlap
            (coordinator.next_epoch(), vec![(400, 500)]),
        ];

        let mut committed = std::collections::HashSet::new();
        
        // Simple simulation loop
        for _ in 0..10 { // Max iterations to resolve conflicts
            let mut active = Vec::new();
            for m in &migrations {
                if !committed.contains(&m.0) {
                    active.push(m.clone());
                }
            }

            if active.is_empty() { break; }

            let mut aborted = std::collections::HashSet::new();

            for i in 0..active.len() {
                for j in (i + 1)..active.len() {
                    if let Some((abort_epoch, _)) = ConflictDetector::detect_conflict(
                        active[i].0, &active[i].1,
                        active[j].0, &active[j].1,
                    ) {
                        aborted.insert(abort_epoch);
                    }
                }
            }

            for (epoch, ranges) in &active {
                if !aborted.contains(epoch) {
                    for p in &mut prepare_phases {
                        p.receive_prepare(PrepareMessage {
                            migration_epoch: *epoch,
                            shard_ranges: ranges.clone(),
                        });
                    }
                    committed.insert(*epoch);
                }
            }
        }

        // Now commit in order of epoch (since prepare sorted them or we just commit sequentially)
        for (i, c) in commit_phases.iter_mut().enumerate() {
            let prepared = prepare_phases[i].prepared_epochs.clone();
            for epoch in prepared {
                assert!(c.commit(epoch).is_ok());
            }
        }
        
        // Verify deterministic ordering: all replicas committed the same epochs in the same order
        for i in 1..replicas_count {
            assert_eq!(
                prepare_phases[0].prepared_epochs,
                prepare_phases[i].prepared_epochs
            );
            assert_eq!(
                commit_phases[0].local_last_committed_epoch,
                commit_phases[i].local_last_committed_epoch
            );
        }
        
        assert_eq!(commit_phases[0].local_last_committed_epoch, 5);
    }
}
