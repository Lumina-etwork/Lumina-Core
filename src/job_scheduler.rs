use alloc::collections::BTreeMap;
use alloc::string::String;

/// Monotonic milliseconds used by the scheduler.
pub type TimestampMillis = u64;

/// Stable identifier for a scheduled job.
pub type JobId = u64;

/// Worker identity authenticated by the caller before it reaches the scheduler.
pub type WorkerId = String;

/// Job priority: larger values are claimed first, then older enqueue time wins.
pub type Priority = u16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobState {
    Queued,
    Leased {
        worker_id: WorkerId,
        expires_at: TimestampMillis,
        epoch: u64,
    },
    Completed,
    Failed {
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Job {
    pub id: JobId,
    pub queue: String,
    pub payload_hash: [u8; 32],
    pub priority: Priority,
    pub enqueued_at: TimestampMillis,
    pub attempts: u32,
    pub max_attempts: u32,
    pub lease_epoch: u64,
    pub state: JobState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lease {
    pub job_id: JobId,
    pub worker_id: WorkerId,
    pub epoch: u64,
    pub expires_at: TimestampMillis,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    DuplicateJob,
    JobNotFound,
    InvalidLease,
    ExhaustedAttempts,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchedulerMetrics {
    pub queued_jobs: u64,
    pub leased_jobs: u64,
    pub completed_jobs: u64,
    pub failed_jobs: u64,
    pub claim_attempts: u64,
    pub claim_successes: u64,
    pub lease_conflicts: u64,
    pub lease_expirations: u64,
}

/// Deterministic lease-based scheduler core.
///
/// The type is storage-agnostic: production services can wrap each mutating
/// method in a compare-and-swap transaction against their backing store, while
/// tests and single-process services can use it directly.
#[derive(Default)]
pub struct LeaseScheduler {
    jobs: BTreeMap<JobId, Job>,
    metrics: SchedulerMetrics,
}

impl LeaseScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&mut self, job: Job) -> Result<(), SchedulerError> {
        if self.jobs.contains_key(&job.id) {
            return Err(SchedulerError::DuplicateJob);
        }
        if matches!(job.state, JobState::Queued) {
            self.metrics.queued_jobs += 1;
        }
        self.jobs.insert(job.id, job);
        Ok(())
    }

    pub fn claim_next(
        &mut self,
        queue: &str,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_ttl_ms: u64,
    ) -> Option<Lease> {
        self.metrics.claim_attempts += 1;
        self.requeue_expired(now);

        let job_id = self
            .jobs
            .values()
            .filter(|job| job.queue == queue && matches!(job.state, JobState::Queued))
            .max_by_key(|job| {
                (
                    job.priority,
                    core::cmp::Reverse(job.enqueued_at),
                    core::cmp::Reverse(job.id),
                )
            })
            .map(|job| job.id)?;

        let job = self.jobs.get_mut(&job_id)?;
        job.attempts += 1;
        if job.attempts > job.max_attempts {
            job.state = JobState::Failed {
                reason: String::from("attempt budget exhausted"),
            };
            self.metrics.queued_jobs = self.metrics.queued_jobs.saturating_sub(1);
            self.metrics.failed_jobs += 1;
            return None;
        }

        job.lease_epoch = job.lease_epoch.saturating_add(1);
        let epoch = job.lease_epoch;
        let expires_at = now.saturating_add(lease_ttl_ms);
        job.state = JobState::Leased {
            worker_id: worker_id.clone(),
            expires_at,
            epoch,
        };
        self.metrics.queued_jobs = self.metrics.queued_jobs.saturating_sub(1);
        self.metrics.leased_jobs += 1;
        self.metrics.claim_successes += 1;

        Some(Lease {
            job_id,
            worker_id,
            epoch,
            expires_at,
        })
    }

    pub fn renew(
        &mut self,
        lease: &Lease,
        now: TimestampMillis,
        lease_ttl_ms: u64,
    ) -> Result<Lease, SchedulerError> {
        let job = self
            .jobs
            .get_mut(&lease.job_id)
            .ok_or(SchedulerError::JobNotFound)?;
        match &mut job.state {
            JobState::Leased {
                worker_id,
                expires_at,
                epoch,
            } if *worker_id == lease.worker_id && *epoch == lease.epoch && *expires_at > now => {
                *expires_at = now.saturating_add(lease_ttl_ms);
                Ok(Lease {
                    job_id: lease.job_id,
                    worker_id: worker_id.clone(),
                    epoch: *epoch,
                    expires_at: *expires_at,
                })
            }
            _ => {
                self.metrics.lease_conflicts += 1;
                Err(SchedulerError::InvalidLease)
            }
        }
    }

    pub fn complete(&mut self, lease: &Lease, now: TimestampMillis) -> Result<(), SchedulerError> {
        let job = self
            .jobs
            .get_mut(&lease.job_id)
            .ok_or(SchedulerError::JobNotFound)?;
        match &job.state {
            JobState::Leased {
                worker_id,
                expires_at,
                epoch,
            } if worker_id == &lease.worker_id && *epoch == lease.epoch && *expires_at > now => {
                job.state = JobState::Completed;
                self.metrics.leased_jobs = self.metrics.leased_jobs.saturating_sub(1);
                self.metrics.completed_jobs += 1;
                Ok(())
            }
            _ => {
                self.metrics.lease_conflicts += 1;
                Err(SchedulerError::InvalidLease)
            }
        }
    }

    pub fn requeue_expired(&mut self, now: TimestampMillis) -> usize {
        let mut expired = 0;
        for job in self.jobs.values_mut() {
            if matches!(job.state, JobState::Leased { expires_at, .. } if expires_at <= now) {
                job.state = JobState::Queued;
                expired += 1;
            }
        }
        if expired > 0 {
            self.metrics.leased_jobs = self.metrics.leased_jobs.saturating_sub(expired as u64);
            self.metrics.queued_jobs += expired as u64;
            self.metrics.lease_expirations += expired as u64;
        }
        expired
    }

    pub fn job(&self, job_id: JobId) -> Option<&Job> {
        self.jobs.get(&job_id)
    }

    pub fn metrics(&self) -> &SchedulerMetrics {
        &self.metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: JobId, priority: Priority, enqueued_at: TimestampMillis) -> Job {
        Job {
            id,
            queue: String::from("critical"),
            payload_hash: [id as u8; 32],
            priority,
            enqueued_at,
            attempts: 0,
            max_attempts: 3,
            lease_epoch: 0,
            state: JobState::Queued,
        }
    }

    #[test]
    fn claims_highest_priority_oldest_job() {
        let mut scheduler = LeaseScheduler::new();
        scheduler.enqueue(job(1, 10, 20)).unwrap();
        scheduler.enqueue(job(2, 50, 30)).unwrap();
        scheduler.enqueue(job(3, 50, 10)).unwrap();

        let lease = scheduler
            .claim_next("critical", String::from("worker-a"), 100, 30)
            .unwrap();

        assert_eq!(lease.job_id, 3);
        assert_eq!(lease.expires_at, 130);
        assert_eq!(scheduler.metrics().claim_successes, 1);
    }

    #[test]
    fn rejects_stale_or_stolen_leases() {
        let mut scheduler = LeaseScheduler::new();
        scheduler.enqueue(job(7, 1, 0)).unwrap();
        let lease = scheduler
            .claim_next("critical", String::from("worker-a"), 10, 5)
            .unwrap();

        assert_eq!(
            scheduler.complete(&lease, 16),
            Err(SchedulerError::InvalidLease)
        );
        scheduler.requeue_expired(16);
        let replacement = scheduler
            .claim_next("critical", String::from("worker-b"), 16, 10)
            .unwrap();

        assert_eq!(
            scheduler.complete(&lease, 17),
            Err(SchedulerError::InvalidLease)
        );
        assert!(scheduler.complete(&replacement, 17).is_ok());
    }

    #[test]
    fn fencing_epoch_advances_when_same_worker_reclaims_expired_job() {
        let mut scheduler = LeaseScheduler::new();
        scheduler.enqueue(job(8, 1, 0)).unwrap();
        let first = scheduler
            .claim_next("critical", String::from("worker-a"), 10, 5)
            .unwrap();
        scheduler.requeue_expired(15);
        let second = scheduler
            .claim_next("critical", String::from("worker-a"), 15, 5)
            .unwrap();

        assert!(second.epoch > first.epoch);
        assert_eq!(
            scheduler.complete(&first, 16),
            Err(SchedulerError::InvalidLease)
        );
        assert!(scheduler.complete(&second, 16).is_ok());
    }

    #[test]
    fn renew_extends_only_active_owner_lease() {
        let mut scheduler = LeaseScheduler::new();
        scheduler.enqueue(job(9, 1, 0)).unwrap();
        let lease = scheduler
            .claim_next("critical", String::from("worker-a"), 100, 10)
            .unwrap();
        let renewed = scheduler.renew(&lease, 105, 50).unwrap();

        assert_eq!(renewed.expires_at, 155);
        assert_eq!(
            scheduler.renew(&lease, 156, 10),
            Err(SchedulerError::InvalidLease)
        );
    }

    #[test]
    fn exhausted_attempts_fail_closed() {
        let mut scheduler = LeaseScheduler::new();
        let mut limited = job(11, 1, 0);
        limited.max_attempts = 1;
        scheduler.enqueue(limited).unwrap();
        scheduler
            .claim_next("critical", String::from("worker-a"), 0, 1)
            .unwrap();
        scheduler.requeue_expired(1);

        assert!(scheduler
            .claim_next("critical", String::from("worker-b"), 2, 1)
            .is_none());
        assert!(matches!(
            scheduler.job(11).unwrap().state,
            JobState::Failed { .. }
        ));
    }
}
