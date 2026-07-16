use crate::identity::key_rotation::KeyEpoch;
/// Per-node Ed25519 key version store.
///
/// Retains the last `MAX_VERSIONS` (3) KeyEpoch records per node in
/// ascending epoch order. Oldest version is evicted when the cap is exceeded.
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub const MAX_VERSIONS: usize = 3;

#[derive(Default)]
pub struct KeyVersionStore {
    /// node_id → versions sorted by activation_epoch ascending.
    versions: BTreeMap<String, Vec<KeyEpoch>>,
}

impl KeyVersionStore {
    /// Insert a new KeyEpoch for `node_id`.
    /// Evicts the oldest version if `MAX_VERSIONS` is exceeded.
    pub fn insert(&mut self, node_id: &str, key: KeyEpoch) {
        let versions = self.versions.entry(node_id.to_string()).or_default();
        // Keep sorted by activation_epoch; avoid duplicates.
        if let Some(pos) = versions
            .iter()
            .position(|k| k.activation_epoch == key.activation_epoch)
        {
            versions[pos] = key;
        } else {
            versions.push(key);
            versions.sort_by_key(|k| k.activation_epoch);
            if versions.len() > MAX_VERSIONS {
                versions.remove(0); // evict oldest
            }
        }
    }

    /// Set the expiry epoch on the current (highest-activation) key for a node.
    /// Called when a rotation commits, to cap the old key's validity window.
    pub fn set_expiry(&mut self, node_id: &str, activation_epoch: u64, expiry_epoch: u64) {
        if let Some(versions) = self.versions.get_mut(node_id) {
            if let Some(k) = versions
                .iter_mut()
                .find(|k| k.activation_epoch == activation_epoch)
            {
                k.expiry_epoch = expiry_epoch;
            }
        }
    }

    /// Return all versions valid at `epoch` (activation_epoch ≤ epoch < expiry_epoch).
    pub fn valid_at(&self, node_id: &str, epoch: u64) -> Vec<&KeyEpoch> {
        self.versions
            .get(node_id)
            .map(|v| v.iter().filter(|k| k.is_valid_at(epoch)).collect())
            .unwrap_or_default()
    }

    /// Return the most recently activated key regardless of epoch validity.
    pub fn current(&self, node_id: &str) -> Option<&KeyEpoch> {
        self.versions.get(node_id).and_then(|v| v.last())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec;

    #[test]
    fn retains_at_most_three_versions() {
        let mut store = KeyVersionStore::default();
        for i in 0..5u64 {
            store.insert("node-1", KeyEpoch::new(vec![i as u8], i * 2));
        }
        assert_eq!(store.versions["node-1"].len(), MAX_VERSIONS);
        // Oldest retained should be version 2 (activation 4)
        assert_eq!(store.versions["node-1"][0].activation_epoch, 4);
    }

    #[test]
    fn valid_at_respects_expiry() {
        let mut store = KeyVersionStore::default();
        let mut old = KeyEpoch::new(vec![1], 0);
        old.expiry_epoch = 5;
        store.insert("node-1", old);
        store.insert("node-1", KeyEpoch::new(vec![2], 3)); // expiry = u64::MAX

        assert_eq!(store.valid_at("node-1", 4).len(), 2); // both valid at epoch 4
        assert_eq!(store.valid_at("node-1", 5).len(), 1); // old expired
        assert_eq!(store.valid_at("node-1", 6).len(), 1);
    }
}
