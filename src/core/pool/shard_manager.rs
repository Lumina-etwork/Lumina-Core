use std::collections::HashMap;
use std::sync::Mutex;

use super::shard_state::{ShardId, MAX_TENANTS, SHARD_CAPACITY, TOTAL_SHARDS};

/// Unique tenant identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TenantId(pub u64);

/// Error conditions for shard operations.
#[derive(Debug, PartialEq, Eq)]
pub enum ShardError {
    NoFreeShards,
    ShardNotOwned,
    TenantLimitReached,
    ShardAlreadyFree,
    InvariantViolation {
        free_count: usize,
        assigned_count: usize,
    },
}

/// Manages connection pool shards across tenants.
///
/// Uses a `Mutex<Vec<ShardId>>` free list instead of a bitmask to eliminate
/// torn-read races on non-atomic u128 operations (see issue: shard leak
/// under concurrent release/reclaim).
pub struct ShardManager {
    state: Mutex<ShardManagerState>,
}

struct ShardManagerState {
    free_shards: Vec<ShardId>,
    /// Maps each assigned shard to its owning tenant.
    shard_owner: [Option<TenantId>; TOTAL_SHARDS],
    /// Maps tenant to its assigned shards.
    tenant_shards: HashMap<TenantId, Vec<ShardId>>,
}

impl ShardManager {
    pub fn new() -> Self {
        let free_shards: Vec<ShardId> = (0..TOTAL_SHARDS as u32).map(ShardId).collect();
        Self {
            state: Mutex::new(ShardManagerState {
                free_shards,
                shard_owner: [None; TOTAL_SHARDS],
                tenant_shards: HashMap::new(),
            }),
        }
    }

    /// Returns the number of currently free shards.
    pub fn free_count(&self) -> usize {
        self.state.lock().unwrap().free_shards.len()
    }

    /// Returns the shard capacity (connections per shard).
    pub fn shard_capacity(&self) -> usize {
        SHARD_CAPACITY
    }

    /// Releases a shard back to the free pool when a tenant disconnects.
    ///
    /// Verifies the shard invariant after mutation.
    pub fn release_shard(&self, tenant: TenantId, shard: ShardId) -> Result<(), ShardError> {
        let mut state = self.state.lock().unwrap();

        match state.shard_owner[shard.as_usize()] {
            Some(owner) if owner == tenant => {}
            _ => return Err(ShardError::ShardNotOwned),
        }

        state.shard_owner[shard.as_usize()] = None;
        state.free_shards.push(shard);

        if let Some(shards) = state.tenant_shards.get_mut(&tenant) {
            shards.retain(|s| *s != shard);
            if shards.is_empty() {
                state.tenant_shards.remove(&tenant);
            }
        }

        verify_shard_invariant(&state)?;
        Ok(())
    }

    /// Reclaims a free shard and assigns it to a tenant.
    ///
    /// Verifies the shard invariant after mutation.
    pub fn reclaim_shard(&self, tenant: TenantId) -> Result<ShardId, ShardError> {
        let mut state = self.state.lock().unwrap();

        if state.tenant_shards.len() >= MAX_TENANTS && !state.tenant_shards.contains_key(&tenant) {
            return Err(ShardError::TenantLimitReached);
        }

        let shard = state.free_shards.pop().ok_or(ShardError::NoFreeShards)?;

        state.shard_owner[shard.as_usize()] = Some(tenant);
        state.tenant_shards.entry(tenant).or_default().push(shard);

        verify_shard_invariant(&state)?;
        Ok(shard)
    }

    /// Returns a snapshot of shard IDs assigned to a tenant.
    pub fn tenant_shard_ids(&self, tenant: TenantId) -> Vec<ShardId> {
        let state = self.state.lock().unwrap();
        state
            .tenant_shards
            .get(&tenant)
            .cloned()
            .unwrap_or_default()
    }

    /// Explicitly runs the invariant check. Useful for external callers.
    pub fn verify_invariant(&self) -> Result<(), ShardError> {
        let state = self.state.lock().unwrap();
        verify_shard_invariant(&state)
    }
}

/// Consistency checker: asserts that free_count + assigned_count == TOTAL_SHARDS.
fn verify_shard_invariant(state: &ShardManagerState) -> Result<(), ShardError> {
    let free_count = state.free_shards.len();
    let assigned_count: usize = state.tenant_shards.values().map(|v| v.len()).sum();

    if free_count + assigned_count != TOTAL_SHARDS {
        return Err(ShardError::InvariantViolation {
            free_count,
            assigned_count,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_manager_has_all_free() {
        let mgr = ShardManager::new();
        assert_eq!(mgr.free_count(), TOTAL_SHARDS);
        mgr.verify_invariant().unwrap();
    }

    #[test]
    fn reclaim_and_release_round_trip() {
        let mgr = ShardManager::new();
        let t = TenantId(1);
        let shard = mgr.reclaim_shard(t).unwrap();
        assert_eq!(mgr.free_count(), TOTAL_SHARDS - 1);
        mgr.release_shard(t, shard).unwrap();
        assert_eq!(mgr.free_count(), TOTAL_SHARDS);
        mgr.verify_invariant().unwrap();
    }

    #[test]
    fn release_unowned_shard_fails() {
        let mgr = ShardManager::new();
        let t1 = TenantId(1);
        let t2 = TenantId(2);
        let shard = mgr.reclaim_shard(t1).unwrap();
        assert_eq!(mgr.release_shard(t2, shard), Err(ShardError::ShardNotOwned));
    }

    #[test]
    fn exhaust_all_shards() {
        let mgr = ShardManager::new();
        let t = TenantId(1);
        for _ in 0..TOTAL_SHARDS {
            mgr.reclaim_shard(t).unwrap();
        }
        assert_eq!(mgr.reclaim_shard(t), Err(ShardError::NoFreeShards));
        assert_eq!(mgr.free_count(), 0);
        mgr.verify_invariant().unwrap();
    }

    #[test]
    fn tenant_limit_enforced() {
        let mgr = ShardManager::new();
        for i in 0..MAX_TENANTS as u64 {
            mgr.reclaim_shard(TenantId(i)).unwrap();
        }
        assert_eq!(
            mgr.reclaim_shard(TenantId(MAX_TENANTS as u64)),
            Err(ShardError::TenantLimitReached)
        );
    }
}
