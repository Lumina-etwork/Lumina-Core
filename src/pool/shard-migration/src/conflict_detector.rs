use crate::coordinator::MigrationBatch;
use std::collections::BTreeMap;

pub struct ConflictDetector {
    // map epoch -> batch
    in_flight: BTreeMap<u64, MigrationBatch>,
}

impl ConflictDetector {
    pub fn new() -> Self {
        Self { in_flight: BTreeMap::new() }
    }

    // register a batch; if there is a conflict with an existing smaller epoch, return Some(conflicting_epoch)
    pub fn check_conflict_and_register(&mut self, epoch: u64, batch: &MigrationBatch) -> Option<u64> {
        // check overlap with any in-flight batch
        for (&e, b) in self.in_flight.iter() {
            if ranges_overlap(b.start, b.end, batch.start, batch.end) {
                // return the lower epoch as the winner
                let winner = if e < epoch { e } else { epoch };
                // if current epoch is winner we still register
                if winner == epoch {
                    self.in_flight.insert(epoch, batch.clone());
                    return None;
                } else {
                    // don't register, signal conflict
                    return Some(e);
                }
            }
        }
        // no conflict, register
        self.in_flight.insert(epoch, batch.clone());
        None
    }

    pub fn deregister(&mut self, epoch: u64) {
        self.in_flight.remove(&epoch);
    }
}

fn ranges_overlap(a_start: u32, a_end: u32, b_start: u32, b_end: u32) -> bool {
    !(a_end < b_start || b_end < a_start)
}
