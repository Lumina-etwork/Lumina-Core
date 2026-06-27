/// Striped pool: 128 independently RwLock-guarded BuddyAllocator stripes.
///
/// Shards are dispatched to a stripe by `tenant_id % NUM_STRIPES`.
/// Each stripe manages a disjoint slab range of `TOTAL_SLABS / NUM_STRIPES` slabs.
/// Cross-stripe allocation uses a two-phase approach: acquire the home stripe
/// first; if full, scan other stripes without double-locking.
use std::sync::{Arc, RwLock, atomic::{AtomicU64, Ordering}};
use std::time::Instant;

use crate::allocator::{BuddyAllocator, AllocError};
use crate::metrics::AllocatorMetrics;
use crate::{ShardId, PoolId};

pub const NUM_STRIPES: usize = 128;
/// Slabs per stripe — total 65,536 / 128 = 512.
const SLABS_PER_STRIPE: usize = 512;

/// Per-stripe state: the allocator plus its metrics.
struct Stripe {
    allocator: BuddyAllocator,
    metrics: AllocatorMetrics,
}

impl Stripe {
    fn new(pool_id: PoolId) -> Self {
        Self {
            allocator: BuddyAllocator::new(SLABS_PER_STRIPE, pool_id),
            metrics: AllocatorMetrics::new(pool_id as u16),
        }
    }
}

/// A globally unique slab address: (stripe_index, local_slab_index).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlabAddr {
    pub stripe: u8,
    pub local: u16,
}

impl SlabAddr {
    /// Convert to a global slab index in [0, 65535].
    pub fn global(&self) -> u16 {
        self.stripe as u16 * SLABS_PER_STRIPE as u16 + self.local
    }
}

/// Contention tracking: atomic counters shared across all stripes.
pub struct ContentionStats {
    pub contention_count: AtomicU64,
    pub total_hold_ns: AtomicU64,
    pub total_ops: AtomicU64,
}

impl ContentionStats {
    fn new() -> Self {
        Self {
            contention_count: AtomicU64::new(0),
            total_hold_ns: AtomicU64::new(0),
            total_ops: AtomicU64::new(0),
        }
    }
}

pub struct StripedPool {
    stripes: Vec<RwLock<Stripe>>,
    pub contention: Arc<ContentionStats>,
}

impl StripedPool {
    pub fn new(pool_id: PoolId) -> Self {
        let stripes = (0..NUM_STRIPES)
            .map(|_| RwLock::new(Stripe::new(pool_id)))
            .collect();
        Self {
            stripes,
            contention: Arc::new(ContentionStats::new()),
        }
    }

    /// Stripe index for a given tenant.
    #[inline]
    fn stripe_for(tenant_id: ShardId) -> usize {
        (tenant_id as usize) % NUM_STRIPES
    }

    /// Allocate a slab for `shard_id`.
    ///
    /// Phase 1: try the home stripe.
    /// Phase 2: if full, scan other stripes round-robin without re-locking
    ///          the home stripe.
    pub fn allocate(&self, shard_id: ShardId) -> Result<SlabAddr, AllocError> {
        let home = Self::stripe_for(shard_id);
        if let Some(addr) = self.try_alloc_in(home, shard_id) {
            return Ok(addr);
        }
        // Phase 2: cross-stripe scan
        for offset in 1..NUM_STRIPES {
            let idx = (home + offset) % NUM_STRIPES;
            if let Some(addr) = self.try_alloc_in(idx, shard_id) {
                self.contention.contention_count.fetch_add(1, Ordering::Relaxed);
                return Ok(addr);
            }
        }
        Err(AllocError::OutOfMemory {
            total: NUM_STRIPES * SLABS_PER_STRIPE,
            allocated: NUM_STRIPES * SLABS_PER_STRIPE,
        })
    }

    fn try_alloc_in(&self, stripe_idx: usize, shard_id: ShardId) -> Option<SlabAddr> {
        let t0 = Instant::now();
        let mut stripe = self.stripes[stripe_idx].write().ok()?;
        let hold_ns = t0.elapsed().as_nanos() as u64;

        stripe.metrics.lock_hold_duration_ns.record(hold_ns);
        self.contention.total_hold_ns.fetch_add(hold_ns, Ordering::Relaxed);
        self.contention.total_ops.fetch_add(1, Ordering::Relaxed);

        match stripe.allocator.allocate(shard_id) {
            Ok(local) => {
                stripe.metrics.total_allocations += 1;
                Some(SlabAddr { stripe: stripe_idx as u8, local })
            }
            Err(_) => None,
        }
    }

    /// Free a slab by its global address.
    pub fn free(&self, addr: SlabAddr) -> Result<ShardId, AllocError> {
        let t0 = Instant::now();
        let mut stripe = self.stripes[addr.stripe as usize]
            .write()
            .map_err(|_| AllocError::InvalidSlab { index: addr.local })?;
        let hold_ns = t0.elapsed().as_nanos() as u64;

        stripe.metrics.lock_hold_duration_ns.record(hold_ns);
        self.contention.total_hold_ns.fetch_add(hold_ns, Ordering::Relaxed);
        self.contention.total_ops.fetch_add(1, Ordering::Relaxed);

        let shard = stripe.allocator.free(addr.local)?;
        stripe.metrics.total_frees += 1;
        Ok(shard)
    }

    /// Snapshot the p90 hold-time across all stripes (ns).
    pub fn p90_hold_ns(&self) -> u64 {
        // Merge all per-stripe histograms into one aggregate view.
        // We use bucket sums to avoid locking all stripes simultaneously.
        let mut buckets = [0u64; 5];
        let mut total = 0u64;
        for stripe_lock in &self.stripes {
            if let Ok(s) = stripe_lock.read() {
                for (i, &b) in s.metrics.lock_hold_duration_ns.buckets.iter().enumerate() {
                    buckets[i] += b;
                    total += b;
                }
            }
        }
        if total == 0 {
            return 0;
        }
        let target = (total * 9 + 9) / 10;
        let bucket_upper = [999u64, 9_999, 99_999, 999_999, u64::MAX];
        let mut cum = 0u64;
        for (i, &b) in buckets.iter().enumerate() {
            cum += b;
            if cum >= target {
                return bucket_upper[i];
            }
        }
        u64::MAX
    }

    /// Total contention events (cross-stripe overflows).
    pub fn contention_count(&self) -> u64 {
        self.contention.contention_count.load(Ordering::Relaxed)
    }
}
