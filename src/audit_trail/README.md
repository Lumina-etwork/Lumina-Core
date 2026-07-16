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
