use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityRecord {
    pub node_id: String,
    pub public_key: Vec<u8>,
    pub confirmed_epoch: u64,
}

#[derive(Debug, Default)]
pub struct IdentityStore {
    identities: HashMap<String, IdentityRecord>,
    conflict_epochs: HashMap<String, u64>,
}

impl IdentityStore {
    pub fn get(&self, node_id: &str) -> Option<&IdentityRecord> {
        self.identities.get(node_id)
    }

    pub fn commit(&mut self, node_id: String, public_key: Vec<u8>, confirmed_epoch: u64) {
        self.identities.insert(
            node_id.clone(),
            IdentityRecord {
                node_id,
                public_key,
                confirmed_epoch,
            },
        );
    }

    pub fn mark_conflict_resolution(&mut self, node_id: String, epoch: u64) {
        self.conflict_epochs.insert(node_id, epoch);
    }

    pub fn last_conflict_epoch(&self, node_id: &str) -> Option<u64> {
        self.conflict_epochs.get(node_id).copied()
    }
}
