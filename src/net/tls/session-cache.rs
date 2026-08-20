use std::collections::HashMap;
use std::time::{Instant, Duration};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

#[path = "../../pool/tenant-registry.rs"]
pub mod tenant_registry;
use tenant_registry::TenantId;

pub struct SessionCache {
    entries: HashMap<String, CacheEntry>,
    pub session_cache_collision_attempts: u64,
    ttl: Duration,
}

struct CacheEntry {
    session_data: Vec<u8>,
    expires_at: Instant,
}

impl Default for SessionCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionCache {
    pub fn new() -> Self {
        Self::with_ttl(300)
    }

    pub fn with_ttl(ttl_secs: u64) -> Self {
        let clamped_ttl = ttl_secs.clamp(60, 600);
        Self {
            entries: HashMap::new(),
            session_cache_collision_attempts: 0,
            ttl: Duration::from_secs(clamped_ttl),
        }
    }

    pub fn compound_key(tenant_id: &TenantId, original_sni: &str) -> String {
        let mut hasher = DefaultHasher::new();
        tenant_id.0.hash(&mut hasher);
        original_sni.hash(&mut hasher);
        hasher.finish().to_string()
    }

    pub fn put(&mut self, tenant_id: &TenantId, original_sni: &str, session_data: Vec<u8>) {
        if self.entries.len() >= 1024 {
            return; // Cache limit per pool (1,024 entries)
        }
        let key = Self::compound_key(tenant_id, original_sni);
        self.entries.insert(key, CacheEntry {
            session_data,
            expires_at: Instant::now() + self.ttl,
        });
    }

    pub fn get(&mut self, tenant_id: &TenantId, original_sni: &str) -> Option<Vec<u8>> {
        let key = Self::compound_key(tenant_id, original_sni);
        if let Some(entry) = self.entries.get(&key) {
            if Instant::now() < entry.expires_at {
                return Some(entry.session_data.clone());
            }
        }
        None
    }
    
    pub fn increment_collision_attempts(&mut self) {
        self.session_cache_collision_attempts += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compound_key_uniqueness() {
        let tenant_a = TenantId("tenant-100".to_string());
        let tenant_b = TenantId("tenant-200".to_string());
        let sni = "api.lumina.network";

        let key_a = SessionCache::compound_key(&tenant_a, sni);
        let key_b = SessionCache::compound_key(&tenant_b, sni);
        assert_ne!(key_a, key_b, "Compound keys for different tenants must not collide");
    }

    #[test]
    fn test_cache_limit() {
        let mut cache = SessionCache::new();
        for i in 0..1024 {
            let tenant = TenantId(format!("tenant_{}", i));
            cache.put(&tenant, "example.com", vec![1]);
        }
        assert_eq!(cache.entries.len(), 1024);

        // 1025th entry should not exceed limit
        let overflow_tenant = TenantId("tenant_overflow".to_string());
        cache.put(&overflow_tenant, "example.com", vec![2]);
        assert_eq!(cache.entries.len(), 1024);
        assert!(cache.get(&overflow_tenant, "example.com").is_none());
    }

    #[test]
    fn test_configurable_ttl_clamping() {
        let cache_low = SessionCache::with_ttl(10);
        assert_eq!(cache_low.ttl, Duration::from_secs(60));

        let cache_high = SessionCache::with_ttl(1000);
        assert_eq!(cache_high.ttl, Duration::from_secs(600));

        let cache_normal = SessionCache::with_ttl(450);
        assert_eq!(cache_normal.ttl, Duration::from_secs(450));
    }
}
