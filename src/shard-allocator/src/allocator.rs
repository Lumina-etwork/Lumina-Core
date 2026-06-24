/// Buddy-tree free-list slab allocator.
///
/// Allocates 64 KB fixed-size slabs to tenants. The buddy tree tracks
/// free contiguous regions at power-of-two granularities, enabling
/// coalescing of adjacent freed slabs into larger blocks.
use crate::{BUDDY_LEVELS, MAX_TENANTS, ShardId, ShardAllocEvent, PoolId};
use std::collections::VecDeque;

/// A single slab — 64 KB of memory for one tenant.
#[derive(Clone, Debug)]
pub struct Slab {
    /// The tenant (shard) assigned to this slab, if any.
    pub owner: Option<ShardId>,
    /// Index of this slab in the pool's slab array.
    pub index: u16,
}

impl Slab {
    pub fn new(index: u16) -> Self {
        Self { owner: None, index }
    }

    pub fn is_free(&self) -> bool {
        self.owner.is_none()
    }
}

/// A node in the buddy tree.
///
/// Each node represents a contiguous region of size 2^order slabs.
/// The tree is a complete binary tree with `MAX_TENANTS` leaves.
#[derive(Clone, Debug)]
struct BuddyNode {
    /// Whether this entire region is free (no allocations).
    free: bool,
    /// Start index in the slab array.
    start: usize,
    /// Number of slabs in this region.
    size: usize,
    /// Order: log2(size) — leaf nodes have order 0.
    order: usize,
}

/// Buddy-tree free-list allocator.
pub struct BuddyAllocator {
    /// All slabs in the pool.
    slabs: Vec<Slab>,
    /// Buddy tree nodes indexed by (level, index).
    tree: Vec<Vec<BuddyNode>>,
    /// Free-list per level: list of completely free node indices at each level.
    free_lists: Vec<VecDeque<usize>>,
    /// Total slabs managed.
    total_slabs: usize,
    /// Currently allocated slabs.
    allocated: usize,
    /// Event log.
    pub events: Vec<ShardAllocEvent>,
    pool_id: PoolId,
}

impl BuddyAllocator {
    /// Create a new allocator with `total_slabs` slabs.
    pub fn new(total_slabs: usize, pool_id: PoolId) -> Self {
        assert!(total_slabs <= MAX_TENANTS);
        assert!(total_slabs.is_power_of_two());

        let slabs: Vec<Slab> = (0..total_slabs).map(|i| Slab::new(i as u16)).collect();

        let levels = total_slabs.ilog2() as usize + 1;
        let mut tree: Vec<Vec<BuddyNode>> = Vec::with_capacity(levels);
        let mut free_lists: Vec<VecDeque<usize>> = Vec::with_capacity(levels);

        // Build buddy tree bottom-up
        for level in 0..levels {
            let node_count = total_slabs >> level;
            let node_size = 1 << level;
            let mut nodes = Vec::with_capacity(node_count);
            let mut free = VecDeque::new();

            for i in 0..node_count {
                let start = i * node_size;
                nodes.push(BuddyNode {
                    free: true,
                    start,
                    size: node_size,
                    order: level,
                });
                free.push_back(i);
            }
            tree.push(nodes);
            free_lists.push(free);
        }

        Self {
            slabs,
            tree,
            free_lists,
            total_slabs,
            allocated: 0,
            events: Vec::new(),
            pool_id,
        }
    }

    /// Allocate a slab for a tenant. Returns the slab index.
    pub fn allocate(&mut self, shard_id: ShardId) -> Result<u16, AllocError> {
        // Find smallest free region at leaf level (order 0)
        if self.free_lists[0].is_empty() {
            // Try to split larger blocks
            if !self.split_to_fit(0) {
                return Err(AllocError::OutOfMemory {
                    total: self.total_slabs,
                    allocated: self.allocated,
                });
            }
        }

        let leaf_idx = self.free_lists[0].pop_front()
            .ok_or(AllocError::OutOfMemory {
                total: self.total_slabs,
                allocated: self.allocated,
            })?;

        let slab_idx = self.tree[0][leaf_idx].start;
        let slab = &mut self.slabs[slab_idx];
        slab.owner = Some(shard_id);
        self.allocated += 1;

        // Mark the leaf as allocated
        self.tree[0][leaf_idx].free = false;

        // Propagate the allocation up the tree
        self.propagate_allocated(0, leaf_idx);

        self.events.push(ShardAllocEvent::ShardAllocated {
            pool: self.pool_id,
            shard_id,
            slab_index: slab_idx as u16,
        });

        Ok(slab_idx as u16)
    }

    /// Free a slab. Returns the shard ID that was freed.
    pub fn free(&mut self, slab_index: u16) -> Result<ShardId, AllocError> {
        let slab = self.slabs.get(slab_index as usize)
            .ok_or(AllocError::InvalidSlab { index: slab_index })?;

        let shard_id = slab.owner
            .ok_or(AllocError::AlreadyFree { index: slab_index })?;

        let slab = &mut self.slabs[slab_index as usize];
        slab.owner = None;
        self.allocated -= 1;

        // Mark the leaf as free and coalesce upward
        let leaf_idx = slab_index as usize;
        self.tree[0][leaf_idx].free = true;
        self.free_lists[0].push_back(leaf_idx);

        // Coalesce buddies upward
        self.coalesce(0, leaf_idx);

        self.events.push(ShardAllocEvent::ShardFreed {
            pool: self.pool_id,
            shard_id,
            slab_index,
        });

        Ok(shard_id)
    }

    /// Split a block at the given level to create a free leaf.
    fn split_to_fit(&mut self, target_level: usize) -> bool {
        for level in (target_level + 1..BUDDY_LEVELS.min(self.tree.len())).rev() {
            if let Some(node_idx) = self.free_lists[level].pop_front() {
                let node = &self.tree[level][node_idx];
                if node.size == 1 {
                    continue;
                }
                // Split into left and right children
                let left_idx = node_idx * 2;
                let right_idx = node_idx * 2 + 1;
                let child_level = level - 1;

                self.tree[child_level][left_idx].free = true;
                self.tree[child_level][right_idx].free = true;
                self.free_lists[child_level].push_back(left_idx);
                self.free_lists[child_level].push_back(right_idx);

                self.tree[level][node_idx].free = false;
                return true;
            }
        }
        false
    }

    /// Propagate an allocation upward — mark parent as not free.
    fn propagate_allocated(&mut self, level: usize, idx: usize) {
        if level + 1 >= self.tree.len() {
            return;
        }
        let parent_idx = idx / 2;
        let sibling_idx = idx ^ 1;

        let sibling_free = self.tree[level]
            .get(sibling_idx)
            .map(|n| n.free)
            .unwrap_or(true);

        if !sibling_free {
            self.tree[level + 1][parent_idx].free = false;
            // Remove from parent free list if present
            self.free_lists[level + 1].retain(|&i| i != parent_idx);
            self.propagate_allocated(level + 1, parent_idx);
        }
    }

    /// Coalesce freed buddies upward.
    fn coalesce(&mut self, level: usize, idx: usize) {
        if level + 1 >= self.tree.len() {
            return;
        }
        let buddy_idx = idx ^ 1;
        let parent_idx = idx / 2;

        let buddy_free = self.tree[level]
            .get(buddy_idx)
            .map(|n| n.free)
            .unwrap_or(false);

        if buddy_free {
            // Both buddies are free — coalesce
            self.tree[level][idx].free = false;
            self.tree[level][buddy_idx].free = false;
            self.free_lists[level].retain(|&i| i != idx && i != buddy_idx);

            self.tree[level + 1][parent_idx].free = true;
            // Add to parent free list if not already there
            if !self.free_lists[level + 1].contains(&parent_idx) {
                self.free_lists[level + 1].push_back(parent_idx);
            }

            self.coalesce(level + 1, parent_idx);
        }
    }

    /// Get the number of free contiguous regions (fragmentation metric).
    /// Counts only maximal free regions — nodes whose parent is not free.
    pub fn free_contiguous_regions(&self) -> usize {
        let mut count = 0;
        for level in 0..self.tree.len() {
            for (i, node) in self.tree[level].iter().enumerate() {
                if !node.free {
                    continue;
                }
                // Check if parent is free — if so, this is not a maximal region
                if level + 1 < self.tree.len() {
                    let parent_idx = i / 2;
                    if self.tree[level + 1][parent_idx].free {
                        continue;
                    }
                }
                count += 1;
            }
        }
        count
    }

    /// Get the fragmentation ratio: 0 = perfect (1 contigous block), 1 = fully fragmented.
    pub fn fragmentation_ratio(&self) -> f64 {
        if self.free_slabs() <= 1 {
            return 0.0;
        }
        let regions = self.free_contiguous_regions();
        if regions <= 1 {
            return 0.0;
        }
        let free = self.free_slabs() as f64;
        let regions = regions as f64;
        // Normalize: ideal = 1 region, worst = free_slabs regions
        (regions - 1.0) / (free - 1.0)
    }

    pub fn free_slabs(&self) -> usize {
        self.total_slabs - self.allocated
    }

    pub fn allocated_slabs(&self) -> usize {
        self.allocated
    }

    pub fn total_slabs(&self) -> usize {
        self.total_slabs
    }

    pub fn pool_id(&self) -> PoolId {
        self.pool_id
    }

    /// Get the owner of a slab at the given index.
    pub fn slab_owner(&self, index: u16) -> Option<ShardId> {
        self.slabs.get(index as usize).and_then(|s| s.owner)
    }

    /// Get all allocated slab indices with their owners.
    pub fn allocated_slabs_list(&self) -> Vec<(u16, ShardId)> {
        self.slabs
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.owner.map(|o| (i as u16, o)))
            .collect()
    }
}

#[derive(Clone, Debug)]
pub enum AllocError {
    OutOfMemory { total: usize, allocated: usize },
    InvalidSlab { index: u16 },
    AlreadyFree { index: u16 },
}

impl std::fmt::Display for AllocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfMemory { total, allocated } => {
                write!(f, "Out of memory: {allocated}/{total} slabs allocated")
            }
            Self::InvalidSlab { index } => {
                write!(f, "Invalid slab index: {index}")
            }
            Self::AlreadyFree { index } => {
                write!(f, "Slab {index} is already free")
            }
        }
    }
}

impl std::error::Error for AllocError {}