use std::collections::HashMap;
use crate::crypto::types::Hash;
use super::types::CommitteeSet;

pub struct AggregateCache {
    cache: HashMap<(Hash, Hash, u64), Vec<u8>>, // (view_hash, message_hash, committee_epoch)
}

impl AggregateCache {
    pub fn new() -> Self {
        Self { cache: HashMap::new() }
    }

    pub fn get(&self, view_hash: &Hash, message_hash: &Hash, committee: &CommitteeSet) -> Option<&Vec<u8>> {
        let key = (view_hash.clone(), message_hash.clone(), committee.committee_epoch);
        self.cache.get(&key)
    }

    pub fn insert(&mut self, view_hash: Hash, message_hash: Hash, committee: &CommitteeSet, agg: Vec<u8>) {
        let key = (view_hash, message_hash, committee.committee_epoch);
        self.cache.insert(key, agg);
    }
}
