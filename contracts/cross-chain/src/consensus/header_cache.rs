use alloc::vec::Vec;

use crate::types::*;

const MAX_CACHED_HEADERS: usize = 256;

#[derive(Debug, Clone)]
pub struct Header {
    pub height: u64,
    pub timestamp_ms: TimestampMs,
    pub hash: [u8; 32],
    pub parent_hash: [u8; 32],
    pub is_finalized: bool,
}

pub struct HeaderCache {
    headers: Vec<Header>,
}

impl HeaderCache {
    pub fn new() -> Self {
        Self {
            headers: Vec::with_capacity(MAX_CACHED_HEADERS),
        }
    }

    pub fn push(&mut self, header: Header) {
        if self.headers.len() >= MAX_CACHED_HEADERS {
            self.headers.remove(0);
        }
        self.headers.push(header);
    }

    pub fn get(&self, height: u64) -> Option<&Header> {
        self.headers.iter().find(|h| h.height == height)
    }

    pub fn latest(&self) -> Option<&Header> {
        self.headers.last()
    }

    pub fn mark_finalized(&mut self, height: u64) -> bool {
        if let Some(h) = self.headers.iter_mut().find(|h| h.height == height) {
            h.is_finalized = true;
            true
        } else {
            false
        }
    }

    pub fn len(&self) -> usize {
        self.headers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    pub fn finalized_count(&self) -> usize {
        self.headers.iter().filter(|h| h.is_finalized).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TimestampMs;

    fn dummy_header(height: u64, timestamp_ms: TimestampMs) -> Header {
        Header {
            height,
            timestamp_ms,
            hash: [height as u8; 32],
            parent_hash: [0u8; 32],
            is_finalized: false,
        }
    }

    #[test]
    fn cache_evicts_oldest_when_full() {
        let mut cache = HeaderCache::new();
        for i in 0..257 {
            cache.push(dummy_header(i, i * 1000));
        }
        assert_eq!(cache.len(), MAX_CACHED_HEADERS);
        assert!(cache.get(0).is_none());
        assert!(cache.get(1).is_some());
        assert!(cache.get(256).is_some());
    }

    #[test]
    fn mark_finalized() {
        let mut cache = HeaderCache::new();
        cache.push(dummy_header(1, 1000));
        assert!(cache.mark_finalized(1));
        assert!(cache.get(1).unwrap().is_finalized);
        assert!(!cache.mark_finalized(999));
    }

    #[test]
    fn latest_header() {
        let mut cache = HeaderCache::new();
        assert!(cache.latest().is_none());
        cache.push(dummy_header(1, 1000));
        cache.push(dummy_header(2, 2000));
        assert_eq!(cache.latest().unwrap().height, 2);
    }

    #[test]
    fn finalized_count() {
        let mut cache = HeaderCache::new();
        for i in 0..10 {
            cache.push(dummy_header(i, i * 1000));
        }
        assert_eq!(cache.finalized_count(), 0);
        cache.mark_finalized(3);
        cache.mark_finalized(7);
        assert_eq!(cache.finalized_count(), 2);
    }
}
