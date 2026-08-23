use std::time::Duration;

pub struct CredentialRenewer {
    pub metrics: RenewerMetrics,
}

#[derive(Default)]
pub struct RenewerMetrics {
    pub renewal_queue_depth: u64,
}

impl CredentialRenewer {
    pub fn new() -> Self {
        Self {
            metrics: RenewerMetrics::default(),
        }
    }

    pub fn calculate_jitter(
        &self,
        base_rand_secs: f64,
        current_time_secs: f64,
        window_start_secs: f64,
        window_duration_secs: f64,
    ) -> Duration {
        let mut expiry_proximity = 0.0;
        if window_duration_secs > 0.0 {
            expiry_proximity = (current_time_secs - window_start_secs) / window_duration_secs;
            if expiry_proximity < 0.0 {
                expiry_proximity = 0.0;
            }
        }
        let delay_secs = base_rand_secs + (expiry_proximity * 60.0);
        Duration::from_secs_f64(delay_secs)
    }

    pub fn enqueue_renewal(&mut self) {
        self.metrics.renewal_queue_depth += 1;
    }

    pub fn dequeue_renewal(&mut self) {
        if self.metrics.renewal_queue_depth > 0 {
            self.metrics.renewal_queue_depth -= 1;
        }
    }
}
