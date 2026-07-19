# Sensitive Payload Field E2EE Runbook

## Dashboards

Import `monitoring/sensitive_payload_dashboard.json` and pin the following
service-level indicators:

- P99 field encryption/decryption latency below 100ms.
- Authentication failure rate below 0.1% of protected fields.
- Payload rejection rate below 0.5% during canary and below 0.1% steady state.

## Alerts

Page security and service owners when any of these conditions hold for five
minutes:

- `sensitive_payload_crypto_auth_failures_total` increases above baseline.
- P99 latency for `sensitive_payload_crypto_duration_seconds` exceeds 100ms.
- A service logs plaintext for a field classified as `Sensitive`.

## Blue-green and canary rollout

1. Deploy the new build to the green environment with encryption in observe-only
   mode for one canary shard.
2. Enable encryption for 5% of traffic and compare latency and rejection metrics
   against blue for at least 30 minutes.
3. Increase to 25%, 50%, and 100% only if authentication failures and P99
   latency remain within SLO.
4. Roll back to blue immediately if rejection rate exceeds the alert threshold
   or if security review identifies plaintext exposure.

## Key rotation

1. Publish recipient key metadata with a new key id.
2. Encrypt new fields with the new key id while accepting the previous key id.
3. After the maximum payload TTL, disable decrypt for the previous key id.
4. Record the rotation in the security review log.
