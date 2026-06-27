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

/// Fixed-size histogram over nanosecond hold-time samples using
/// power-of-two buckets: [0,1μs), [1μs,10μs), [10μs,100μs), [100μs,1ms), [1ms,∞).
#[derive(Debug, Default)]
pub struct HoldTimeHistogram {
    /// Bucket counts: indices 0-4 correspond to the ranges above.
    pub buckets: [u64; 5],
    /// Total samples recorded.
    pub count: u64,
    /// Sum of all samples in nanoseconds.
    pub sum_ns: u64,
}

impl HoldTimeHistogram {
    pub fn record(&mut self, hold_ns: u64) {
        self.count += 1;
        self.sum_ns = self.sum_ns.saturating_add(hold_ns);
        let bucket = match hold_ns {
            0..=999          => 0,
            1_000..=9_999    => 1,
            10_000..=99_999  => 2,
            100_000..=999_999 => 3,
            _                => 4,
        };
        self.buckets[bucket] += 1;
    }

    /// Estimate the p90 hold time in nanoseconds using linear interpolation
    /// within the bucket that contains the 90th-percentile sample.
    pub fn p90_ns(&self) -> u64 {
        if self.count == 0 {
            return 0;
        }
        let target = (self.count * 9 + 9) / 10; // ceil(90% of count)
        let bucket_bounds: [(u64, u64); 5] = [
            (0,       1_000),
            (1_000,   10_000),
            (10_000,  100_000),
            (100_000, 1_000_000),
            (1_000_000, u64::MAX),
        ];
        let mut cumulative = 0u64;
        for (i, &count) in self.buckets.iter().enumerate() {
            cumulative += count;
            if cumulative >= target {
                // Return the upper bound of the bucket as a conservative estimate.
                return bucket_bounds[i].1.saturating_sub(1);
            }
        }
        u64::MAX
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
    /// Number of times a lock acquisition had to wait (contention events).
    pub lock_contention_count: u64,
    /// Histogram of lock hold durations in nanoseconds.
    pub lock_hold_duration_ns: HoldTimeHistogram,
}

impl AllocatorMetrics {
    pub fn new(pool_id: u16) -> Self {
        Self {
            fragmentation_gauge: FragmentationGauge::new(pool_id),
            total_allocations: 0,
            total_frees: 0,
            total_defrags: 0,
            slabs_relocated: 0,
            lock_contention_count: 0,
            lock_hold_duration_ns: HoldTimeHistogram::default(),
        }
    }
}