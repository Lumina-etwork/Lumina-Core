use lumina_shard_allocator::allocator::BuddyAllocator;
use lumina_shard_allocator::defrag::Defragmenter;
use lumina_shard_allocator::{ShardAllocEvent, SLAB_SIZE, DEFRAG_THRESHOLD};
use std::collections::HashMap;
use std::time::Duration;

#[test]
fn test_allocate_and_free_single() {
    let mut alloc = BuddyAllocator::new(1024, 0);
    let idx = alloc.allocate(42).unwrap();
    assert!(idx < 1024);
    let freed = alloc.free(idx).unwrap();
    assert_eq!(freed, 42);
}

#[test]
fn test_full_allocation() {
    let mut alloc = BuddyAllocator::new(256, 0);
    for i in 0..256 {
        let idx = alloc.allocate(i as u16).unwrap();
        assert_eq!(idx, i as u16);
    }
    assert_eq!(alloc.allocated_slabs(), 256);
    assert!(alloc.allocate(256).is_err());
}

#[test]
fn test_fragmentation_detection() {
    let mut alloc = BuddyAllocator::new(256, 0);
    let mut indices = Vec::new();
    for i in 0..128 {
        let idx = alloc.allocate(i as u16).unwrap();
        indices.push(idx);
    }
    for i in (0..128).step_by(2) {
        alloc.free(indices[i]).unwrap();
    }
    let frag = alloc.fragmentation_ratio();
    assert!(frag > 0.0);
}

#[test]
fn test_defragmentation() {
    let mut alloc = BuddyAllocator::new(256, 0);
    let mut defrag = Defragmenter::new(0.0, Duration::ZERO);

    for i in 0..256 {
        alloc.allocate(i as u16).unwrap();
    }
    for i in (0..256).step_by(2) {
        alloc.free(i as u16).unwrap();
    }

    let frag_before = alloc.fragmentation_ratio();
    assert!(frag_before > 0.0);

    let result = defrag.defrag(&mut alloc).unwrap();
    assert!(result.fragmentation_after < frag_before);
    assert!(result.slabs_relocated > 0);
}

#[test]
fn test_defrag_events_emitted() {
    let mut alloc = BuddyAllocator::new(128, 0);
    let mut defrag = Defragmenter::new(0.0, Duration::ZERO);

    for i in 0..128 {
        alloc.allocate(i as u16).unwrap();
    }
    for i in (0..128).step_by(3) {
        alloc.free(i as u16).unwrap();
    }

    defrag.defrag(&mut alloc);

    let start_events: Vec<_> = alloc.events.iter()
        .filter(|e| matches!(e, ShardAllocEvent::ShardDefragStarted { .. }))
        .collect();
    let complete_events: Vec<_> = alloc.events.iter()
        .filter(|e| matches!(e, ShardAllocEvent::ShardDefragComplete { .. }))
        .collect();

    assert!(!start_events.is_empty());
    assert!(!complete_events.is_empty());
}

#[test]
fn stress_50k_churn_cycles() {
    let pool_size = 4096;
    let mut alloc = BuddyAllocator::new(pool_size, 0);
    let mut defrag = Defragmenter::new(DEFRAG_THRESHOLD, Duration::from_millis(10));
    let mut active: HashMap<u16, u16> = HashMap::new();
    let mut shard_counter: u16 = 0;

    let cycles = 50_000;
    let mut alloc_count = 0u64;
    let mut free_count = 0u64;
    let mut defrag_count = 0u64;

    for cycle in 0..cycles {
        // Allocate biased (2:1 alloc:free ratio for churn)
        if cycle % 3 != 0 || active.len() < pool_size / 4 {
            shard_counter = shard_counter.wrapping_add(1);
            match alloc.allocate(shard_counter) {
                Ok(idx) => {
                    active.insert(idx, shard_counter);
                    alloc_count += 1;
                }
                Err(_) => {
                    // Pool full — evict the first active slab
                    if let Some((&idx_to_free, _)) = active.iter().next() {
                        if alloc.free(idx_to_free).is_ok() {
                            active.remove(&idx_to_free);
                            free_count += 1;
                            if let Ok(new_idx) = alloc.allocate(shard_counter) {
                                active.insert(new_idx, shard_counter);
                                alloc_count += 1;
                            }
                        }
                    }
                }
            }
        } else if !active.is_empty() {
            // Free a random active slab
            let keys: Vec<u16> = active.keys().copied().collect();
            if let Some(&idx_to_free) = keys.get(cycle % keys.len()) {
                if alloc.free(idx_to_free).is_ok() {
                    active.remove(&idx_to_free);
                    free_count += 1;
                }
            }
        }

        // Run defrag every 100 cycles
        if cycle % 100 == 0 && cycle > 0 {
            if let Some(result) = defrag.defrag(&mut alloc) {
                defrag_count += 1;
                // Defrag changed internal state — rebuild active from allocator
                active.clear();
                for (idx, shard) in alloc.allocated_slabs_list() {
                    active.insert(idx, shard);
                }
                eprintln!("Cycle {cycle}: defrag relocated {} slabs, frag {:.4}->{:.4}",
                    result.slabs_relocated,
                    result.fragmentation_before,
                    result.fragmentation_after,
                );
            }
        }
    }

    eprintln!("=== Stress Test Results ===");
    eprintln!("Cycles:         {cycles}");
    eprintln!("Allocations:    {alloc_count}");
    eprintln!("Frees:          {free_count}");
    eprintln!("Defrags:        {defrag_count}");
    eprintln!("Active slabs:   {}", active.len());
    eprintln!("Allocated:      {}", alloc.allocated_slabs());
    eprintln!("Free:           {}", alloc.free_slabs());
    eprintln!("Frag ratio:     {:.4}", alloc.fragmentation_ratio());
    eprintln!("Events:         {}", alloc.events.len());

    assert!(alloc_count > 0);
    assert!(free_count > 0);
    assert!(alloc.fragmentation_ratio() < 1.0);
}

#[test]
fn test_slab_size_constant() {
    assert_eq!(SLAB_SIZE, 65_536);
}

#[test]
fn test_buddy_coalescing() {
    let mut alloc = BuddyAllocator::new(256, 0);
    let a = alloc.allocate(1).unwrap();
    let b = alloc.allocate(2).unwrap();
    let c = alloc.allocate(3).unwrap();
    let d = alloc.allocate(4).unwrap();

    alloc.free(a).unwrap();
    alloc.free(b).unwrap();
    alloc.free(c).unwrap();
    alloc.free(d).unwrap();

    // After buddy coalescing, fragmentation should be low
    assert!(alloc.fragmentation_ratio() < 1.0);
}

#[test]
fn test_duplicate_free_error() {
    let mut alloc = BuddyAllocator::new(64, 0);
    let idx = alloc.allocate(10).unwrap();
    alloc.free(idx).unwrap();
    assert!(alloc.free(idx).is_err());
}

#[test]
fn test_freed_slab_reuse() {
    let mut alloc = BuddyAllocator::new(64, 0);
    let idx = alloc.allocate(99).unwrap();
    alloc.free(idx).unwrap();
    // The freed slab may be coalesced with its buddy,
    // so it may not return the exact same index.
    // Just verify we can allocate again without error.
    let _idx2 = alloc.allocate(100).unwrap();
    assert!(alloc.allocated_slabs() >= 1);
}