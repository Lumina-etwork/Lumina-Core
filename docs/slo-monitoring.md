# Service Level Objective Monitoring and Burn Rate Alerts

## Objectives

Lumina Core tracks a system-wide availability SLO of **99.99%** and a critical-path latency SLO of **P99 <= 100 ms**. Every service emits request totals, error totals, and latency histograms with the labels `service`, `route`, `method`, and `environment`.

## Architecture

1. Services record SLO events by incrementing request and error counters and by observing latency histograms at request boundaries.
2. Prometheus scrapes each service every 15 seconds and evaluates multi-window burn-rate recording rules.
3. Alertmanager routes page-level burn-rate alerts to the primary on-call and ticket-level alerts to the owning service team.
4. Grafana dashboards show budget remaining, burn rate, latency P99, and canary health for each service.
5. Deployments use blue-green rollout with a 5% canary gate. Canary promotion is blocked if burn rate reaches page severity or P99 exceeds 100 ms.

## Alert Policy

| Alert | Window | Burn rate | Action |
| --- | --- | --- | --- |
| Critical fast burn | 5m and 1h | >= 14.4x | Page on-call |
| Critical slow burn | 30m and 6h | >= 6x | Page on-call |
| Warning burn | 2h and 1d | >= 3x | Open ticket |
| Latency SLO breach | 5m | P99 > 100 ms | Page service owner |

## Runbook

1. Open the SLO dashboard and identify the service, route, and environment with the highest burn rate.
2. Compare current deployment markers with the start of the burn-rate window.
3. If a canary or blue-green switch is active, pause promotion and shift traffic back to the last healthy color.
4. Check dependency, database, and chain-finality panels for correlated saturation or lag.
5. Mitigate by rolling back, disabling the failing feature flag, or scaling the saturated component.
6. After mitigation, verify burn rate is below 1x for at least one short window and create a post-incident review for consumed budget.

## Security Review Checklist

- Metrics endpoints expose no secrets, tokens, wallet addresses, or personally identifiable information.
- Dashboard access is restricted to authenticated operators.
- Alert payloads include service identifiers and symptoms only, not request bodies.
- Runbook actions requiring traffic shifts or rollbacks are audited.
