//! Cache for tracking used nonces per node and epoch, with expiry for old epochs.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Tracks used nonces for each (node_id, epoch_id) pair.
/// Automatically expires entries older than `current_epoch - 2`.
pub struct NonceCache {
    /// Cache entries: node_id -> epoch_id -> set of used nonces.
    entries: BTreeMap<String, BTreeMap<u32, Vec<[u8; 32]>>>,
}

impl NonceCache {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Check if a nonce has already been used for the given node and epoch.
    /// Automatically cleans up entries older than `current_epoch - 2`.
    ///
    /// # Arguments
    /// * `node_id` - The node ID
    /// * `epoch_id` - The epoch ID
    /// * `nonce` - The nonce to check
    /// * `current_epoch` - The current epoch (for expiry)
    ///
    /// # Returns
    /// true if nonce is already used, false otherwise
    pub fn is_used(
        &mut self,
        node_id: &str,
        epoch_id: u32,
        nonce: &[u8; 32],
        current_epoch: u32,
    ) -> bool {
        self.cleanup(current_epoch);

        if let Some(node_entries) = self.entries.get(node_id) {
            if let Some(epoch_nonces) = node_entries.get(&epoch_id) {
                return epoch_nonces.contains(nonce);
            }
        }
        false
    }

    /// Mark a nonce as used for the given node and epoch.
    ///
    /// # Arguments
    /// * `node_id` - The node ID
    /// * `epoch_id` - The epoch ID
    /// * `nonce` - The nonce to mark as used
    /// * `current_epoch` - The current epoch (for expiry)
    pub fn mark_used(&mut self, node_id: &str, epoch_id: u32, nonce: [u8; 32], current_epoch: u32) {
        self.cleanup(current_epoch);

        let node_entries = self
            .entries
            .entry(node_id.to_string())
            .or_default();
        let epoch_nonces = node_entries.entry(epoch_id).or_default();

        if !epoch_nonces.contains(&nonce) {
            epoch_nonces.push(nonce);
        }
    }

    /// Remove all entries for epochs older than `current_epoch - 2`.
    fn cleanup(&mut self, current_epoch: u32) {
        let min_epoch = current_epoch.saturating_sub(2);

        self.entries.retain(|_, node_entries| {
            node_entries.retain(|&epoch, _| epoch >= min_epoch);
            !node_entries.is_empty()
        });
    }
}

impl Default for NonceCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_nonce_used_and_check() {
        let mut cache = NonceCache::new();
        let nonce = [1u8; 32];

        assert!(!cache.is_used("node-1", 1, &nonce, 1));
        cache.mark_used("node-1", 1, nonce, 1);
        assert!(cache.is_used("node-1", 1, &nonce, 1));
    }

    #[test]
    fn cleanup_removes_old_entries() {
        let mut cache = NonceCache::new();
        let nonce1 = [1u8; 32];
        let nonce2 = [2u8; 32];
        let nonce3 = [3u8; 32];

        cache.mark_used("node-1", 0, nonce1, 0);
        cache.mark_used("node-1", 1, nonce2, 1);
        cache.mark_used("node-1", 2, nonce3, 2);

        // Cleanup at epoch 3: min_epoch = 1
        cache.cleanup(3);

        assert!(!cache.is_used("node-1", 0, &nonce1, 3)); // Should be cleaned up
        assert!(cache.is_used("node-1", 1, &nonce2, 3)); // Still valid
        assert!(cache.is_used("node-1", 2, &nonce3, 3)); // Still valid
    }
}
