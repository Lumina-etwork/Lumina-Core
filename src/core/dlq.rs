use alloc::vec::Vec;
use alloc::string::String;
use alloc::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub struct FailedMessage {
    pub message_id: String,
    pub payload: Vec<u8>,
    pub error_reason: String,
    pub timestamp: u64,
    pub retry_count: u8,
}

pub struct DeadLetterQueue {
    pub queue: VecDeque<FailedMessage>,
    pub max_size: usize,
    pub metrics: DlqMetrics,
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct DlqMetrics {
    pub total_enqueued: u64,
    pub total_processed: u64,
    pub total_dropped: u64,
}

impl DeadLetterQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            max_size,
            metrics: DlqMetrics::default(),
        }
    }

    pub fn enqueue(&mut self, message: FailedMessage) -> Result<(), &'static str> {
        if self.queue.len() >= self.max_size {
            self.metrics.total_dropped += 1;
            return Err("Queue is full");
        }
        self.queue.push_back(message);
        self.metrics.total_enqueued += 1;
        Ok(())
    }

    pub fn dequeue(&mut self) -> Option<FailedMessage> {
        if let Some(message) = self.queue.pop_front() {
            self.metrics.total_processed += 1;
            Some(message)
        } else {
            None
        }
    }

    pub fn get_metrics(&self) -> DlqMetrics {
        self.metrics.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    #[test]
    fn test_enqueue_dequeue() {
        let mut dlq = DeadLetterQueue::new(10);
        let msg = FailedMessage {
            message_id: "1".to_string(),
            payload: vec![1, 2, 3],
            error_reason: "timeout".to_string(),
            timestamp: 123456789,
            retry_count: 0,
        };
        assert!(dlq.enqueue(msg.clone()).is_ok());
        let dequeued = dlq.dequeue();
        assert_eq!(dequeued, Some(msg));
        assert_eq!(dlq.metrics.total_enqueued, 1);
        assert_eq!(dlq.metrics.total_processed, 1);
    }

    #[test]
    fn test_queue_full() {
        let mut dlq = DeadLetterQueue::new(1);
        let msg = FailedMessage {
            message_id: "1".to_string(),
            payload: vec![1, 2, 3],
            error_reason: "timeout".to_string(),
            timestamp: 123456789,
            retry_count: 0,
        };
        assert!(dlq.enqueue(msg.clone()).is_ok());
        assert!(dlq.enqueue(msg.clone()).is_err());
        assert_eq!(dlq.metrics.total_dropped, 1);
    }
}
