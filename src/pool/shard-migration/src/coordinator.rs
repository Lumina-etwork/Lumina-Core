use std::sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}};
use std::thread;
use std::time::{Duration, Instant};
use crate::prepare_phase::{Replica, PrepareAck};
use crate::conflict_detector::ConflictDetector;

#[derive(Clone, Debug)]
pub struct MigrationBatch {
    pub start: u32,
    pub end: u32, // inclusive
    pub shards: Vec<u32>,
}

pub struct Coordinator {
    epoch_counter: AtomicU64,
    replicas: Vec<Arc<Mutex<Replica>>>,
    conflict_detector: Arc<Mutex<ConflictDetector>>,
}

impl Coordinator {
    pub fn new(replicas: Vec<Arc<Mutex<Replica>>>) -> Self {
        Self {
            epoch_counter: AtomicU64::new(0),
            replicas,
            conflict_detector: Arc::new(Mutex::new(ConflictDetector::new())),
        }
    }

    fn next_epoch(&self) -> u64 {
        self.epoch_counter.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn submit_migration(&self, batch: MigrationBatch) -> Result<(), &'static str> {
        if batch.shards.len() > 64 {
            return Err("migration batch size exceeds 64 shards");
        }

        let epoch = self.next_epoch();
        let cd = self.conflict_detector.clone();
        let replicas = self.replicas.clone();

        // spawn a thread to run migration lifecycle (prepare -> commit)
        thread::spawn(move || {
            let mut backoff = 100u64; // ms
            loop {
                // Attempt to register batch; if conflict, conflict_detector will tell us to abort and retry
                let mut detector = cd.lock().unwrap();
                if let Some(conflicting_epoch) = detector.check_conflict_and_register(epoch, &batch) {
                    // conflict detected: abort higher epoch (this) and retry after backoff
                    if epoch > conflicting_epoch {
                        drop(detector);
                        thread::sleep(Duration::from_millis(backoff));
                        backoff = (backoff * 2).min(1600);
                        continue;
                    }
                }
                // No blocking conflict; proceed to prepare
                drop(detector);

                // Send prepare to all replicas with a 10 second timeout
                let mut acks = 0usize;
                let start = Instant::now();
                let mut timed_out = false;
                for r in &replicas {
                    if start.elapsed() > Duration::from_secs(10) {
                        timed_out = true;
                        break;
                    }
                    let mut rep = r.lock().unwrap();
                    let ack = rep.handle_prepare(epoch, &batch);
                    if let PrepareAck::Ok = ack {
                        acks += 1;
                    }
                }

                if timed_out {
                    let mut detector = cd.lock().unwrap();
                    detector.deregister(epoch);
                    drop(detector);
                    thread::sleep(Duration::from_millis(backoff));
                    backoff = (backoff * 2).min(1600);
                    continue;
                }

                // commit if have 2f+1 acks; assume f = (N-1)/4, so 2f+1 = majority for 4f+1
                let n = replicas.len();
                let f = (n - 1) / 4;
                let required = 2 * f + 1;
                if acks >= required {
                    // send commit
                    for r in &replicas {
                        let mut rep = r.lock().unwrap();
                        let _ = rep.handle_commit(epoch);
                    }
                    // deregister from conflict detector
                    let mut detector = cd.lock().unwrap();
                    detector.deregister(epoch);
                    break;
                } else {
                    // failed to gather enough acks -> abort and retry
                    let mut detector = cd.lock().unwrap();
                    detector.deregister(epoch);
                    drop(detector);
                    thread::sleep(Duration::from_millis(backoff));
                    backoff = (backoff * 2).min(1600);
                    continue;
                }
            }
        });

        Ok(())
    }
}
