//! Service-level objective helpers for availability and latency burn-rate alerts.
//!
//! The module is dependency-free so latency-critical services can evaluate SLO
//! health on hot paths and export the same values to metrics collectors.

/// The system-wide availability objective expressed as a ratio.
pub const SYSTEM_AVAILABILITY_OBJECTIVE: f64 = 0.9999;

/// The critical-path P99 latency objective in milliseconds.
pub const CRITICAL_PATH_P99_LATENCY_MS: u64 = 100;

/// Alert severity for an SLO window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlertSeverity {
    /// No page or ticket is required.
    Healthy,
    /// A ticket should be opened for investigation.
    Ticket,
    /// The on-call engineer should be paged.
    Page,
}

/// Error-budget status for a single measurement window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BurnRateWindow {
    /// Number of requests or operations observed in the window.
    pub total_events: u64,
    /// Number of failed or over-objective events in the window.
    pub bad_events: u64,
    /// SLO objective as a ratio, for example `0.9999` for 99.99%.
    pub objective: f64,
}

impl BurnRateWindow {
    /// Construct a new window. Objectives outside `(0.0, 1.0)` are clamped to
    /// the system availability objective to keep callers fail-safe.
    pub fn new(total_events: u64, bad_events: u64, objective: f64) -> Self {
        let objective = if objective > 0.0 && objective < 1.0 {
            objective
        } else {
            SYSTEM_AVAILABILITY_OBJECTIVE
        };

        Self {
            total_events,
            bad_events: bad_events.min(total_events),
            objective,
        }
    }

    /// Ratio of events that failed the objective in this window.
    pub fn error_ratio(&self) -> f64 {
        if self.total_events == 0 {
            0.0
        } else {
            self.bad_events as f64 / self.total_events as f64
        }
    }

    /// Fraction of the allowed error budget consumed per unit of time.
    pub fn burn_rate(&self) -> f64 {
        let budget = 1.0 - self.objective;
        if budget <= 0.0 {
            0.0
        } else {
            self.error_ratio() / budget
        }
    }

    /// Classify the burn rate using standard multi-window thresholds.
    pub fn severity(&self) -> AlertSeverity {
        let burn_rate = self.burn_rate();
        if burn_rate >= 14.4 {
            AlertSeverity::Page
        } else if burn_rate >= 6.0 {
            AlertSeverity::Ticket
        } else {
            AlertSeverity::Healthy
        }
    }
}

/// Convert a latency histogram snapshot into a bad-event window by counting
/// requests above the critical-path P99 objective as budget-consuming events.
pub fn latency_window(total_requests: u64, requests_over_100ms: u64) -> BurnRateWindow {
    BurnRateWindow::new(
        total_requests,
        requests_over_100ms,
        SYSTEM_AVAILABILITY_OBJECTIVE,
    )
}

/// Returns true when a deployment can continue during canary analysis.
pub fn canary_within_slo(error_window: BurnRateWindow, latency_p99_ms: u64) -> bool {
    error_window.severity() != AlertSeverity::Page && latency_p99_ms <= CRITICAL_PATH_P99_LATENCY_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burn_rate_uses_error_budget() {
        let window = BurnRateWindow::new(1_000_000, 100, SYSTEM_AVAILABILITY_OBJECTIVE);
        assert!((window.error_ratio() - 0.0001).abs() < f64::EPSILON);
        assert!((window.burn_rate() - 1.0).abs() < 0.0000001);
        assert_eq!(window.severity(), AlertSeverity::Healthy);
    }

    #[test]
    fn severity_pages_on_fast_budget_burn() {
        let page = BurnRateWindow::new(1_000_000, 1_500, SYSTEM_AVAILABILITY_OBJECTIVE);
        let ticket = BurnRateWindow::new(1_000_000, 700, SYSTEM_AVAILABILITY_OBJECTIVE);

        assert_eq!(page.severity(), AlertSeverity::Page);
        assert_eq!(ticket.severity(), AlertSeverity::Ticket);
    }

    #[test]
    fn canary_blocks_on_latency_or_page() {
        let healthy = BurnRateWindow::new(1_000_000, 10, SYSTEM_AVAILABILITY_OBJECTIVE);
        let paging = BurnRateWindow::new(1_000_000, 1_500, SYSTEM_AVAILABILITY_OBJECTIVE);

        assert!(canary_within_slo(healthy, CRITICAL_PATH_P99_LATENCY_MS));
        assert!(!canary_within_slo(healthy, CRITICAL_PATH_P99_LATENCY_MS + 1));
        assert!(!canary_within_slo(paging, 50));
    }

    #[test]
    fn latency_window_clamps_bad_events() {
        let window = latency_window(10, 11);
        assert_eq!(window.bad_events, 10);
        assert_eq!(window.error_ratio(), 1.0);
    }
}
