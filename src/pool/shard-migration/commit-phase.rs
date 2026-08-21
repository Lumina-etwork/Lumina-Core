pub struct CommitPhase {
    pub local_last_committed_epoch: u64,
}

impl CommitPhase {
    pub fn new() -> Self {
        Self {
            local_last_committed_epoch: 0,
        }
    }

    pub fn commit(&mut self, epoch: u64) -> Result<(), &'static str> {
        if epoch != self.local_last_committed_epoch + 1 {
            return Err("Rejected commit: migration_epoch is not exactly local_last_committed_epoch + 1");
        }
        self.local_last_committed_epoch = epoch;
        Ok(())
    }
}
