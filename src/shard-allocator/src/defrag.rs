/// Background defragmenter with mark-sweep compaction.
///
/// When fragmentation exceeds the threshold (30%), the defragmenter
/// relocates allocated slabs to create larger contiguous free regions.
use crate::allocator::BuddyAllocator;
use crate::{ShardAllocEvent, PoolId, DEFRAG_THRESHOLD};
use std::time::{Duration, Instant};

/// Defragmentation result.
#[derive(Clone, Debug)]
pub struct DefragResult {
    /// Number of slabs relocated during this defragmentation pass.
    pub slabs_relocated: usize,
    /// Fragmentation ratio before defrag.
    pub fragmentation_before: f64,
    /// Fragmentation ratio after defrag.
    pub fragmentation_after: f64,
    /// Time taken for this defrag pass.
    pub elapsed: Duration,
    pub pool_id: PoolId,
}

/// Background defragmenter.
pub struct Defragmenter {
    /// Fragmentation threshold — trigger defrag when exceeded.
    pub threshold: f64,
    /// Minimum interval between defrag passes.
    pub min_interval: Duration,
    /// Last defrag execution time.
    last_defrag: Option<Instant>,
}

impl Default for Defragmenter {
    fn default() -> Self {
        Self {
            threshold: DEFRAG_THRESHOLD,
            min_interval: Duration::from_secs(1),
            last_defrag: None,
        }
    }
}

impl Defragmenter {
    pub fn new(threshold: f64, min_interval: Duration) -> Self {
        Self {
            threshold,
            min_interval,
            last_defrag: None,
        }
    }

    /// Check if defragmentation should run.
    pub fn should_defrag(&self, frag_ratio: f64) -> bool {
        if frag_ratio < self.threshold {
            return false;
        }
        if let Some(last) = self.last_defrag {
            if last.elapsed() < self.min_interval {
                return false;
            }
        }
        true
    }

    /// Run a defragmentation pass — mark-sweep with compaction.
    ///
    /// Strategy:
    /// 1. MARK: Identify all allocated slabs
    /// 2. COMPACT: Relocate slabs toward the beginning of the pool
    /// 3. COALESCE: Buddy tree automatically coalesces freed regions
    pub fn defrag(&mut self, allocator: &mut BuddyAllocator) -> Option<DefragResult> {
        let frag_before = allocator.fragmentation_ratio();

        if !self.should_defrag(frag_before) {
            return None;
        }

        let start = Instant::now();
        let pool_id = allocator.pool_id();

        let event = ShardAllocEvent::ShardDefragStarted {
            pool: pool_id,
            fragmentation_ratio: frag_before,
            free_slabs: allocator.free_slabs(),
            total_slabs: allocator.total_slabs(),
        };
        allocator.events.push(event);

        let mut relocated = 0usize;

        // Mark-sweep: collect all allocated slabs
        let allocated = allocator.allocated_slabs_list();

        // Compact: free all slabs, then re-allocate in order
        // This maximizes contiguity at the start of the pool
        for (idx, _) in &allocated {
            let _ = allocator.free(*idx);
        }

        // Re-allocate in order at the beginning
        for (_, shard) in &allocated {
            if let Ok(_new_idx) = allocator.allocate(*shard) {
                relocated += 1;
            }
        }

        let frag_after = allocator.fragmentation_ratio();
        let elapsed = start.elapsed();

        allocator.events.push(ShardAllocEvent::ShardDefragComplete {
            pool: pool_id,
            reclaimed_slabs: relocated,
            fragmentation_before: frag_before,
            fragmentation_after: frag_after,
            elapsed_ns: elapsed.as_nanos() as u64,
        });

        self.last_defrag = Some(Instant::now());

        Some(DefragResult {
            slabs_relocated: relocated,
            fragmentation_before: frag_before,
            fragmentation_after: frag_after,
            elapsed,
            pool_id,
        })
    }
}