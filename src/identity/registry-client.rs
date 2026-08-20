use std::collections::HashMap;

// Using a module definition for the hyphenated file if needed, or just relying on the structure.
#[path = "key-rotation.rs"]
pub mod key_rotation;
use key_rotation::KeyEpoch;

pub struct RegistryClient {
    // Cache the last 3 key versions per node
    pub key_cache: HashMap<String, Vec<KeyEpoch>>,
}

impl RegistryClient {
    pub fn new() -> Self {
        Self {
            key_cache: HashMap::new(),
        }
    }

    pub fn update_key(&mut self, node_id: String, key_epoch: KeyEpoch) {
        let entry = self.key_cache.entry(node_id).or_insert_with(Vec::new);
        entry.push(key_epoch);
        if entry.len() > 3 {
            entry.remove(0);
        }
    }

    pub fn get_keys(&self, node_id: &str) -> Option<&Vec<KeyEpoch>> {
        self.key_cache.get(node_id)
    }
}
