use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

pub struct ExpirationWatcher {
    pub cohorts_count: u32,
    pub renewal_window: Duration,
}

impl ExpirationWatcher {
    pub fn new() -> Self {
        Self {
            cohorts_count: 10,
            renewal_window: Duration::from_secs(144 * 60), // 2.4 hours = 144 mins
        }
    }

    pub fn get_cohort(&self, node_id: &str) -> u32 {
        let mut hasher = DefaultHasher::new();
        node_id.hash(&mut hasher);
        (hasher.finish() % self.cohorts_count as u64) as u32
    }

    pub fn get_cohort_sub_window(&self, node_id: &str) -> (Duration, Duration) {
        let cohort = self.get_cohort(node_id);
        let sub_window_duration = self.renewal_window.as_secs_f64() / self.cohorts_count as f64;
        let start = Duration::from_secs_f64(cohort as f64 * sub_window_duration);
        let end = start + Duration::from_secs_f64(sub_window_duration);
        (start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::credential_renewer::CredentialRenewer;
    use crate::attestation::credential_signer::{CredentialSigner, SignerError};
    use std::time::Instant;

    #[test]
    fn load_test_thundering_herd_mitigation() {
        // Simulate 1,000 nodes with synchronized clocks
        let num_nodes = 1000;
        let mut nodes = Vec::new();
        for i in 0..num_nodes {
            nodes.push(format!("node_{}", i));
        }

        let watcher = ExpirationWatcher::new();
        let mut renewer = CredentialRenewer::new();
        let mut signer = CredentialSigner::new();
        
        let start_time = Instant::now();
        let window_duration_secs = watcher.renewal_window.as_secs_f64();
        
        // In a real simulation, we would advance time and trigger events. 
        // Here we just test that spreading works by assigning them delays and counting per second.
        let mut requests_by_second: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();

        for node in &nodes {
            let (sub_window_start, _) = watcher.get_cohort_sub_window(node);
            
            // base rand: 0 - 5 mins (300 secs)
            // for determinism in test, we hash the node id for pseudo-rand
            let mut hasher = DefaultHasher::new();
            node.hash(&mut hasher);
            let pseudo_rand = (hasher.finish() % 301) as f64; 
            
            // Assume current time is exactly their sub-window start for max overlap simulation
            let delay = renewer.calculate_jitter(
                pseudo_rand, 
                sub_window_start.as_secs_f64(), 
                sub_window_start.as_secs_f64(), 
                window_duration_secs
            );
            
            let request_time_secs = sub_window_start.as_secs() + delay.as_secs();
            *requests_by_second.entry(request_time_secs).or_insert(0) += 1;
        }

        let mut max_rate = 0;
        for (_, count) in requests_by_second {
            if count > max_rate {
                max_rate = count;
            }
        }
        
        // Assert peak signing rate < 60/sec
        assert!(max_rate < 60, "Peak signing rate exceeded 60 sigs/sec! Max was {}", max_rate);
    }
}
