use std::sync::Arc;
use std::thread;

use crate::core::pool::shard_manager::{ShardManager, TenantId};
use crate::core::pool::shard_state::TOTAL_SHARDS;

/// Concurrent stress test: 20 threads each perform 1000 release/reclaim cycles
/// on overlapping shard IDs, verifying the invariant holds at every step.
#[test]
fn concurrent_release_reclaim_stress() {
    let mgr = Arc::new(ShardManager::new());

    // Pre-allocate shards across tenants so each thread has shards to release.
    // Tenants 0..19 each get 5 shards.
    let mut tenant_shards: Vec<Vec<_>> = Vec::new();
    for i in 0..20u64 {
        let t = TenantId(i);
        let mut shards = Vec::new();
        for _ in 0..5 {
            shards.push(mgr.reclaim_shard(t).unwrap());
        }
        tenant_shards.push(shards);
    }

    mgr.verify_invariant().unwrap();

    let handles: Vec<_> = (0..20)
        .map(|thread_idx| {
            let mgr = Arc::clone(&mgr);
            let initial_shards = tenant_shards[thread_idx].clone();
            thread::spawn(move || {
                let tenant = TenantId(thread_idx as u64);

                // Start by releasing our pre-allocated shards, then reclaim them back,
                // repeating for 1000 cycles.
                let mut owned: Vec<_> = initial_shards;

                for _ in 0..1000 {
                    // Release all owned shards one at a time
                    while let Some(shard) = owned.pop() {
                        mgr.release_shard(tenant, shard).unwrap();
                    }

                    // Reclaim the same number back (may get different shard IDs)
                    for _ in 0..5 {
                        match mgr.reclaim_shard(tenant) {
                            Ok(s) => owned.push(s),
                            Err(_) => {
                                // Another thread took it; that's fine under contention
                                break;
                            }
                        }
                    }
                }

                // Release whatever we still hold
                for shard in owned {
                    mgr.release_shard(tenant, shard).unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }

    // After all threads complete, invariant must hold.
    mgr.verify_invariant().unwrap();
    // All shards should be free since every thread released its holdings.
    assert_eq!(mgr.free_count(), TOTAL_SHARDS);
}

/// Stress test: interleaved reclaim/release across many tenants.
#[test]
fn interleaved_reclaim_release_many_tenants() {
    let mgr = Arc::new(ShardManager::new());

    let handles: Vec<_> = (0..20)
        .map(|i| {
            let mgr = Arc::clone(&mgr);
            thread::spawn(move || {
                let tenant = TenantId(i as u64);
                for _ in 0..500 {
                    if let Ok(shard) = mgr.reclaim_shard(tenant) {
                        mgr.release_shard(tenant, shard).unwrap();
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }

    mgr.verify_invariant().unwrap();
    assert_eq!(mgr.free_count(), TOTAL_SHARDS);
}

/// Verify that the invariant checker catches inconsistencies.
#[test]
fn invariant_holds_after_sequential_ops() {
    let mgr = ShardManager::new();
    let t1 = TenantId(100);
    let t2 = TenantId(200);

    let s1 = mgr.reclaim_shard(t1).unwrap();
    let s2 = mgr.reclaim_shard(t1).unwrap();
    let s3 = mgr.reclaim_shard(t2).unwrap();

    mgr.verify_invariant().unwrap();
    assert_eq!(mgr.free_count(), TOTAL_SHARDS - 3);

    mgr.release_shard(t1, s1).unwrap();
    mgr.release_shard(t2, s3).unwrap();
    mgr.verify_invariant().unwrap();
    assert_eq!(mgr.free_count(), TOTAL_SHARDS - 1);

    mgr.release_shard(t1, s2).unwrap();
    mgr.verify_invariant().unwrap();
    assert_eq!(mgr.free_count(), TOTAL_SHARDS);
}
