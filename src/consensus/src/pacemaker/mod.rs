/// View-change timeout orchestration.
///
/// Implements adaptive timeouts based on round-trip time measurements.
/// Bounds: 2s–30s per view-change round, with 4× RTT as the target.
use std::time::Duration;

/// Configuration for the pacemaker timeout controller.
#[derive(Clone, Debug)]
pub struct TimeoutConfig {
    /// Minimum timeout (2s).
    pub min_timeout: Duration,
    /// Maximum timeout (30s).
    pub max_timeout: Duration,
    /// Multiplier for RTT (4×).
    pub rtt_multiplier: f64,
    /// Number of rounds before exponential back-off kicks in.
    pub backoff_rounds: u64,
    /// Back-off multiplier applied after backoff_rounds.
    pub backoff_factor: f64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            min_timeout: Duration::from_secs(2),
            max_timeout: Duration::from_secs(30),
            rtt_multiplier: 4.0,
            backoff_rounds: 3,
            backoff_factor: 1.5,
        }
    }
}

/// State of the view-change timeout controller.
#[derive(Clone, Debug)]
pub struct TimeoutController {
    config: TimeoutConfig,
    /// Smoothed RTT estimate (nanos).
    smoothed_rtt_ns: f64,
    /// Current view number.
    current_view: u64,
    /// Number of consecutive timeouts in the current view.
    timeout_count: u64,
    /// Deadline for the current view-change round.
    deadline: Option<Duration>,
}

impl TimeoutController {
    pub fn new(config: TimeoutConfig) -> Self {
        Self {
            smoothed_rtt_ns: Duration::from_millis(500).as_nanos() as f64,
            current_view: 0,
            timeout_count: 0,
            deadline: None,
            config,
        }
    }

    /// Update smoothed RTT with a new measurement.
    pub fn observe_rtt(&mut self, rtt: Duration) {
        let rtt_ns = rtt.as_nanos() as f64;
        // EWMA with alpha = 0.125
        self.smoothed_rtt_ns = 0.875 * self.smoothed_rtt_ns + 0.125 * rtt_ns;
    }

    /// Compute the timeout for the current view-change round.
    pub fn compute_timeout(&self) -> Duration {
        let mut timeout_ns = self.smoothed_rtt_ns * self.config.rtt_multiplier;

        // Apply exponential back-off after consecutive timeouts
        if self.timeout_count > self.config.backoff_rounds {
            let extra = self.timeout_count - self.config.backoff_rounds;
            timeout_ns *= self.config.backoff_factor.powi(extra as i32);
        }

        // Clamp to [min, max]
        let timeout = Duration::from_nanos(timeout_ns as u64);
        timeout.clamp(self.config.min_timeout, self.config.max_timeout)
    }

    /// Start a view-change round: compute deadline and advance view.
    pub fn start_view_change(&mut self) -> Duration {
        let timeout = self.compute_timeout();
        self.timeout_count += 1;
        self.deadline = Some(timeout);
        timeout
    }

    /// Called when a view-change round completes (QC received).
    pub fn on_view_change_complete(&mut self, new_view: u64) {
        self.current_view = new_view;
        self.timeout_count = 0;
        self.deadline = None;
    }

    /// Check if the current round has timed out.
    pub fn is_timed_out(&self, elapsed: Duration) -> bool {
        self.deadline
            .map(|d| elapsed >= d)
            .unwrap_or(false)
    }

    pub fn current_view(&self) -> u64 {
        self.current_view
    }

    pub fn timeout_count(&self) -> u64 {
        self.timeout_count
    }
}