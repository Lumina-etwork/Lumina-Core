# Capacity Planning with Historical Usage Trending

## Architecture

The capacity-planning path is implemented as an in-process deterministic planner so every service can evaluate capacity without remote calls on critical paths. Services submit timestamped `UsageSample` values containing provisioned and consumed capacity units. `CapacityPlanner` sorts the bounded sample window, computes an integer linear trend in units per day, forecasts demand over the configured horizon, and returns a `CapacityPlan` with required headroom and scale-out status.

## Operating Targets

- Critical-path evaluation target: keep each planner invocation under 100 ms P99 by passing a bounded rolling sample window.
- Availability target: 99.99% by keeping planning local and making monitoring/alerts fail-open for serving traffic.
- Security target: samples must be aggregate capacity counters only; do not include tenant secrets or personally identifiable data.

## Deployment Strategy

1. Ship the planner behind a feature flag in the green environment.
2. Mirror historical usage samples into blue and green without taking scaling actions.
3. Run canary analysis for projected utilization, recommendation deltas, and planner latency.
4. Enable automated scale-out actions only after alert noise is below the runbook threshold for one full forecast window.

## Dashboards

The capacity dashboard should include current utilization, projected utilization, recommended capacity units, trend per day, planner latency P99, and scale-out decision counts. Alert ownership belongs to the service SRE rotation.
