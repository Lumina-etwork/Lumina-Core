pub mod redis_cache;

pub trait Cache {
    fn get(&self, key: &str) -> Option<alloc::vec::Vec<u8>>;
    fn set(&mut self, key: &str, value: alloc::vec::Vec<u8>, ttl_seconds: u64);
}
