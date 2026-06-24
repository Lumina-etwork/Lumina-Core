use std::time::Duration;
use super::super::proposal::equivocation_detector::EquivocationProof;

pub struct TimeoutLeader {
    pub current_view: u64,
    pub current_timeout: Duration,
    pub view_start_time: std::time::Instant,
}

impl TimeoutLeader {
    pub fn new() -> Self {
        Self {
            current_view: 0,
            current_timeout: Duration::from_secs(4),
            view_start_time: std::time::Instant::now(),
        }
    }

    // Timeout progression: linear from 4s, doubling each view (4s -> 8s -> 16s -> capped 120s)
    pub fn calculate_timeout(view: u64) -> Duration {
        let seconds = (4 * (1u64 << view.min(5))).min(120);
        Duration::from_secs(seconds)
    }

    pub fn advance_view(&mut self) {
        self.current_view += 1;
        self.current_timeout = Self::calculate_timeout(self.current_view);
        self.view_start_time = std::time::Instant::now();
    }

    pub fn handle_timeout_check(&mut self) -> bool {
        if self.view_start_time.elapsed() >= self.current_timeout {
            self.advance_view();
            true
        } else {
            false
        }
    }

    pub fn handle_equivocation_proof(&mut self, _proof: &EquivocationProof) {
        // immediately advance to the next view without waiting for timeout
        self.advance_view();
    }
}
