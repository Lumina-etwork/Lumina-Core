# Capacity Planning Runbook

## Alerts

### CapacityProjectionAboveThreshold

1. Confirm whether `scale_out_required` is true for the affected service and region.
2. Compare the projected usage with deploy, traffic, and batch-job events from the last 24 hours.
3. If growth is organic, provision the `recommended_capacity_units` returned by the planner.
4. If growth is anomalous, disable automated scale-out and escalate to security review.

### CapacityPlannerLatencyHigh

1. Verify callers are sending bounded rolling windows rather than unbounded history.
2. Reduce the per-service sample window until P99 is below 100 ms.
3. Keep serving traffic fail-open; planner failures must not block requests.

## Blue-Green and Canary Checks

- Green must match blue's scale-out decisions for at least 95% of services during canary.
- No critical latency alert may fire during canary.
- Roll back by disabling the planner feature flag in green and keeping blue as the serving environment.
