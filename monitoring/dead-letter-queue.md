# Dead Letter Queue Monitoring

Recommended metrics:

- `message_dlq_pending_total` by `failure_stage` and `error_class`.
- `message_dlq_oldest_pending_age_seconds`.
- `message_dlq_retry_attempts_total` by retry result.
- `message_dlq_resolved_total`.
- `message_send_latency_ms` with P50, P95, and P99 views.

Recommended alerts:

- Critical: oldest pending DLQ entry is older than 15 minutes.
- Warning: oldest pending DLQ entry is older than 5 minutes.
- Critical: message send P99 latency exceeds 100ms for 5 minutes.
- Warning: DLQ insertion rate increases by 3x over the 1-hour baseline.
