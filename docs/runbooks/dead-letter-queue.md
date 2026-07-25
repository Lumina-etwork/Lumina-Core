# Dead Letter Queue Runbook

## Triage

1. Check `message_dead_letters` for entries with `status IN ('pending', 'retrying')`.
2. Group failures by `failure_stage` and `error_class` to identify systemic
   outages versus malformed individual messages.
3. Verify message send P99 latency remains below 100ms and DLQ enqueue rate is
   not masking a database or worker outage.

## Retry

1. Select entries whose `next_retry_at <= NOW()`.
2. Move selected rows to `retrying` before replay to avoid duplicate workers.
3. Re-run the failed stage using the original message identifier or a safe
   replay mechanism.
4. Mark successful entries as `resolved` and set `resolved_at = NOW()`.

## Escalation

Escalate to security review if an entry includes sensitive payload material or if
error messages expose secrets. Escalate to incident response when unresolved DLQ
age exceeds 15 minutes or pending queue depth continues to grow for 10 minutes.
