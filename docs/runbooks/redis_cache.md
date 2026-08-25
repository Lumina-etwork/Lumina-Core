# Redis Cache Runbook

## Monitoring and Alerting
- Dashboard: Grafana dashboard `Redis Overview`.
- Alerting: PagerDuty triggered on cache hit ratio drop below 70% or latency P99 > 100ms.

## Troubleshooting
1. Check Redis cluster health.
2. Verify network latency.
3. Validate TTL configuration settings.
