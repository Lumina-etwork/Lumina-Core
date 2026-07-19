# Incident Response Runbook Automation with PagerDuty

## Architecture

Lumina incident automation standardizes SLO breach handling for all services. The core library classifies incidents from service SLOs, observed P99 latency, and availability basis points, then builds PagerDuty Events API v2 payloads enriched with runbook steps and deployment context.

```text
metrics -> SLO classifier -> PagerDuty event builder -> PagerDuty Events API
                                 |
                                 +-> runbook URL, dashboard links, canary strategy
```

## SLO bounds

- Critical path latency target: P99 below `100ms`.
- Availability target: `99.99%` (`9999` basis points).
- Security: route every automation change through security review before production enablement.

## PagerDuty integration

1. Store the PagerDuty Events API routing key in secret management as `PAGERDUTY_ROUTING_KEY`.
2. Instantiate `IncidentRunbookAutomation` with the routing key and this runbook URL.
3. Send the JSON returned by `PagerDutyEvent::to_json_body()` to `https://events.pagerduty.com/v2/enqueue` from the service-specific adapter.
4. Never log the routing key or raw secret values; log only the `dedup_key`, service, severity, and PagerDuty response status.

## Runbook workflow

1. Acknowledge the PagerDuty incident within five minutes.
2. Open the `Lumina Incident Response` dashboard and confirm whether the latency or availability alert is still firing.
3. Run the service health command listed in the PagerDuty custom details.
4. If the service is on a new release, pause rollout and shift traffic back to the blue environment.
5. Start canary analysis at 5%, then continue through 25%, 50%, and 100% only when SLO burn rate and error budget checks are clean.
6. Resolve the PagerDuty incident only after alerts have been green for two consecutive evaluation windows.
7. File a post-incident review for every critical incident.

## Blue-green and canary deployment gates

- Maintain blue and green stacks with independent health checks.
- Promote green only when smoke checks, security checks, and SLO canaries pass.
- Abort deployment when P99 exceeds `100ms`, availability drops below `99.99%`, or security telemetry flags a critical finding.

## Monitoring and dashboards

Import `monitoring/incident_response_alerts.yaml` into the monitoring stack. Dashboards must show:

- P99 latency by service.
- Availability by service.
- PagerDuty trigger, acknowledge, and resolve counts.
- Canary error budget burn rate.
- Current blue/green traffic split.
