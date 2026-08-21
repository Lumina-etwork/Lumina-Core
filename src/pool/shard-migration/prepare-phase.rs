pub struct PrepareMessage {
    pub migration_epoch: u64,
    pub shard_ranges: Vec<(u64, u64)>,
}

pub struct PreparePhase {
    pub prepared_epochs: Vec<u64>,
}

impl PreparePhase {
    pub fn new() -> Self {
        Self {
            prepared_epochs: Vec::new(),
        }
    }

    pub fn receive_prepare(&mut self, msg: PrepareMessage) {
        self.prepared_epochs.push(msg.migration_epoch);
        self.prepared_epochs.sort_unstable();
    }
}
