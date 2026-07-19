# Structured Logging with OpenTelemetry Semantic Conventions

## Architecture

Lumina services emit JSON logs through `tracing` and `tracing-subscriber`. Each runnable service initializes logging once during startup and attaches OpenTelemetry resource attributes to startup and request events:

- `service.name`
- `service.version`
- `deployment.environment.name`
- networking attributes such as `network.local.address` and `network.local.port`
- HTTP attributes such as `http.request.method` and `url.path`
- user/session/message attributes for WebSocket and analytics events

The format is intentionally line-delimited JSON so collectors can ingest it without regex parsing. Operators should route stdout/stderr to the platform collector, enrich with deployment metadata, and forward to the central log backend.

## Performance and availability guardrails

- Keep log calls on critical paths at `info` or lower volume; use `debug` for chatty WebSocket events.
- Do not log message bodies, secrets, bearer tokens, database URLs, or private keys.
- Prefer identifiers, counts, sizes, and status values over payloads.
- Configure `RUST_LOG` per environment instead of recompiling.
- Use rolling/canary deployments and compare P99 latency before increasing log volume.

## Monitoring and alerting

Recommended dashboard panels:

- log events by `service.name`, `deployment.environment.name`, and level
- error rate by `http.request.method` and `url.path`
- WebSocket disconnects and heartbeat failures
- prediction request volume and failure count
- P95/P99 request latency from metrics or traces correlated by service fields

Recommended alerts:

- sustained error-level log rate above baseline for 5 minutes
- WebSocket heartbeat failures above baseline for 10 minutes
- missing logs from any production service for 2 minutes
- P99 latency above 100 ms on critical paths during canary analysis

## Deployment runbook

1. Deploy the new version to the green environment with `DEPLOYMENT_ENVIRONMENT` set.
2. Route 5% of traffic to green and verify JSON log ingestion within 2 minutes.
3. Compare error rate, heartbeat failures, and P99 latency against blue.
4. Increase traffic to 25%, 50%, then 100% when canary checks remain healthy.
5. Roll back to blue immediately if structured logs stop arriving, critical path P99 exceeds 100 ms, or error logs breach alert thresholds.
