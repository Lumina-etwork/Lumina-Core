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
}

struct CacheEntry {
    session_data: Vec<u8>,
    expires_at: Instant,
}

impl SessionCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            session_cache_collision_attempts: 0,
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
            return; // Cache limit per pool
        }
        let key = Self::compound_key(tenant_id, original_sni);
        self.entries.insert(key, CacheEntry {
            session_data,
            expires_at: Instant::now() + Duration::from_secs(300), // 300s TTL
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
