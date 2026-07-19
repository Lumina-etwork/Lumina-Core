# Distributed Job Scheduler Runbook

## Symptoms

- Claim P99 latency exceeds 100ms.
- Lease conflict rate exceeds 5% of claim attempts.
- Queue depth increases while workers report healthy heartbeats.

## Triage

1. Check backing-store latency and conditional-write conflict dashboards.
2. Verify workers have synchronized monotonic clocks and are renewing before half of the lease TTL elapses.
3. Confirm no deployment is mixing incompatible lease epoch semantics.
4. Inspect queue-level authorization denials for worker credential issues.

## Mitigation

- Increase worker concurrency only when backing-store latency is healthy.
- Temporarily lengthen lease TTL for long-running queues to reduce false expirations.
- Roll back the blue-green target when canary conflict or latency SLOs regress.
- Pause low-priority queues if critical queues approach SLO depth.
