#[cfg(test)]
mod tests {
    use crate::core::pool::bandwidth_allocator::{BandwidthAllocator, TOTAL_POOL_BANDWIDTH};
    use crate::core::pool::shard_manager::TenantId;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_concurrent_allocation() {
        let allocator = Arc::new(BandwidthAllocator::new(TOTAL_POOL_BANDWIDTH));
        let num_tenants = 50;
        let mut handles = vec![];

        for i in 0..num_tenants {
            let allocator_clone = Arc::clone(&allocator);
            let tenant_id = TenantId(i as u64);
            let handle = thread::spawn(move || {
                // Request random bandwidth between 1-100 Mbps (using pseudo-random for determinism or simple calculation)
                let requested = (i % 100) + 1;
                let _ = allocator_clone.allocate_bandwidth(tenant_id, requested);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let total_allocated = allocator.get_active_allocations();
        assert!(
            total_allocated <= TOTAL_POOL_BANDWIDTH,
            "Total allocated {} exceeds pool bandwidth {}",
            total_allocated,
            TOTAL_POOL_BANDWIDTH
        );
    }
}
