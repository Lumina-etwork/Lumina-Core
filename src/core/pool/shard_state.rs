use std::sync::atomic::{AtomicU64, Ordering};

pub const TOTAL_SHARDS: usize = 100;
pub const SHARD_CAPACITY: usize = 10;
pub const MAX_TENANTS: usize = 50;

/// Unique identifier for a shard (0..TOTAL_SHARDS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShardId(pub u32);

impl ShardId {
    pub fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// Thread-safe bitmask for tracking free/allocated shards.
///
/// Uses two `AtomicU64` values to cover 128 bits (only bits 0..99 are valid).
/// Bit = 1 means the shard is free; bit = 0 means allocated.
pub struct ShardBitmask {
    words: [AtomicU64; 2],
}

impl ShardBitmask {
    /// Creates a bitmask with all `TOTAL_SHARDS` bits set (all free).
    pub fn all_free() -> Self {
        let lo = if TOTAL_SHARDS >= 64 { u64::MAX } else { (1u64 << TOTAL_SHARDS) - 1 };
        let hi_bits = TOTAL_SHARDS.saturating_sub(64);
        let hi = if hi_bits == 0 { 0 } else if hi_bits >= 64 { u64::MAX } else { (1u64 << hi_bits) - 1 };
        Self {
            words: [AtomicU64::new(lo), AtomicU64::new(hi)],
        }
    }

    /// Creates a bitmask with no bits set (all allocated).
    pub fn none_free() -> Self {
        Self {
            words: [AtomicU64::new(0), AtomicU64::new(0)],
        }
    }

    /// Atomically sets a bit (marks shard as free). Returns the previous value of the bit.
    pub fn set_bit(&self, shard: ShardId) -> bool {
        let idx = shard.as_usize();
        assert!(idx < TOTAL_SHARDS, "shard index out of range");
        let (word_idx, bit_pos) = (idx / 64, idx % 64);
        let mask = 1u64 << bit_pos;
        let prev = self.words[word_idx].fetch_or(mask, Ordering::AcqRel);
        (prev & mask) != 0
    }

    /// Atomically clears a bit (marks shard as allocated). Returns the previous value of the bit.
    pub fn clear_bit(&self, shard: ShardId) -> bool {
        let idx = shard.as_usize();
        assert!(idx < TOTAL_SHARDS, "shard index out of range");
        let (word_idx, bit_pos) = (idx / 64, idx % 64);
        let mask = 1u64 << bit_pos;
        let prev = self.words[word_idx].fetch_and(!mask, Ordering::AcqRel);
        (prev & mask) != 0
    }

    /// Returns whether a shard is free.
    pub fn is_free(&self, shard: ShardId) -> bool {
        let idx = shard.as_usize();
        assert!(idx < TOTAL_SHARDS, "shard index out of range");
        let (word_idx, bit_pos) = (idx / 64, idx % 64);
        (self.words[word_idx].load(Ordering::Acquire) >> bit_pos) & 1 == 1
    }

    /// Returns the count of free shards.
    pub fn free_count(&self) -> usize {
        let lo = self.words[0].load(Ordering::Acquire).count_ones() as usize;
        let hi_bits = TOTAL_SHARDS.saturating_sub(64);
        let hi_mask = if hi_bits == 0 { 0 } else if hi_bits >= 64 { u64::MAX } else { (1u64 << hi_bits) - 1 };
        let hi = (self.words[1].load(Ordering::Acquire) & hi_mask).count_ones() as usize;
        lo + hi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_free_has_correct_count() {
        let bm = ShardBitmask::all_free();
        assert_eq!(bm.free_count(), TOTAL_SHARDS);
    }

    #[test]
    fn set_and_clear_bit() {
        let bm = ShardBitmask::none_free();
        assert!(!bm.is_free(ShardId(5)));
        let was_free = bm.set_bit(ShardId(5));
        assert!(!was_free);
        assert!(bm.is_free(ShardId(5)));
        let was_free = bm.clear_bit(ShardId(5));
        assert!(was_free);
        assert!(!bm.is_free(ShardId(5)));
    }

    #[test]
    fn boundary_shards() {
        let bm = ShardBitmask::all_free();
        assert!(bm.is_free(ShardId(0)));
        assert!(bm.is_free(ShardId(63)));
        assert!(bm.is_free(ShardId(64)));
        assert!(bm.is_free(ShardId(99)));
    }

    #[test]
    #[should_panic(expected = "shard index out of range")]
    fn out_of_range_panics() {
        let bm = ShardBitmask::all_free();
        bm.is_free(ShardId(100));
    }
}
