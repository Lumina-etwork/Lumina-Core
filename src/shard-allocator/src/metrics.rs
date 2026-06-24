use crate::allocator::BuddyAllocator;

/// Fragmentation ratio gauge for Prometheus-style metrics.
#[derive(Clone, Debug)]
pub struct FragmentationGauge {
    pool_id: u16,
    value: f64,
}

impl FragmentationGauge {
    pub fn new(pool_id: u16) -> Self {
        Self { pool_id, value: 0.0 }
    }

    /// Update from the current allocator state.
    pub fn update(&mut self, allocator: &BuddyAllocator) {
        self.value = allocator.fragmentation_ratio();
    }

    /// Current fragmentation ratio (0.0 = perfect, 1.0 = fully fragmented).
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Pool ID this gauge belongs to.
    pub fn pool_id(&self) -> u16 {
        self.pool_id
    }
}

/// Metrics for the shard allocator.
#[derive(Debug)]
pub struct AllocatorMetrics {
    pub fragmentation_gauge: FragmentationGauge,
    pub total_allocations: u64,
    pub total_frees: u64,
    pub total_defrags: u64,
    pub slabs_relocated: u64,
}

impl AllocatorMetrics {
    pub fn new(pool_id: u16) -> Self {
        Self {
            fragmentation_gauge: FragmentationGauge::new(pool_id),
            total_allocations: 0,
            total_frees: 0,
            total_defrags: 0,
            slabs_relocated: 0,
        }
    }
}