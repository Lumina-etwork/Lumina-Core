use super::Cache;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use alloc::string::String;

pub struct RedisCacheMock {
    store: BTreeMap<String, (Vec<u8>, u64)>,
}

impl RedisCacheMock {
    pub fn new() -> Self {
        Self {
            store: BTreeMap::new(),
        }
    }
}

impl Cache for RedisCacheMock {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        // In a real implementation, this would query Redis
        self.store.get(key).map(|(v, _ttl)| v.clone())
    }

    fn set(&mut self, key: &str, value: Vec<u8>, ttl_seconds: u64) {
        // In a real implementation, this would set the value with a TTL in Redis
        self.store.insert(String::from(key), (value, ttl_seconds));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_set_get() {
        let mut cache = RedisCacheMock::new();
        cache.set("test_key", alloc::vec![1, 2, 3], 60);
        assert_eq!(cache.get("test_key"), Some(alloc::vec![1, 2, 3]));
        assert_eq!(cache.get("missing_key"), None);
    }
}
