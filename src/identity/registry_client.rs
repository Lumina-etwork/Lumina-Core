use crate::identity::ed25519::store::KeyVersionStore;
use crate::identity::key_rotation::KeyEpoch;
/// Distributed registry client with a versioned key cache.
///
/// Caches up to `MAX_VERSIONS` (3) KeyEpoch records per node.
/// `keys_valid_at` returns every cached version whose validity window
/// covers the requested epoch — the verifier uses this to retry with
/// old keys during the grace period.
use alloc::vec::Vec;

pub struct RegistryClient {
    cache: KeyVersionStore,
}

impl RegistryClient {
    pub fn new() -> Self {
        Self {
            cache: KeyVersionStore::default(),
        }
    }

    /// Publish a newly activated key (or a key with its expiry capped after
    /// a subsequent rotation).  The cache evicts the oldest beyond MAX_VERSIONS.
    pub fn publish(&mut self, node_id: &str, key: KeyEpoch) {
        self.cache.insert(node_id, key);
    }

    /// Cap the expiry of an existing key version (called on rotation commit).
    pub fn cap_expiry(&mut self, node_id: &str, activation_epoch: u64, expiry_epoch: u64) {
        self.cache
            .set_expiry(node_id, activation_epoch, expiry_epoch);
    }

    /// Return all cached key versions valid at `epoch`.
    /// Ordered newest → oldest so the verifier tries the current key first.
    pub fn keys_valid_at(&self, node_id: &str, epoch: u64) -> Vec<&KeyEpoch> {
        let mut keys = self.cache.valid_at(node_id, epoch);
        keys.sort_by(|a, b| b.activation_epoch.cmp(&a.activation_epoch));
        keys
    }

    /// Convenience: return the single most-recent key regardless of epoch.
    pub fn current_key(&self, node_id: &str) -> Option<&KeyEpoch> {
        self.cache.current(node_id)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec;

    #[test]
    fn grace_period_lookup_returns_both_keys() {
        let mut client = RegistryClient::new();
        // Old key: active from epoch 0, expires at epoch 9
        let mut old_key = KeyEpoch::new(vec![1], 0);
        old_key.expiry_epoch = 9;
        client.publish("node-1", old_key);
        // New key: activates at epoch 7
        client.publish("node-1", KeyEpoch::new(vec![2], 7));

        // During grace period (epoch 8): both keys valid
        let valid = client.keys_valid_at("node-1", 8);
        assert_eq!(valid.len(), 2);
        // Newest first
        assert_eq!(valid[0].activation_epoch, 7);
    }

    #[test]
    fn after_grace_only_new_key_valid() {
        let mut client = RegistryClient::new();
        let mut old_key = KeyEpoch::new(vec![1], 0);
        old_key.expiry_epoch = 9;
        client.publish("node-1", old_key);
        client.publish("node-1", KeyEpoch::new(vec![2], 7));

        let valid = client.keys_valid_at("node-1", 9);
        assert_eq!(valid.len(), 1);
        assert_eq!(valid[0].public_key, vec![2]);
    }
}
