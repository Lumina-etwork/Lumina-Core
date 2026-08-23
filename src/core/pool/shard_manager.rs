use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use spin::{RwLock, Mutex};
use core::sync::atomic::{AtomicU64, Ordering};

use super::shard_state::{ShardId, MAX_TENANTS, SHARD_CAPACITY, TOTAL_SHARDS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TenantId(pub u64);

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

const STRIPES: usize = 128;

pub struct ShardManager {
    stripes: Vec<RwLock<ShardManagerStripe>>,
    tokens: AtomicU64,
}

struct ShardManagerStripe {
    free_shards: Vec<ShardId>,
    shard_owner: Vec<Option<TenantId>>,
    tenant_shards: BTreeMap<TenantId, Vec<ShardId>>,
    lock_contention_count: u64,
    lock_hold_duration_ms: [u64; 10],
}

impl ShardManager {
    pub fn new() -> Self {
        let mut stripes = Vec::with_capacity(STRIPES);
        let shards_per_stripe = TOTAL_SHARDS / STRIPES;
        
        for s in 0..STRIPES {
            let start = s * shards_per_stripe;
            let end = if s == STRIPES - 1 { TOTAL_SHARDS } else { start + shards_per_stripe };
            
            let mut free_shards = Vec::new();
            for i in start..end {
                free_shards.push(ShardId(i as u32));
            }
            
            stripes.push(RwLock::new(ShardManagerStripe {
                free_shards,
                shard_owner: vec![None; end - start],
                tenant_shards: BTreeMap::new(),
                lock_contention_count: 0,
                lock_hold_duration_ms: [0; 10],
            }));
        }
        
        Self {
            stripes,
            tokens: AtomicU64::new(10000),
        }
    }

    fn stripe_index_for_tenant(tenant: TenantId) -> usize {
        (tenant.0 as usize) % STRIPES
    }

    fn stripe_index_for_shard(shard: ShardId) -> usize {
        let shards_per_stripe = TOTAL_SHARDS / STRIPES;
        let idx = (shard.0 as usize) / shards_per_stripe;
        if idx >= STRIPES { STRIPES - 1 } else { idx }
    }

    pub fn free_count(&self) -> usize {
        self.stripes.iter().map(|s| s.read().free_shards.len()).sum()
    }

    pub fn shard_capacity(&self) -> usize {
        SHARD_CAPACITY
    }

    pub fn release_shard(&self, tenant: TenantId, shard: ShardId) -> Result<(), ShardError> {
        let home_idx = Self::stripe_index_for_tenant(tenant);
        let shard_idx = Self::stripe_index_for_shard(shard);
        
        if home_idx == shard_idx {
            let mut stripe = self.stripes[home_idx].write();
            let local_shard_idx = shard.0 as usize - (home_idx * (TOTAL_SHARDS / STRIPES));
            
            match stripe.shard_owner[local_shard_idx] {
                Some(owner) if owner == tenant => {}
                _ => return Err(ShardError::ShardNotOwned),
            }
            
            stripe.shard_owner[local_shard_idx] = None;
            stripe.free_shards.push(shard);
            
            if let Some(shards) = stripe.tenant_shards.get_mut(&tenant) {
                shards.retain(|s| *s != shard);
                if shards.is_empty() {
                    stripe.tenant_shards.remove(&tenant);
                }
            }
        } else {
            let (mut min_stripe, mut max_stripe) = if home_idx < shard_idx {
                (self.stripes[home_idx].write(), self.stripes[shard_idx].write())
            } else {
                let s1 = self.stripes[shard_idx].write();
                let s2 = self.stripes[home_idx].write();
                (s2, s1)
            };
            
            let mut home = if home_idx < shard_idx { &mut min_stripe } else { &mut max_stripe };
            let mut foreign = if home_idx < shard_idx { &mut max_stripe } else { &mut min_stripe };
            
            let local_shard_idx = shard.0 as usize - (shard_idx * (TOTAL_SHARDS / STRIPES));
            
            match foreign.shard_owner[local_shard_idx] {
                Some(owner) if owner == tenant => {}
                _ => return Err(ShardError::ShardNotOwned),
            }
            
            foreign.shard_owner[local_shard_idx] = None;
            foreign.free_shards.push(shard);
            
            if let Some(shards) = home.tenant_shards.get_mut(&tenant) {
                shards.retain(|s| *s != shard);
                if shards.is_empty() {
                    home.tenant_shards.remove(&tenant);
                }
            }
        }
        
        Ok(())
    }

    pub fn reclaim_shard(&self, tenant: TenantId) -> Result<ShardId, ShardError> {
        let home_idx = Self::stripe_index_for_tenant(tenant);
        
        {
            let mut home = self.stripes[home_idx].write();
            
            if let Some(shard) = home.free_shards.pop() {
                let local_shard_idx = shard.0 as usize - (home_idx * (TOTAL_SHARDS / STRIPES));
                home.shard_owner[local_shard_idx] = Some(tenant);
                home.tenant_shards.entry(tenant).or_default().push(shard);
                return Ok(shard);
            }
        }
        
        for i in 1..STRIPES {
            let foreign_idx = (home_idx + i) % STRIPES;
            
            let (mut min_stripe, mut max_stripe) = if home_idx < foreign_idx {
                (self.stripes[home_idx].write(), self.stripes[foreign_idx].write())
            } else {
                let s1 = self.stripes[foreign_idx].write();
                let s2 = self.stripes[home_idx].write();
                (s2, s1)
            };
            
            let mut home = if home_idx < foreign_idx { &mut min_stripe } else { &mut max_stripe };
            let mut foreign = if home_idx < foreign_idx { &mut max_stripe } else { &mut min_stripe };
            
            if let Some(shard) = foreign.free_shards.pop() {
                let local_shard_idx = shard.0 as usize - (foreign_idx * (TOTAL_SHARDS / STRIPES));
                foreign.shard_owner[local_shard_idx] = Some(tenant);
                home.tenant_shards.entry(tenant).or_default().push(shard);
                return Ok(shard);
            }
        }
        
        Err(ShardError::NoFreeShards)
    }

    pub fn tenant_shard_ids(&self, tenant: TenantId) -> Vec<ShardId> {
        let home_idx = Self::stripe_index_for_tenant(tenant);
        let stripe = self.stripes[home_idx].read();
        stripe.tenant_shards.get(&tenant).cloned().unwrap_or_default()
    }

    pub fn verify_invariant(&self) -> Result<(), ShardError> {
        let mut free_count = 0;
        let mut assigned_count = 0;
        
        for stripe in &self.stripes {
            let s = stripe.read();
            free_count += s.free_shards.len();
            assigned_count += s.tenant_shards.values().map(|v| v.len()).sum::<usize>();
        }
        
        if free_count + assigned_count != TOTAL_SHARDS {
            return Err(ShardError::InvariantViolation {
                free_count,
                assigned_count,
            });
        }
        Ok(())
    }
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
    fn test_tenant_shard_ids() {
        let mgr = ShardManager::new();
        let t1 = TenantId(1);
        let t2 = TenantId(2);
        
        let s1 = mgr.reclaim_shard(t1).unwrap();
        let s2 = mgr.reclaim_shard(t1).unwrap();
        let s3 = mgr.reclaim_shard(t2).unwrap();
        
        let t1_shards = mgr.tenant_shard_ids(t1);
        assert_eq!(t1_shards.len(), 2);
        assert!(t1_shards.contains(&s1));
        assert!(t1_shards.contains(&s2));
        
        let t2_shards = mgr.tenant_shard_ids(t2);
        assert_eq!(t2_shards.len(), 1);
        assert!(t2_shards.contains(&s3));
        
        let t3_shards = mgr.tenant_shard_ids(TenantId(3));
        assert!(t3_shards.is_empty());
    }
}
