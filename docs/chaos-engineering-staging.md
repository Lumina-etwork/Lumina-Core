# Blueprint for Chaos Engineering Testing in Staging

This blueprint defines how Lumina-Core runs controlled chaos engineering exercises in staging without compromising the 99.99% availability objective, the <100 ms P99 critical-path latency target, or the security review requirements for changes that affect all services.

## Goals and non-goals

### Goals

- Validate that smart-contract, core-engine, analytics, social, and operational workflows tolerate dependency failures before production rollout.
- Exercise failure handling, observability, rollback, and incident response paths with measurable success criteria.
- Keep experiments repeatable, auditable, and gated behind explicit staging only controls.
- Produce evidence for security review, blue-green promotion, canary analysis, and runbook updates.

### Non-goals

- Running destructive experiments in production.
- Changing protocol economics, contract state, or governance authority as part of a chaos test.
- Bypassing normal deployment, security review, or approval gates.

## System-wide architecture

```text
+-------------------+      +----------------------+      +-------------------+
| Experiment catalog| ---> | Staging chaos runner | ---> | Fault providers   |
| docs + manifests  |      | CI/manual job        |      | network, process, |
+-------------------+      +----------+-----------+      | storage, RPC      |
                                      |                  +---------+---------+
                                      v                            |
+-------------------+      +----------------------+               |
| Runbooks          | <--- | Evidence collector   | <-------------+
| rollback + comms  |      | logs, metrics, traces|
+-------------------+      +----------+-----------+
                                      |
                                      v
                           +----------------------+
                           | Canary decision gate |
                           | SLO/security checks  |
                           +----------------------+
```

The staging chaos runner is the only component allowed to trigger experiments. It consumes versioned experiment manifests, verifies guardrails, applies a fault through an approved provider, captures telemetry, and emits a pass/fail decision for deployment gates.

## Guardrails

Every experiment must define these guardrails before execution:

| Guardrail | Required value |
| --- | --- |
| Environment | `staging` only; production credentials and production RPC endpoints are denied. |
| Blast radius | One service, shard, worker pool, or synthetic tenant per experiment unless explicitly approved. |
| Critical-path latency | Abort if P99 exceeds 100 ms for 5 consecutive minutes. |
| Availability | Abort if synthetic availability drops below 99.99% during the observation window. |
| Security | Abort on authz failures, secret exposure alerts, unexpected privileged calls, or unsigned artifacts. |
| Rollback | A tested rollback command and owner must be recorded in the manifest. |
| Duration | Default 10 minutes; maximum 30 minutes without an incident commander approval. |

## Experiment catalog

Start with the following staging experiments and expand only after each scenario has a stable runbook and dashboard.

| ID | Scenario | Fault | Expected behavior | Primary signals |
| --- | --- | --- | --- | --- |
| `chaos-rpc-latency` | External Stellar/Soroban RPC latency | Add 250-500 ms latency and 2% packet loss to staging RPC egress. | Clients use retries/backoff, critical read paths remain under 100 ms P99 through cache hits, and write paths surface bounded retry errors. | RPC latency, retry count, endpoint-cache hit ratio, request P99. |
| `chaos-relay-partition` | Relay endpoint partition | Drop traffic to one relay endpoint or registry peer. | Relay registry removes unhealthy endpoint, traffic shifts to healthy endpoints, and connectivity proofs degrade gracefully. | Relay health, connectivity proof failures, error budget burn. |
| `chaos-db-failover` | Analytics/social database failover | Restart staging database primary or force read-only window. | Services reconnect, queues buffer writes, read-only errors are visible, and no data corruption appears after recovery. | DB connections, queue depth, failed writes, recovery time objective. |
| `chaos-worker-crash` | Background worker crash loop | Kill one analytics or attestation worker replica. | Supervisor restarts worker, duplicate processing is avoided, and idempotency checks pass. | Restart count, job lag, duplicate job detector, dead-letter count. |
| `chaos-contract-call-revert` | Contract dependency revert | Route synthetic canary calls to a failing mock contract in staging. | Callers map failures to explicit errors, alerts fire, and no privileged state transition succeeds unexpectedly. | Contract error codes, privileged-call audit logs, synthetic canary failures. |

## Implementation plan

1. **Design and document** experiment manifests with owner, blast radius, steady-state hypothesis, fault action, rollback action, and observability links.
2. **Implement core runner logic** as a staging-only job that validates manifests, enforces guardrails, and records immutable evidence for each run.
3. **Add comprehensive tests** for manifest validation, staging-only enforcement, abort thresholds, rollback execution, and evidence generation.
4. **Add monitoring and alerting** for latency, availability, security anomalies, retries, queue depth, worker restarts, and contract error codes.
5. **Deploy with blue-green strategy** by enabling the runner only in the green staging environment, executing low-risk experiments first, and promoting after canary analysis passes.
6. **Update runbooks** with trigger conditions, owner escalation, rollback steps, dashboard links, and post-experiment review templates.

## Monitoring and alerting requirements

Dashboards must show the following panels for every experiment window:

- Request volume, error rate, and P50/P95/P99 latency by service and route.
- Synthetic availability and error-budget burn rate.
- RPC endpoint health, relay registry membership, and endpoint-cache hit ratio.
- Database connection state, queue depth, replay lag, and dead-letter messages.
- Worker restart count, job duration, and duplicate-processing detector output.
- Security signals: failed authorization checks, unexpected privileged calls, secret-scanner alerts, and artifact signature verification.

Alerts must page the staging incident commander when an abort guardrail is breached and must notify the owning service channel when an experiment starts, aborts, rolls back, or completes.

## Blue-green and canary analysis

1. Deploy the chaos runner and manifests to the green staging environment while blue remains a clean fallback.
2. Run smoke tests and synthetic canaries against green before fault injection.
3. Execute one experiment at a time with the documented blast radius.
4. Compare green against blue for P99 latency, availability, error rate, and security events.
5. Promote only when canary metrics remain within guardrails, rollback is not invoked, and evidence is attached to the change record.
6. Roll back to blue immediately if an abort guardrail is breached or if telemetry is incomplete.

## Runbook template

Each experiment runbook must include:

- Experiment ID, service owner, incident commander, and approval record.
- Steady-state hypothesis and expected telemetry.
- Exact start, observe, abort, and rollback commands.
- Dashboard and log links.
- Customer impact assessment for staging users and synthetic tenants.
- Post-experiment review notes, defects filed, and follow-up owner.

## Security review checklist

- Confirm the runner rejects production endpoints, production secrets, and non-staging namespaces.
- Confirm experiment artifacts are signed or generated by trusted CI.
- Confirm least-privilege credentials for fault providers.
- Confirm audit logs capture who started an experiment, what was changed, and how rollback completed.
- Confirm contract-call experiments use mocks or synthetic staging state only.
