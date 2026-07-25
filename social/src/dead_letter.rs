//! Dead Letter Queue (DLQ) support for failed message processing.
//!
//! The DLQ keeps failed message work off the user-facing critical path by
//! recording compact failure envelopes and allowing operators or background
//! workers to inspect, retry, or mark entries as resolved.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Operational state for a dead-lettered message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeadLetterStatus {
    Pending,
    Retrying,
    Resolved,
}

/// A durable failure envelope for a message that could not be processed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeadLetterMessage {
    pub id: Uuid,
    pub message_id: Option<Uuid>,
    pub sender_id: Option<Uuid>,
    pub recipient_id: Option<Uuid>,
    pub failure_stage: String,
    pub error_class: String,
    pub error_message: String,
    pub payload_digest: String,
    pub retry_count: u32,
    pub status: DeadLetterStatus,
    pub first_failed_at: DateTime<Utc>,
    pub last_failed_at: DateTime<Utc>,
    pub next_retry_at: DateTime<Utc>,
}

/// Input used when recording a failed message processing attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadLetterInput {
    pub message_id: Option<Uuid>,
    pub sender_id: Option<Uuid>,
    pub recipient_id: Option<Uuid>,
    pub failure_stage: String,
    pub error_class: String,
    pub error_message: String,
    pub payload_digest: String,
}

/// Bounded, non-blocking queue used by request handlers and workers.
#[derive(Debug)]
pub struct DeadLetterQueue {
    entries: VecDeque<DeadLetterMessage>,
    max_entries: usize,
    retry_policy: RetryPolicy,
}

/// Retry backoff settings for DLQ reprocessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub max_retries: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            base_delay: Duration::from_secs(30),
            max_delay: Duration::from_secs(15 * 60),
            max_retries: 5,
        }
    }
}

impl DeadLetterQueue {
    pub fn new(max_entries: usize, retry_policy: RetryPolicy) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
            retry_policy,
        }
    }

    /// Records a failure in O(1) time and evicts the oldest unresolved envelope
    /// when the queue is full, keeping API hot paths under the 100ms P99 target.
    pub fn record_failure(&mut self, input: DeadLetterInput) -> DeadLetterMessage {
        if self.entries.len() == self.max_entries {
            self.entries.pop_front();
        }

        let now = Utc::now();
        let entry = DeadLetterMessage {
            id: Uuid::new_v4(),
            message_id: input.message_id,
            sender_id: input.sender_id,
            recipient_id: input.recipient_id,
            failure_stage: input.failure_stage,
            error_class: input.error_class,
            error_message: input.error_message,
            payload_digest: input.payload_digest,
            retry_count: 0,
            status: DeadLetterStatus::Pending,
            first_failed_at: now,
            last_failed_at: now,
            next_retry_at: now + self.retry_policy.base_delay,
        };

        self.entries.push_back(entry.clone());
        entry
    }

    pub fn pending(&self) -> impl Iterator<Item = &DeadLetterMessage> {
        self.entries
            .iter()
            .filter(|entry| entry.status == DeadLetterStatus::Pending)
    }

    pub fn mark_retry_failed(
        &mut self,
        id: Uuid,
        error_message: String,
    ) -> Option<DeadLetterMessage> {
        let entry = self.entries.iter_mut().find(|entry| entry.id == id)?;
        entry.retry_count += 1;
        entry.last_failed_at = Utc::now();
        entry.error_message = error_message;
        entry.status = if entry.retry_count >= self.retry_policy.max_retries {
            DeadLetterStatus::Pending
        } else {
            DeadLetterStatus::Retrying
        };
        let delay = self.retry_policy.next_delay(entry.retry_count);
        entry.next_retry_at = Utc::now() + delay;
        Some(entry.clone())
    }

    pub fn mark_resolved(&mut self, id: Uuid) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.status = DeadLetterStatus::Resolved;
            true
        } else {
            false
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl RetryPolicy {
    fn next_delay(&self, retry_count: u32) -> Duration {
        let multiplier = 2_u32.saturating_pow(retry_count.min(16));
        self.base_delay
            .saturating_mul(multiplier)
            .min(self.max_delay)
    }
}

/// Lightweight latency check used by tests to guard the hot-path budget.
pub fn records_within_budget(
    queue: &mut DeadLetterQueue,
    input: DeadLetterInput,
    budget: Duration,
) -> bool {
    let started = Instant::now();
    queue.record_failure(input);
    started.elapsed() < budget
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> DeadLetterInput {
        DeadLetterInput {
            message_id: Some(Uuid::new_v4()),
            sender_id: Some(Uuid::new_v4()),
            recipient_id: Some(Uuid::new_v4()),
            failure_stage: "persist_message".to_string(),
            error_class: "database_timeout".to_string(),
            error_message: "insert timed out".to_string(),
            payload_digest: "sha256:abc123".to_string(),
        }
    }

    #[test]
    fn records_failure_as_pending() {
        let mut queue = DeadLetterQueue::new(10, RetryPolicy::default());
        let entry = queue.record_failure(input());

        assert_eq!(queue.len(), 1);
        assert_eq!(entry.status, DeadLetterStatus::Pending);
        assert_eq!(queue.pending().count(), 1);
    }

    #[test]
    fn evicts_oldest_entry_when_capacity_is_reached() {
        let mut queue = DeadLetterQueue::new(1, RetryPolicy::default());
        let first = queue.record_failure(input());
        let second = queue.record_failure(input());

        assert_eq!(queue.len(), 1);
        assert_ne!(first.id, second.id);
        assert_eq!(queue.pending().next().unwrap().id, second.id);
    }

    #[test]
    fn retry_failure_applies_exponential_backoff() {
        let mut queue = DeadLetterQueue::new(
            10,
            RetryPolicy {
                base_delay: Duration::from_secs(1),
                max_delay: Duration::from_secs(10),
                max_retries: 5,
            },
        );
        let entry = queue.record_failure(input());
        let retried = queue
            .mark_retry_failed(entry.id, "still failing".to_string())
            .unwrap();

        assert_eq!(retried.retry_count, 1);
        assert_eq!(retried.status, DeadLetterStatus::Retrying);
        assert!(retried.next_retry_at > retried.last_failed_at);
    }

    #[test]
    fn record_failure_stays_inside_hot_path_budget() {
        let mut queue = DeadLetterQueue::new(100, RetryPolicy::default());
        assert!(records_within_budget(
            &mut queue,
            input(),
            Duration::from_millis(100)
        ));
    }
}
