use std::collections::BTreeMap;
use crate::coordinator::MigrationBatch;

#[derive(Debug)]
pub enum PrepareAck { Ok, Reject }

#[derive(Clone, Debug)]
pub struct Replica {
    pub id: u32,
    // map of epoch -> MigrationBatch that are prepared but not yet committed
    pub prepared: BTreeMap<u64, MigrationBatch>,
    pub last_committed_epoch: u64,
}

impl Replica {
    pub fn new(id: u32) -> Self {
        Self { id, prepared: BTreeMap::new(), last_committed_epoch: 0 }
    }

    // replicas must apply prepares in order of epoch: they accept prepare if epoch == last_committed_epoch+1 or greater (queue)
    pub fn handle_prepare(&mut self, epoch: u64, batch: &MigrationBatch) -> PrepareAck {
        // simple ordering: allow prepare and store it; real node would validate ranges etc.
        self.prepared.insert(epoch, batch.clone());
        PrepareAck::Ok
    }

    // commit-phase: only accept commit if epoch == last_committed_epoch+1
    pub fn handle_commit(&mut self, epoch: u64) -> Result<(), &'static str> {
        if epoch != self.last_committed_epoch + 1 {
            return Err("commit epoch not next");
        }
        // apply commit
        self.prepared.remove(&epoch);
        self.last_committed_epoch = epoch;
        Ok(())
    }
}
