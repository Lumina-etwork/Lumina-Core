use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::confirmation_collector::{CollectorOutcome, ConfirmationCollector};
use super::storage::IdentityStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    IdRegistrationConflict,
    ConflictCooldownActive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationResult {
    Pending,
    Committed,
}

#[derive(Debug, Clone)]
pub struct RegistrationHandler {
    collector: Arc<Mutex<ConfirmationCollector>>,
    storage: Arc<Mutex<IdentityStore>>,
}

impl RegistrationHandler {
    pub fn new(f: usize) -> Self {
        Self {
            collector: Arc::new(Mutex::new(ConfirmationCollector::new(f))),
            storage: Arc::new(Mutex::new(IdentityStore::default())),
        }
    }

    pub fn register_confirmation(
        &self,
        node_id: String,
        public_key: Vec<u8>,
        epoch: u64,
        replica_id: String,
        now: Instant,
    ) -> Result<RegistrationResult, RegistrationError> {
        if self.is_in_cooldown(&node_id, epoch) {
            return Err(RegistrationError::ConflictCooldownActive);
        }

        let outcome = self
            .collector
            .lock()
            .expect("confirmation collector lock poisoned")
            .add_confirmation(node_id.clone(), public_key.clone(), epoch, replica_id, now);

        match outcome {
            CollectorOutcome::Pending => Ok(RegistrationResult::Pending),
            CollectorOutcome::Confirmed(attempt) => {
                self.storage
                    .lock()
                    .expect("identity store lock poisoned")
                    .commit(attempt.node_id, attempt.public_key, epoch);
                Ok(RegistrationResult::Committed)
            }
            CollectorOutcome::ConflictResolved(resolution) => {
                let mut storage = self.storage.lock().expect("identity store lock poisoned");
                storage.mark_conflict_resolution(node_id.clone(), epoch);
                storage.commit(
                    resolution.winner.node_id,
                    resolution.winner.public_key.clone(),
                    epoch,
                );
                if resolution.winner.public_key == public_key {
                    Ok(RegistrationResult::Committed)
                } else {
                    Err(RegistrationError::IdRegistrationConflict)
                }
            }
        }
    }

    pub fn committed_key(&self, node_id: &str) -> Option<Vec<u8>> {
        self.storage
            .lock()
            .expect("identity store lock poisoned")
            .get(node_id)
            .map(|record| record.public_key.clone())
    }

    fn is_in_cooldown(&self, node_id: &str, epoch: u64) -> bool {
        self.storage
            .lock()
            .expect("identity store lock poisoned")
            .last_conflict_epoch(node_id)
            .is_some_and(|conflict_epoch| epoch <= conflict_epoch + 1)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Instant;

    use super::*;
    #[test]
    fn concurrent_registrations_commit_exactly_one_identity() {
        let handler = Arc::new(RegistrationHandler::new(1));
        let barrier = Arc::new(Barrier::new(5));
        let node_id = "node-7".to_string();
        let keys: Vec<Vec<u8>> = (0u8..5)
            .map(|suffix| vec![b'k', b'e', b'y', suffix])
            .collect();
        let mut threads = Vec::new();
        for key in keys.clone() {
            let handler = Arc::clone(&handler);
            let barrier = Arc::clone(&barrier);
            let node_id = node_id.clone();
            threads.push(thread::spawn(move || {
                barrier.wait();
                let mut final_result = Ok(RegistrationResult::Pending);
                for replica in 0..3 {
                    final_result = handler.register_confirmation(
                        node_id.clone(),
                        key.clone(),
                        10,
                        format!("replica-{replica}"),
                        Instant::now(),
                    );
                }
                final_result
            }));
        }

        for thread in threads {
            let _ = thread.join().expect("thread completed");
        }

        let committed_key = handler.committed_key(&node_id).expect("one key committed");
        assert!(keys.contains(&committed_key));
    }

    #[test]
    fn rejects_registration_during_conflict_cooldown() {
        let handler = RegistrationHandler::new(1);
        let now = Instant::now();
        for replica in 0..3 {
            let _ = handler.register_confirmation(
                "node-8".to_string(),
                b"key-a".to_vec(),
                5,
                format!("a-{replica}"),
                now,
            );
            let _ = handler.register_confirmation(
                "node-8".to_string(),
                b"key-b".to_vec(),
                5,
                format!("b-{replica}"),
                now,
            );
        }

        assert_eq!(
            handler.register_confirmation(
                "node-8".to_string(),
                b"key-c".to_vec(),
                6,
                "replica-x".to_string(),
                Instant::now(),
            ),
            Err(RegistrationError::ConflictCooldownActive)
        );
    }
}
