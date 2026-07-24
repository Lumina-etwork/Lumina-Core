use chrono::Utc;
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::endpoint_cache::EndpointCache;

/// Registry of known relay nodes.
/// Acts as the source of truth for which relays are authorized.
pub struct RelayRegistry {
    relays: HashMap<String, RelayInfo>,
}

/// Information about a registered relay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayInfo {
    pub relay_id: String,
    pub public_key_bytes: Vec<u8>, // Ed25519 public key raw bytes
    pub registered_at: u64,        // unix timestamp
    pub active: bool,
}

impl Default for RelayRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RelayRegistry {
    pub fn new() -> Self {
        Self {
            relays: HashMap::new(),
        }
    }

    /// Register a relay. Called by admin or on successful relay join.
    pub fn register_relay(&mut self, relay_id: &str, public_key: &VerifyingKey) -> RelayInfo {
        let info = RelayInfo {
            relay_id: relay_id.to_string(),
            public_key_bytes: public_key.to_bytes().to_vec(),
            registered_at: Utc::now().timestamp() as u64,
            active: true,
        };
        self.relays.insert(relay_id.to_string(), info.clone());
        info
    }

    /// Deactivate a relay (admin action).
    pub fn deactivate_relay(&mut self, relay_id: &str) {
        if let Some(info) = self.relays.get_mut(relay_id) {
            info.active = false;
        }
    }

    /// Check if a relay is registered and active.
    pub fn is_active(&self, relay_id: &str) -> bool {
        self.relays.get(relay_id).map(|i| i.active).unwrap_or(false)
    }

    /// Get relay info.
    pub fn get_relay(&self, relay_id: &str) -> Option<&RelayInfo> {
        self.relays.get(relay_id)
    }

    /// Get all active relays.
    pub fn active_relays(&self) -> Vec<&RelayInfo> {
        self.relays.values().filter(|i| i.active).collect()
    }

    /// Sync relay public keys into the endpoint cache.
    /// Call this after startup or registry changes.
    pub fn sync_to_cache(&self, cache: &mut EndpointCache) {
        for (id, info) in &self.relays {
            if info.active {
                if let Ok(bytes) = <[u8; 32]>::try_from(info.public_key_bytes.as_slice()) {
                    if let Ok(pk) = VerifyingKey::from_bytes(&bytes) {
                        cache.register_relay(id.clone(), pk);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::relay_ticket::generate_relay_keypair;

    #[test]
    fn test_register_and_query() {
        let (_sk, vk) = generate_relay_keypair();
        let mut registry = RelayRegistry::new();
        let info = registry.register_relay("relay_1", &vk);
        assert!(info.active);
        assert!(registry.is_active("relay_1"));
        assert!(!registry.is_active("unknown"));
    }

    #[test]
    fn test_deactivate() {
        let (_, vk) = generate_relay_keypair();
        let mut registry = RelayRegistry::new();
        registry.register_relay("relay_1", &vk);
        registry.deactivate_relay("relay_1");
        assert!(!registry.is_active("relay_1"));
    }

    #[test]
    fn test_sync_to_cache() {
        let (_, vk) = generate_relay_keypair();
        let mut registry = RelayRegistry::new();
        registry.register_relay("relay_1", &vk);

        let config = super::super::endpoint_cache::CacheConfig::default();
        let mut cache = EndpointCache::new(config);
        registry.sync_to_cache(&mut cache);

        let stats = cache.stats();
        assert_eq!(stats.registered_relays, 1);
    }
}
