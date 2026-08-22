use crate::core::pool::allocation_store::AllocationStore;
use crate::core::pool::shard_manager::TenantId;
use alloc::sync::Arc;
use spin::Mutex;

pub const TOTAL_POOL_BANDWIDTH: u64 = 1000;
pub const ALLOCATION_GRANULARITY: u64 = 1;

#[derive(Debug, Clone)]
pub struct BandwidthAllocator {
    store: AllocationStore,
    total_bandwidth: u64,
    pool_alloc_lock: Arc<Mutex<()>>,
}

impl BandwidthAllocator {
    pub fn new(total_bandwidth: u64) -> Self {
        Self {
            store: AllocationStore::new(),
            total_bandwidth,
            pool_alloc_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn allocate_bandwidth(
        &self,
        tenant_id: TenantId,
        requested: u64,
    ) -> Result<(), &'static str> {
        let _lock = self.pool_alloc_lock.lock();

        let current_allocation = self.store.get_allocation(&tenant_id);
        let active_allocations = self.store.get_active_allocations();

        let available = self
            .total_bandwidth
            .saturating_sub(active_allocations.saturating_sub(current_allocation));

        if requested <= available {
            self.store.set_allocation(tenant_id, requested);
            Ok(())
        } else {
            Err("Not enough bandwidth")
        }
    }

    pub fn get_active_allocations(&self) -> u64 {
        self.store.get_active_allocations()
    }
}
