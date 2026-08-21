use std::collections::HashMap;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Baseline probe interval used when a connection is healthy.
pub const PROBE_INTERVAL_MIN: Duration = Duration::from_millis(500);

/// Maximum probe interval reached after repeated failures (exponential backoff).
pub const PROBE_INTERVAL_MAX: Duration = Duration::from_secs(30);

/// Number of consecutive probe failures before a connection is evicted.
pub const MAX_CONSECUTIVE_FAILURES: u8 = 5;

/// P99 latency budget for health-probe critical paths (100 ms).
pub const PROBE_LATENCY_BUDGET_MS: u64 = 100;

/// Minimum pool size — the pool will never shrink below this.
pub const POOL_SIZE_MIN: usize = 1;

/// Maximum pool size — the pool will never grow beyond this.
pub const POOL_SIZE_MAX: usize = 128;

/// Pool grows by this many connections per successful scale-out decision.
pub const POOL_SCALE_STEP: usize = 4;

/// Fraction of connections that must be in-use before the pool scales out.
/// Represented as a percentage (0–100).
pub const SCALE_OUT_UTILISATION_PCT: u8 = 75;

/// Fraction of connections in-use below which the pool scales in.
pub const SCALE_IN_UTILISATION_PCT: u8 = 25;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Unique identifier for a pooled connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(pub u64);

/// Observable health of a single pooled connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionHealth {
    /// Probe succeeded within the latency budget.
    Healthy,
    /// Probe succeeded but latency exceeded [`PROBE_LATENCY_BUDGET_MS`].
    Degraded,
    /// Probe failed (no response / error returned).
    Failed,
}

/// Events emitted by the health-probe subsystem, consumed by monitors and
/// the adaptive-sizing controller.
#[derive(Debug, Clone, PartialEq)]
pub enum ProbeEvent {
    /// A connection passed its health probe.
    ProbeSucceeded {
        conn_id: ConnectionId,
        latency_ms: u64,
    },
    /// A connection probe returned an error or exceeded the deadline.
    ProbeFailed {
        conn_id: ConnectionId,
        consecutive_failures: u8,
    },
    /// A connection was evicted after exceeding [`MAX_CONSECUTIVE_FAILURES`].
    ConnectionEvicted {
        conn_id: ConnectionId,
    },
    /// Pool size was increased by the adaptive controller.
    PoolScaledOut {
        previous_size: usize,
        new_size: usize,
    },
    /// Pool size was decreased by the adaptive controller.
    PoolScaledIn {
        previous_size: usize,
        new_size: usize,
    },
}

/// Per-connection state tracked by the health probe.
#[derive(Debug, Clone)]
pub struct ConnectionProbeState {
    /// Current health assessment.
    pub health: ConnectionHealth,
    /// Number of consecutive probe failures without an intervening success.
    pub consecutive_failures: u8,
    /// Exponential-backoff interval for the next probe.
    pub probe_interval: Duration,
    /// Timestamp of the last probe attempt.
    pub last_probe_at: Option<Instant>,
    /// Whether this connection is currently checked out by a caller.
    pub in_use: bool,
}

impl ConnectionProbeState {
    /// Creates state for a freshly established connection.
    pub fn new() -> Self {
        Self {
            health: ConnectionHealth::Healthy,
            consecutive_failures: 0,
            probe_interval: PROBE_INTERVAL_MIN,
            last_probe_at: None,
            in_use: false,
        }
    }

    /// Returns `true` if enough time has elapsed since the last probe.
    pub fn is_due(&self, now: Instant) -> bool {
        match self.last_probe_at {
            None => true,
            Some(t) => now.saturating_duration_since(t) >= self.probe_interval,
        }
    }

    /// Records a successful probe result.
    ///
    /// Resets the failure counter and backoff interval.  Returns
    /// [`ConnectionHealth::Degraded`] when `latency_ms` exceeds the budget.
    pub fn record_success(&mut self, latency_ms: u64, now: Instant) {
        self.consecutive_failures = 0;
        self.probe_interval = PROBE_INTERVAL_MIN;
        self.last_probe_at = Some(now);
        self.health = if latency_ms > PROBE_LATENCY_BUDGET_MS {
            ConnectionHealth::Degraded
        } else {
            ConnectionHealth::Healthy
        };
    }

    /// Records a failed probe result.
    ///
    /// Advances the exponential-backoff interval (doubles up to
    /// [`PROBE_INTERVAL_MAX`]).
    pub fn record_failure(&mut self, now: Instant) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.probe_interval = (self.probe_interval * 2).min(PROBE_INTERVAL_MAX);
        self.last_probe_at = Some(now);
        self.health = ConnectionHealth::Failed;
    }
}

impl Default for ConnectionProbeState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HealthProbeManager
// ---------------------------------------------------------------------------

/// Manages health probes and adaptive sizing for a PostgreSQL connection pool.
///
/// # Responsibilities
///
/// * Tracks per-connection probe state (success/failure/backoff).
/// * Evicts connections that exceed [`MAX_CONSECUTIVE_FAILURES`].
/// * Emits [`ProbeEvent`]s consumed by the caller (metrics, alerting).
/// * Adjusts the *target* pool size based on utilisation heuristics:
///   - Scale out when ≥ [`SCALE_OUT_UTILISATION_PCT`]% of connections are in use.
///   - Scale in  when ≤ [`SCALE_IN_UTILISATION_PCT`]% are in use.
pub struct HealthProbeManager {
    connections: HashMap<ConnectionId, ConnectionProbeState>,
    /// Current target pool size (number of connections the pool should hold).
    target_size: usize,
    /// Monotonically increasing ID counter for new connections.
    next_id: u64,
    /// Cumulative count of connections evicted due to probe failures.
    pub eviction_count: u64,
}

impl HealthProbeManager {
    /// Creates a manager with `initial_size` healthy connections.
    pub fn new(initial_size: usize) -> Self {
        let clamped = initial_size.clamp(POOL_SIZE_MIN, POOL_SIZE_MAX);
        let mut mgr = Self {
            connections: HashMap::new(),
            target_size: clamped,
            next_id: 0,
            eviction_count: 0,
        };
        for _ in 0..clamped {
            mgr.add_connection();
        }
        mgr
    }

    // -----------------------------------------------------------------------
    // Connection lifecycle
    // -----------------------------------------------------------------------

    /// Registers a new connection and returns its [`ConnectionId`].
    pub fn add_connection(&mut self) -> ConnectionId {
        let id = ConnectionId(self.next_id);
        self.next_id += 1;
        self.connections.insert(id, ConnectionProbeState::new());
        id
    }

    /// Removes a connection unconditionally (e.g. graceful teardown).
    pub fn remove_connection(&mut self, id: ConnectionId) {
        self.connections.remove(&id);
    }

    /// Marks a connection as checked out.
    pub fn checkout(&mut self, id: ConnectionId) {
        if let Some(s) = self.connections.get_mut(&id) {
            s.in_use = true;
        }
    }

    /// Marks a connection as returned to the pool.
    pub fn checkin(&mut self, id: ConnectionId) {
        if let Some(s) = self.connections.get_mut(&id) {
            s.in_use = false;
        }
    }

    // -----------------------------------------------------------------------
    // Probing
    // -----------------------------------------------------------------------

    /// Processes a probe result for `conn_id`.
    ///
    /// * `latency_ms` — round-trip time of the probe in milliseconds.
    /// * `succeeded`  — whether the probe call itself returned without error.
    ///
    /// Returns the list of [`ProbeEvent`]s produced (0–2 events per call).
    pub fn record_probe(
        &mut self,
        conn_id: ConnectionId,
        latency_ms: u64,
        succeeded: bool,
        now: Instant,
    ) -> Vec<ProbeEvent> {
        let mut events = Vec::new();

        let state = match self.connections.get_mut(&conn_id) {
            Some(s) => s,
            None => return events,
        };

        if succeeded {
            state.record_success(latency_ms, now);
            events.push(ProbeEvent::ProbeSucceeded {
                conn_id,
                latency_ms,
            });
        } else {
            state.record_failure(now);
            let consecutive = state.consecutive_failures;
            events.push(ProbeEvent::ProbeFailed {
                conn_id,
                consecutive_failures: consecutive,
            });

            if consecutive >= MAX_CONSECUTIVE_FAILURES {
                self.connections.remove(&conn_id);
                self.eviction_count += 1;
                events.push(ProbeEvent::ConnectionEvicted { conn_id });
            }
        }

        events
    }

    /// Returns the IDs of connections whose probes are currently due.
    pub fn due_connections(&self, now: Instant) -> Vec<ConnectionId> {
        self.connections
            .iter()
            .filter(|(_, s)| s.is_due(now))
            .map(|(id, _)| *id)
            .collect()
    }

    // -----------------------------------------------------------------------
    // Adaptive sizing
    // -----------------------------------------------------------------------

    /// Evaluates current utilisation and adjusts the target pool size.
    ///
    /// Returns any [`ProbeEvent::PoolScaledOut`] or [`ProbeEvent::PoolScaledIn`]
    /// events produced.  The caller is responsible for actually creating or
    /// destroying connections based on the new [`target_size`](Self::target_size).
    pub fn evaluate_pool_size(&mut self) -> Vec<ProbeEvent> {
        let total = self.connections.len();
        if total == 0 {
            return Vec::new();
        }

        let in_use = self.connections.values().filter(|s| s.in_use).count();
        let utilisation_pct = (in_use * 100) / total;

        let mut events = Vec::new();

        if utilisation_pct >= SCALE_OUT_UTILISATION_PCT as usize
            && self.target_size < POOL_SIZE_MAX
        {
            let previous_size = self.target_size;
            self.target_size = (self.target_size + POOL_SCALE_STEP).min(POOL_SIZE_MAX);
            events.push(ProbeEvent::PoolScaledOut {
                previous_size,
                new_size: self.target_size,
            });
        } else if utilisation_pct <= SCALE_IN_UTILISATION_PCT as usize
            && self.target_size > POOL_SIZE_MIN
        {
            let previous_size = self.target_size;
            self.target_size = (self.target_size.saturating_sub(POOL_SCALE_STEP)).max(POOL_SIZE_MIN);
            events.push(ProbeEvent::PoolScaledIn {
                previous_size,
                new_size: self.target_size,
            });
        }

        events
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Returns the current target pool size.
    pub fn target_size(&self) -> usize {
        self.target_size
    }

    /// Returns the number of connections currently tracked.
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Returns the health state for a specific connection, if known.
    pub fn connection_state(&self, id: ConnectionId) -> Option<&ConnectionProbeState> {
        self.connections.get(&id)
    }

    /// Returns the utilisation ratio as a percentage (0–100).
    pub fn utilisation_pct(&self) -> usize {
        let total = self.connections.len();
        if total == 0 {
            return 0;
        }
        let in_use = self.connections.values().filter(|s| s.in_use).count();
        (in_use * 100) / total
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn now() -> Instant {
        Instant::now()
    }

    // --- ConnectionProbeState ---

    #[test]
    fn new_connection_is_healthy_and_probe_is_due() {
        let state = ConnectionProbeState::new();
        assert_eq!(state.health, ConnectionHealth::Healthy);
        assert_eq!(state.consecutive_failures, 0);
        assert!(state.is_due(now()));
    }

    #[test]
    fn probe_success_within_budget_marks_healthy() {
        let mut state = ConnectionProbeState::new();
        let t = now();
        state.record_success(50, t);
        assert_eq!(state.health, ConnectionHealth::Healthy);
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.probe_interval, PROBE_INTERVAL_MIN);
    }

    #[test]
    fn probe_success_over_budget_marks_degraded() {
        let mut state = ConnectionProbeState::new();
        let t = now();
        state.record_success(PROBE_LATENCY_BUDGET_MS + 1, t);
        assert_eq!(state.health, ConnectionHealth::Degraded);
    }

    #[test]
    fn probe_failure_increments_counter_and_backs_off() {
        let mut state = ConnectionProbeState::new();
        let t = now();

        state.record_failure(t);
        assert_eq!(state.consecutive_failures, 1);
        assert_eq!(state.probe_interval, PROBE_INTERVAL_MIN * 2);

        state.record_failure(t);
        assert_eq!(state.consecutive_failures, 2);
        assert_eq!(state.probe_interval, PROBE_INTERVAL_MIN * 4);
    }

    #[test]
    fn backoff_caps_at_max_interval() {
        let mut state = ConnectionProbeState::new();
        let t = now();
        // 500ms → 1s → 2s → 4s → 8s → 16s → 30s (capped)
        for _ in 0..10 {
            state.record_failure(t);
        }
        assert_eq!(state.probe_interval, PROBE_INTERVAL_MAX);
    }

    #[test]
    fn success_after_failures_resets_state() {
        let mut state = ConnectionProbeState::new();
        let t = now();
        state.record_failure(t);
        state.record_failure(t);
        state.record_success(10, t);
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.probe_interval, PROBE_INTERVAL_MIN);
        assert_eq!(state.health, ConnectionHealth::Healthy);
    }

    #[test]
    fn probe_not_due_before_interval_elapses() {
        let mut state = ConnectionProbeState::new();
        let t = now();
        state.record_success(10, t);
        // Probe just happened — should not be due until interval passes.
        assert!(!state.is_due(t));
        // After the full interval it becomes due again.
        assert!(state.is_due(t + PROBE_INTERVAL_MIN));
    }

    // --- HealthProbeManager ---

    #[test]
    fn new_manager_tracks_correct_connection_count() {
        let mgr = HealthProbeManager::new(8);
        assert_eq!(mgr.connection_count(), 8);
        assert_eq!(mgr.target_size(), 8);
    }

    #[test]
    fn initial_size_clamped_to_bounds() {
        let small = HealthProbeManager::new(0);
        assert_eq!(small.connection_count(), POOL_SIZE_MIN);

        let large = HealthProbeManager::new(999);
        assert_eq!(large.connection_count(), POOL_SIZE_MAX);
    }

    #[test]
    fn record_probe_success_emits_event() {
        let mut mgr = HealthProbeManager::new(1);
        let id = *mgr.connections.keys().next().unwrap();
        let t = now();
        let events = mgr.record_probe(id, 30, true, t);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ProbeEvent::ProbeSucceeded { latency_ms: 30, .. }));
    }

    #[test]
    fn record_probe_failure_emits_event() {
        let mut mgr = HealthProbeManager::new(1);
        let id = *mgr.connections.keys().next().unwrap();
        let t = now();
        let events = mgr.record_probe(id, 0, false, t);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            ProbeEvent::ProbeFailed { consecutive_failures: 1, .. }
        ));
    }

    #[test]
    fn connection_evicted_after_max_failures() {
        let mut mgr = HealthProbeManager::new(1);
        let id = *mgr.connections.keys().next().unwrap();
        let t = now();

        let mut evicted = false;
        for _ in 0..=MAX_CONSECUTIVE_FAILURES {
            let events = mgr.record_probe(id, 0, false, t);
            for e in &events {
                if matches!(e, ProbeEvent::ConnectionEvicted { .. }) {
                    evicted = true;
                }
            }
        }

        assert!(evicted, "connection must be evicted after max consecutive failures");
        assert_eq!(mgr.eviction_count, 1);
        assert_eq!(mgr.connection_count(), 0);
    }

    #[test]
    fn eviction_removes_connection_from_pool() {
        let mut mgr = HealthProbeManager::new(2);
        let ids: Vec<_> = mgr.connections.keys().copied().collect();
        let victim = ids[0];
        let t = now();

        for _ in 0..MAX_CONSECUTIVE_FAILURES {
            mgr.record_probe(victim, 0, false, t);
        }

        assert!(mgr.connection_state(victim).is_none());
        assert_eq!(mgr.connection_count(), 1);
    }

    #[test]
    fn unknown_connection_probe_returns_no_events() {
        let mut mgr = HealthProbeManager::new(1);
        let phantom = ConnectionId(9999);
        let events = mgr.record_probe(phantom, 10, true, now());
        assert!(events.is_empty());
    }

    #[test]
    fn checkout_and_checkin_update_in_use_flag() {
        let mut mgr = HealthProbeManager::new(1);
        let id = *mgr.connections.keys().next().unwrap();

        mgr.checkout(id);
        assert!(mgr.connection_state(id).unwrap().in_use);

        mgr.checkin(id);
        assert!(!mgr.connection_state(id).unwrap().in_use);
    }

    #[test]
    fn utilisation_pct_calculates_correctly() {
        let mut mgr = HealthProbeManager::new(4);
        let ids: Vec<_> = mgr.connections.keys().copied().collect();

        mgr.checkout(ids[0]);
        mgr.checkout(ids[1]);
        // 2 of 4 = 50%
        assert_eq!(mgr.utilisation_pct(), 50);
    }

    // --- Adaptive sizing ---

    #[test]
    fn scale_out_triggers_at_high_utilisation() {
        let mut mgr = HealthProbeManager::new(4);
        let ids: Vec<_> = mgr.connections.keys().copied().collect();

        // Check out 3 of 4 = 75% — exactly at the threshold.
        mgr.checkout(ids[0]);
        mgr.checkout(ids[1]);
        mgr.checkout(ids[2]);

        let events = mgr.evaluate_pool_size();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ProbeEvent::PoolScaledOut { .. }));

        if let ProbeEvent::PoolScaledOut { previous_size, new_size } = &events[0] {
            assert_eq!(*previous_size, 4);
            assert_eq!(*new_size, 4 + POOL_SCALE_STEP);
        }
        assert_eq!(mgr.target_size(), 4 + POOL_SCALE_STEP);
    }

    #[test]
    fn scale_in_triggers_at_low_utilisation() {
        let mut mgr = HealthProbeManager::new(8);
        // 0 of 8 in use = 0% ≤ 25%  → scale in
        let events = mgr.evaluate_pool_size();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ProbeEvent::PoolScaledIn { .. }));

        if let ProbeEvent::PoolScaledIn { previous_size, new_size } = &events[0] {
            assert_eq!(*previous_size, 8);
            assert_eq!(*new_size, 8 - POOL_SCALE_STEP);
        }
    }

    #[test]
    fn no_scaling_event_at_moderate_utilisation() {
        let mut mgr = HealthProbeManager::new(4);
        let ids: Vec<_> = mgr.connections.keys().copied().collect();

        // 2 of 4 = 50% — between 25% and 75%
        mgr.checkout(ids[0]);
        mgr.checkout(ids[1]);

        let events = mgr.evaluate_pool_size();
        assert!(events.is_empty());
    }

    #[test]
    fn pool_size_does_not_exceed_max() {
        let mut mgr = HealthProbeManager::new(POOL_SIZE_MAX);
        // Force utilisation to 100%
        let ids: Vec<_> = mgr.connections.keys().copied().collect();
        for id in &ids {
            mgr.checkout(*id);
        }

        let events = mgr.evaluate_pool_size();
        // At max size, no scale-out should be emitted.
        assert!(events.is_empty());
        assert_eq!(mgr.target_size(), POOL_SIZE_MAX);
    }

    #[test]
    fn pool_size_does_not_go_below_min() {
        let mut mgr = HealthProbeManager::new(POOL_SIZE_MIN);
        // 0% utilisation but already at minimum.
        let events = mgr.evaluate_pool_size();
        assert!(events.is_empty());
        assert_eq!(mgr.target_size(), POOL_SIZE_MIN);
    }

    #[test]
    fn due_connections_returns_unprobed_connections() {
        let mgr = HealthProbeManager::new(3);
        let t = now();
        let due = mgr.due_connections(t);
        // All freshly created connections have never been probed, so all are due.
        assert_eq!(due.len(), 3);
    }

    #[test]
    fn due_connections_excludes_recently_probed() {
        let mut mgr = HealthProbeManager::new(2);
        let ids: Vec<_> = mgr.connections.keys().copied().collect();
        let t = now();

        // Probe connection 0 right now.
        mgr.record_probe(ids[0], 10, true, t);

        // At the same instant, ids[0] is no longer due (interval not elapsed).
        let due = mgr.due_connections(t);
        assert!(!due.contains(&ids[0]));
        // ids[1] was never probed so it is still due.
        assert!(due.contains(&ids[1]));
    }

    #[test]
    fn forced_recovery_backoff_progression() {
        // Verifies exponential backoff follows: 500ms → 1s → 2s → 4s → 8s → 16s → 30s
        let expected_ms: &[u64] = &[1000, 2000, 4000, 8000, 16000, 30000, 30000];
        let mut state = ConnectionProbeState::new();
        let t = now();

        for &expected in expected_ms {
            state.record_failure(t);
            assert_eq!(
                state.probe_interval.as_millis() as u64,
                expected,
                "unexpected backoff interval after failure"
            );
        }
    }
}
