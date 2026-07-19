# Lumina Audit Trail

This crate implements a tamper-evident audit trail for Lumina services.
It uses a chained cryptographic digest to ensure every audit record depends on all previous entries.

## Architecture

- `AuditEntry`: Records a single event with a service name, action, payload hash, timestamp, and the previous entry hash.
- `AuditChain`: Maintains an ordered append-only list of `AuditEntry` records.
- `verify()`: Walks the chain to validate both individual entry hashes and the chain linkage.

By anchoring each record to the previous entry hash, the audit chain becomes tamper-evident. Any modification to an earlier entry corrupts the final chain root.

## Service Integration

Lumina services can integrate the audit trail by:

1. Including `lumina-audit` as a dependency.
2. Creating an `AuditChain` or service-specific wrapper.
3. Appending audit events for critical state transitions.
4. Periodically calling `verify()` and comparing the current root hash against a trusted reference.

### Consensus

The consensus engine adds a helper for converting `ConsensusEvent` values into audit entries, so view-change and QC conflicts can be tracked in the same chain.

### Core Engine

The core engine includes a dedicated wrapper around `AuditChain` to capture attestation, relay, and endpoint events.

## Monitoring and Alerting

Recommended monitoring:

- Alert when `verify()` returns a chain integrity error.
- Track the current root hash as an observability metric.
- Emit audit chain root changes whenever new entries are appended.

Dashboards should visualize:

- Audit chain length over time.
- Verification success/failure rate.
- Latest root hash compared to a trusted anchor.

## Runtime Configuration Auditing and Drift Detection

Runtime configuration snapshots provide a system-wide control for detecting
configuration drift before it impacts security or availability. Each service
should publish an approved baseline during deployment and periodically compare
live settings against that baseline.

### Solution Architecture

1. **Baseline capture**: deployment tooling creates a `RuntimeConfigSnapshot`
   from reviewed settings after security approval.
2. **Runtime sampling**: each service converts live settings into sorted
   `RuntimeConfigItem` values and computes a deterministic `config_hash()`.
3. **Audit anchoring**: services call `RuntimeConfigSnapshot::audit()` to add a
   compact `runtime_config_snapshot` event to the tamper-evident `AuditChain`.
4. **Drift detection**: `DriftDetector` compares the runtime snapshot with the
   baseline and emits a `DriftReport` with per-key severity.
5. **Deployment safety**: blue-green and canary pipelines should block promotion
   when `DriftReport::has_critical_drift()` is true.

The detector avoids wall-clock timestamp input in the configuration digest, so
hashes remain stable for identical service settings and can be compared in
low-latency critical paths.

### Monitoring, Alerting, and Dashboards

Recommended metrics and alerts:

- `runtime_config_drift_findings_total{service,severity}` from each
  `DriftReport`.
- `runtime_config_snapshot_hash{service}` as the latest observed config hash.
- Page on any critical finding, and create a ticket for warning findings.
- Dashboard panels for clean/dirty services, critical drift count, latest
  baseline hash versus observed hash, and audit-chain verification status.

### Runbook

1. Verify the service's audit chain with `AuditChain::verify()`.
2. Compare the `baseline_hash` and `observed_hash` in the latest `DriftReport`.
3. For critical drift, stop canary promotion or shift traffic back to blue.
4. Reconcile the live configuration with the approved baseline, or submit a new
   baseline through security review.
5. Confirm `DriftReport::is_clean()` before resuming rollout.
