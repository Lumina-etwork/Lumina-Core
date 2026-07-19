# Distributed Job Scheduler with Lease-based Worker Claiming

## Architecture

Lumina services enqueue deterministic job records keyed by `JobId`. Workers claim jobs through the scheduler using short-lived leases that include a worker id, expiration timestamp, and fencing epoch. Every completion or renewal must present the current lease tuple, which prevents stale workers from completing work after a lease expires and is reclaimed.

The core scheduler is intentionally storage-agnostic. Production deployments should execute each mutating method inside a backing-store compare-and-swap transaction, using `job_id` plus the lease epoch as the write fence. This keeps the critical claim path to one indexed queue read and one conditional write, supporting the sub-100ms P99 target when backed by a colocated quorum store.

## Claiming flow

1. Requeue expired leases using monotonic service time.
2. Select the highest-priority queued job in the requested queue, with the oldest enqueue timestamp as the tie breaker.
3. Increment attempts, fail the job when the attempt budget is exhausted, or grant a new lease.
4. Emit claim, conflict, expiration, and terminal-state metrics.

## Availability and deployment

Run scheduler API instances statelessly behind the existing service mesh. Store job rows in the highly available metadata store and roll out with blue-green deployment. Canary analysis should compare claim latency, lease conflict rate, queue depth, and completion rate before shifting traffic.

## Security

Worker identity must be authenticated before scheduler calls. Authorization policies should restrict workers to approved queues, redact payload data from logs by storing only payload hashes in scheduler metadata, and alert on high lease-conflict rates as a potential replay or credential-sharing signal.

## Monitoring

Expose the counters in `SchedulerMetrics` as service metrics:

- `scheduler_queued_jobs`
- `scheduler_leased_jobs`
- `scheduler_completed_jobs_total`
- `scheduler_failed_jobs_total`
- `scheduler_claim_attempts_total`
- `scheduler_claim_successes_total`
- `scheduler_lease_conflicts_total`
- `scheduler_lease_expirations_total`

Critical alerts should page when claim P99 exceeds 100ms for five minutes, lease conflicts exceed 5% of claims, or any queue remains above its SLO depth for fifteen minutes.
