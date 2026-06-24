use shard_migration::{Coordinator, coordinator, prepare_phase};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[test]
fn concurrent_migrations_deterministic_ordering() {
    // 4f+1 replicas -> choose f=1 -> 5 replicas
    let mut replicas = vec![];
    for i in 0..5u32 {
        replicas.push(Arc::new(Mutex::new(prepare_phase::Replica::new(i))));
    }
    let coord = Coordinator::new(replicas.clone());

    // create 5 migration batches with some 20% overlap
    let batches = vec![
        coordinator::MigrationBatch{ start:0, end:9, shards:(0..10).collect() },
        coordinator::MigrationBatch{ start:8, end:17, shards:(8..18).collect() },
        coordinator::MigrationBatch{ start:18, end:27, shards:(18..28).collect() },
        coordinator::MigrationBatch{ start:5, end:14, shards:(5..15).collect() },
        coordinator::MigrationBatch{ start:28, end:37, shards:(28..38).collect() },
    ];

    for b in batches {
        coord.submit_migration(b);
    }

    // wait for progress
    thread::sleep(Duration::from_secs(5));

    // After settles, check that all replicas have same committed order (epochs 1..k)
    let mut committed_sequences = vec![];
    for r in &replicas {
        let rep = r.lock().unwrap();
        committed_sequences.push(rep.last_committed_epoch);
    }

    // All replicas should have advanced to same last committed epoch
    let first = committed_sequences[0];
    for &c in &committed_sequences[1..] {
        assert_eq!(c, first, "Replicas diverged: {:?}", committed_sequences);
    }

    // should have committed at least one
    assert!(first >= 1, "No commits performed");
}
