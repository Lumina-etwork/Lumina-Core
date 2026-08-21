use std::sync::atomic::{AtomicU64, Ordering};

pub struct MigrationCoordinator {
    migration_epoch: AtomicU64,
}

impl MigrationCoordinator {
    pub fn new() -> Self {
        Self {
            migration_epoch: AtomicU64::new(0),
        }
    }

    pub fn next_epoch(&self) -> u64 {
        self.migration_epoch.fetch_add(1, Ordering::SeqCst)
    }
}
