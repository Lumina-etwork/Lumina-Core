# Dead Letter Queue for Failed Message Processing

## Architecture

The social messaging service uses a Dead Letter Queue (DLQ) for failures that
happen after a request has been accepted for message processing. The request path
records a compact failure envelope instead of retrying synchronously, keeping the
critical path below the 100ms P99 target.

Each DLQ entry stores:

- message, sender, and recipient identifiers when available;
- the processing stage that failed;
- an error class and sanitized error message;
- a payload digest instead of plaintext or encrypted content;
- retry counters, status, and retry scheduling timestamps.

The durable table is `message_dead_letters`. Runtime handlers use the bounded
`DeadLetterQueue` abstraction to provide constant-time enqueue behavior; workers
can persist or rehydrate entries from the database table.

## Operations

- Alert when pending DLQ entries are older than 5 minutes.
- Alert when retry exhaustion exceeds 1% of processed messages over 10 minutes.
- Dashboard panels should include pending count, oldest pending age, retry rate,
  retry success rate, and failures by stage/error class.
- Canary deployments should compare DLQ insertion rate and message send latency
  between baseline and canary before widening traffic.

## Security

DLQ entries must not contain message plaintext. Store only opaque identifiers,
sanitized error details, and payload digests so end-to-end encrypted messaging
confidentiality is preserved during incident response.
