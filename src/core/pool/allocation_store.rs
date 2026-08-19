use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::core::pool::shard_manager::TenantId;

#[derive(Debug, Clone)]
pub struct AllocationStore {
    allocations: Arc<RwLock<HashMap<TenantId, u64>>>,
}

impl AllocationStore {
    pub fn new() -> Self {
        Self {
            allocations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_active_allocations(&self) -> u64 {
        let lock = self.allocations.read().unwrap();
        lock.values().sum()
    }

    pub fn set_allocation(&self, tenant_id: TenantId, bandwidth: u64) {
        let mut lock = self.allocations.write().unwrap();
        lock.insert(tenant_id, bandwidth);
    }
    
    pub fn get_allocation(&self, tenant_id: &TenantId) -> u64 {
        let lock = self.allocations.read().unwrap();
        lock.get(tenant_id).copied().unwrap_or(0)
    }
}
