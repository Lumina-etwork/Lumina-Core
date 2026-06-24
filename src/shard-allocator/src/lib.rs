pub mod allocator;
pub mod defrag;
pub mod metrics;

/// Total slab size per tenant in bytes.
pub const SLAB_SIZE: usize = 65_536; // 64 KB

/// Maximum number of tenants per pool.
pub const MAX_TENANTS: usize = 65_536; // 2^16

/// Minimum free contiguous regions before defrag triggers.
pub const DEFRAG_THRESHOLD: f64 = 0.30; // 30% fragmentation

/// Number of free-list levels in the buddy tree (2^16 max slabs).
pub const BUDDY_LEVELS: usize = 17; // log2(65536) + 1

/// A unique identifier for a shard (tenant slot).
pub type ShardId = u16;

/// Unique pool identifier.
pub type PoolId = u16;

/// Events emitted by the allocator for observability.
#[derive(Clone, Debug, PartialEq)]
pub enum ShardAllocEvent {
    ShardAllocated {
        pool: PoolId,
        shard_id: ShardId,
        slab_index: u16,
    },
    ShardFreed {
        pool: PoolId,
        shard_id: ShardId,
        slab_index: u16,
    },
    ShardDefragStarted {
        pool: PoolId,
        fragmentation_ratio: f64,
        free_slabs: usize,
        total_slabs: usize,
    },
    ShardDefragComplete {
        pool: PoolId,
        reclaimed_slabs: usize,
        fragmentation_before: f64,
        fragmentation_after: f64,
        elapsed_ns: u64,
    },
}

/// Fragmentation ratio metric.
#[derive(Clone, Debug)]
pub struct FragmentationGauge(pub f64);

impl FragmentationGauge {
    pub fn ratio(&self) -> f64 {
        self.0
    }
}