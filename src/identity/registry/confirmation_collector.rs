use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use super::conflict_resolver::{resolve_conflict, ConflictResolution, RegistrationAttempt};

pub const ID_COLLISION_WINDOW: Duration = Duration::from_millis(500);
pub const CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectorOutcome {
    Pending,
    Confirmed(RegistrationAttempt),
    ConflictResolved(ConflictResolution),
}

#[derive(Debug)]
struct PendingAttempt {
    attempt: RegistrationAttempt,
    confirmations: HashSet<String>,
    first_seen: Instant,
    threshold_seen: Option<Instant>,
}

#[derive(Debug)]
pub struct ConfirmationCollector {
    threshold: usize,
    pending_registrations: HashMap<String, Vec<PendingAttempt>>,
}

impl ConfirmationCollector {
    pub fn new(f: usize) -> Self {
        Self {
            threshold: 2 * f + 1,
            pending_registrations: HashMap::new(),
        }
    }

    pub fn add_confirmation(
        &mut self,
        node_id: String,
        public_key: Vec<u8>,
        epoch: u64,
        replica_id: String,
        now: Instant,
    ) -> CollectorOutcome {
        let attempts = self
            .pending_registrations
            .entry(node_id.clone())
            .or_default();
        let attempt = attempts
            .iter_mut()
            .find(|pending| pending.attempt.public_key == public_key);

        match attempt {
            Some(pending) => {
                pending.confirmations.insert(replica_id);
                if pending.confirmations.len() >= self.threshold && pending.threshold_seen.is_none()
                {
                    pending.threshold_seen = Some(now);
                }
            }
            None => {
                let mut confirmations = HashSet::new();
                confirmations.insert(replica_id);
                attempts.push(PendingAttempt {
                    attempt: RegistrationAttempt {
                        node_id: node_id.clone(),
                        public_key,
                        epoch,
                    },
                    confirmations,
                    first_seen: now,
                    threshold_seen: None,
                });
            }
        }

        self.evaluate(&node_id, now)
    }

    fn evaluate(&mut self, node_id: &str, now: Instant) -> CollectorOutcome {
        let Some(attempts) = self.pending_registrations.get(node_id) else {
            return CollectorOutcome::Pending;
        };

        let in_window: Vec<_> = attempts
            .iter()
            .filter(|pending| now.duration_since(pending.first_seen) <= ID_COLLISION_WINDOW)
            .filter(|pending| pending.confirmations.len() >= self.threshold)
            .map(|pending| pending.attempt.clone())
            .collect();

        if in_window.len() > 1 {
            let resolved_epoch = in_window
                .iter()
                .map(|attempt| attempt.epoch)
                .max()
                .unwrap_or(0);
            let Some(resolution) = resolve_conflict(in_window, resolved_epoch) else {
                return CollectorOutcome::Pending;
            };
            self.pending_registrations.remove(node_id);
            return CollectorOutcome::ConflictResolved(resolution);
        }

        let ready = attempts.iter().find(|pending| {
            pending
                .threshold_seen
                .is_some_and(|seen| now.duration_since(seen) >= CONFIRMATION_TIMEOUT)
        });

        if let Some(ready) = ready {
            let confirmed = ready.attempt.clone();
            self.pending_registrations.remove(node_id);
            return CollectorOutcome::Confirmed(confirmed);
        }

        CollectorOutcome::Pending
    }
}
