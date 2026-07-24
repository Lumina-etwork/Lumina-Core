//! Tamper-evident audit trail with hash-chain verification.
//!
//! This crate provides a lightweight append-only audit chain implementation
//! suitable for Lumina services that require provable integrity of critical
//! operational events.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const AUDIT_TRAIL_PREFIX: &[u8] = b"LUMINA_AUDIT_V1";

const CONFIG_SNAPSHOT_PREFIX: &[u8] = b"LUMINA_CONFIG_SNAPSHOT_V1";

/// A normalized runtime configuration item included in drift detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeConfigItem {
    /// Stable configuration key, for example `consensus.block_ms`.
    pub key: String,

    /// Runtime value represented in canonical string form.
    pub value: String,
}

/// Canonical runtime configuration snapshot for a service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeConfigSnapshot {
    /// Logical service or component that produced this snapshot.
    pub service: String,

    /// Monotonic or wall-clock snapshot timestamp in milliseconds.
    pub timestamp_ms: u64,

    /// Sorted canonical configuration items.
    pub items: Vec<RuntimeConfigItem>,
}

/// Severity assigned to a configuration drift finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftSeverity {
    /// Informational difference that should be tracked but does not page.
    Info,

    /// Difference that needs operator review.
    Warning,

    /// Difference on a critical key that should trigger alerting/canary gates.
    Critical,
}

/// A single runtime configuration drift finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftFinding {
    /// Configuration key that drifted.
    pub key: String,

    /// Expected/baseline value, if the key exists in the baseline.
    pub expected: Option<String>,

    /// Observed/runtime value, if the key exists in the runtime snapshot.
    pub observed: Option<String>,

    /// Classification used by monitors and deployment gates.
    pub severity: DriftSeverity,
}

/// Complete drift report for one service snapshot comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftReport {
    /// Service being compared.
    pub service: String,

    /// Baseline snapshot hash.
    pub baseline_hash: [u8; 32],

    /// Runtime snapshot hash.
    pub observed_hash: [u8; 32],

    /// Per-key findings. Empty means no drift.
    pub findings: Vec<DriftFinding>,
}

/// Runtime configuration drift detector with critical-key escalation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftDetector {
    /// Golden snapshot approved through review/deployment gates.
    pub baseline: RuntimeConfigSnapshot,

    /// Keys whose drift is security- or availability-critical.
    pub critical_keys: Vec<String>,
}

/// A single audit entry in the tamper-evident chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Sequence number within the chain.
    pub sequence: u64,

    /// Logical service or component name emitting the event.
    pub service: String,

    /// Action or event type being recorded.
    pub action: String,

    /// Hash of the event payload.
    pub payload_hash: [u8; 32],

    /// Unix timestamp in milliseconds when the entry was created.
    pub timestamp_ms: u64,

    /// Hash of the previous chain entry.
    pub prev_hash: [u8; 32],

    /// Hash for this entry, covering all fields above.
    pub entry_hash: [u8; 32],
}

/// Errors returned during audit chain verification.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuditError {
    #[error("entry hash mismatch at sequence {0}")]
    InvalidEntryHash(u64),

    #[error("previous hash mismatch at sequence {0}")]
    InvalidPreviousHash(u64),
}

/// An append-only tamper-evident audit chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditChain {
    /// The ordered audit entries.
    pub entries: Vec<AuditEntry>,

    /// The genesis hash used for the first record.
    pub genesis_hash: [u8; 32],
}

impl RuntimeConfigItem {
    /// Create a normalized runtime configuration item.
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        RuntimeConfigItem {
            key: key.into(),
            value: value.into(),
        }
    }
}

impl RuntimeConfigSnapshot {
    /// Build a canonical snapshot with entries sorted by key for deterministic hashing.
    pub fn new(
        service: impl Into<String>,
        timestamp_ms: u64,
        items: impl IntoIterator<Item = RuntimeConfigItem>,
    ) -> Self {
        let mut items: Vec<RuntimeConfigItem> = items.into_iter().collect();
        items.sort_by(|left, right| left.key.cmp(&right.key));
        RuntimeConfigSnapshot {
            service: service.into(),
            timestamp_ms,
            items,
        }
    }

    /// Compute a deterministic digest of the service name and normalized key/value pairs.
    pub fn config_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(CONFIG_SNAPSHOT_PREFIX);
        AuditEntry::update_with_length_prefixed(&mut hasher, self.service.as_bytes());
        for item in &self.items {
            AuditEntry::update_with_length_prefixed(&mut hasher, item.key.as_bytes());
            AuditEntry::update_with_length_prefixed(&mut hasher, item.value.as_bytes());
        }
        let result = hasher.finalize();
        let mut output = [0u8; 32];
        output.copy_from_slice(&result);
        output
    }

    /// Append this snapshot to an audit chain as a compact runtime configuration record.
    pub fn audit(&self, chain: &mut AuditChain) -> AuditEntry {
        let payload = self.config_hash();
        chain
            .append(
                self.service.clone(),
                "runtime_config_snapshot",
                &payload,
                self.timestamp_ms,
            )
            .clone()
    }

    fn value_for(&self, key: &str) -> Option<&str> {
        self.items
            .binary_search_by(|item| item.key.as_str().cmp(key))
            .ok()
            .map(|index| self.items[index].value.as_str())
    }
}

impl DriftDetector {
    /// Create a detector from an approved baseline snapshot and critical keys.
    pub fn new(
        baseline: RuntimeConfigSnapshot,
        critical_keys: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut critical_keys: Vec<String> = critical_keys.into_iter().map(Into::into).collect();
        critical_keys.sort();
        critical_keys.dedup();
        DriftDetector {
            baseline,
            critical_keys,
        }
    }

    /// Compare an observed runtime snapshot against the baseline.
    pub fn detect(&self, observed: &RuntimeConfigSnapshot) -> DriftReport {
        let mut keys: Vec<String> = self
            .baseline
            .items
            .iter()
            .map(|item| item.key.clone())
            .collect();
        keys.extend(observed.items.iter().map(|item| item.key.clone()));
        keys.sort();
        keys.dedup();

        let findings = keys
            .into_iter()
            .filter_map(|key| {
                let expected = self.baseline.value_for(&key).map(ToOwned::to_owned);
                let actual = observed.value_for(&key).map(ToOwned::to_owned);
                (expected != actual).then(|| DriftFinding {
                    severity: self.severity_for(&key, expected.as_deref(), actual.as_deref()),
                    key,
                    expected,
                    observed: actual,
                })
            })
            .collect();

        DriftReport {
            service: observed.service.clone(),
            baseline_hash: self.baseline.config_hash(),
            observed_hash: observed.config_hash(),
            findings,
        }
    }

    fn severity_for(
        &self,
        key: &str,
        expected: Option<&str>,
        observed: Option<&str>,
    ) -> DriftSeverity {
        if self
            .critical_keys
            .binary_search_by(|candidate| candidate.as_str().cmp(key))
            .is_ok()
        {
            DriftSeverity::Critical
        } else if expected.is_none() || observed.is_none() {
            DriftSeverity::Warning
        } else {
            DriftSeverity::Info
        }
    }
}

impl DriftReport {
    /// True when no configuration drift is present.
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    /// True when any critical configuration drift is present.
    pub fn has_critical_drift(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity == DriftSeverity::Critical)
    }
}

impl AuditEntry {
    /// Create a new audit entry.
    pub fn new(
        sequence: u64,
        service: impl Into<String>,
        action: impl Into<String>,
        payload: &[u8],
        timestamp_ms: u64,
        prev_hash: [u8; 32],
    ) -> Self {
        let service = service.into();
        let action = action.into();
        let payload_hash = Self::hash_payload(payload);
        let mut entry = AuditEntry {
            sequence,
            service,
            action,
            payload_hash,
            timestamp_ms,
            prev_hash,
            entry_hash: [0u8; 32],
        };
        entry.entry_hash = entry.compute_hash();
        entry
    }

    /// Compute the canonical hash for this entry.
    pub fn compute_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(AUDIT_TRAIL_PREFIX);
        hasher.update(self.sequence.to_le_bytes());
        hasher.update(self.timestamp_ms.to_le_bytes());
        Self::update_with_length_prefixed(&mut hasher, self.service.as_bytes());
        Self::update_with_length_prefixed(&mut hasher, self.action.as_bytes());
        hasher.update(self.payload_hash);
        hasher.update(self.prev_hash);
        let result = hasher.finalize();
        let mut output = [0u8; 32];
        output.copy_from_slice(&result);
        output
    }

    fn update_with_length_prefixed(hasher: &mut Sha256, data: &[u8]) {
        hasher.update((data.len() as u64).to_le_bytes());
        hasher.update(data);
    }

    /// Verify that the recorded hash matches the canonical hash.
    pub fn verify_hash(&self) -> bool {
        self.entry_hash == self.compute_hash()
    }

    fn hash_payload(payload: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(payload);
        let result = hasher.finalize();
        let mut output = [0u8; 32];
        output.copy_from_slice(&result);
        output
    }
}

impl AuditChain {
    /// Create a new audit chain with an optional genesis anchor.
    pub fn new(genesis_hash: Option<[u8; 32]>) -> Self {
        AuditChain {
            entries: Vec::new(),
            genesis_hash: genesis_hash.unwrap_or([0u8; 32]),
        }
    }

    /// Append a new entry to the chain.
    pub fn append(
        &mut self,
        service: impl Into<String>,
        action: impl Into<String>,
        payload: &[u8],
        timestamp_ms: u64,
    ) -> &AuditEntry {
        let sequence = self.entries.len() as u64 + 1;
        let prev_hash = self.current_root_hash();
        let entry = AuditEntry::new(sequence, service, action, payload, timestamp_ms, prev_hash);
        self.entries.push(entry);
        self.entries.last().unwrap()
    }

    /// Compute the current root hash for the chain.
    pub fn current_root_hash(&self) -> [u8; 32] {
        self.entries
            .last()
            .map(|entry| entry.entry_hash)
            .unwrap_or(self.genesis_hash)
    }

    /// Verify the full chain integrity and return the final root hash.
    pub fn verify(&self) -> Result<[u8; 32], AuditError> {
        let mut previous_hash = self.genesis_hash;
        for entry in &self.entries {
            if entry.prev_hash != previous_hash {
                return Err(AuditError::InvalidPreviousHash(entry.sequence));
            }
            if !entry.verify_hash() {
                return Err(AuditError::InvalidEntryHash(entry.sequence));
            }
            previous_hash = entry.entry_hash;
        }
        Ok(previous_hash)
    }

    /// Return the number of entries in the chain.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return true when no entries have been recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_verify_chain() {
        let mut chain = AuditChain::new(None);
        chain.append("identity", "register_node", b"node=peer1", 1_700_000_000);
        chain.append(
            "attestation",
            "ticket_issued",
            b"relay=relay1;target=peer1",
            1_700_000_100,
        );
        chain.append(
            "network",
            "route_added",
            b"peer=peer1;port=3478",
            1_700_000_200,
        );

        assert_eq!(chain.len(), 3);
        assert!(chain.verify().is_ok());
    }

    #[test]
    fn tamper_detected_when_payload_changes() {
        let mut chain = AuditChain::new(None);
        chain.append("identity", "register_node", b"node=peer1", 1_700_000_000);
        chain.append(
            "attestation",
            "ticket_issued",
            b"relay=relay1;target=peer1",
            1_700_000_100,
        );

        chain.entries[1].action = "ticket_revoked".to_string();
        assert_eq!(chain.verify(), Err(AuditError::InvalidEntryHash(2)));
    }

    #[test]
    fn tamper_detected_when_previous_hash_modified() {
        let mut chain = AuditChain::new(None);
        chain.append("network", "peer_added", b"peer=peer1", 1_700_000_000);
        chain.append("network", "peer_removed", b"peer=peer1", 1_700_000_100);

        chain.entries[1].prev_hash = [0u8; 32];
        assert_eq!(chain.verify(), Err(AuditError::InvalidPreviousHash(2)));
    }

    #[test]
    fn root_hash_changes_when_chain_changes() {
        let mut chain = AuditChain::new(None);
        let first_root = chain.current_root_hash();
        chain.append("identity", "register_node", b"node=peer1", 1_700_000_000);
        let second_root = chain.current_root_hash();

        assert_ne!(first_root, second_root);
        assert_eq!(chain.verify().unwrap(), second_root);
    }

    #[test]
    fn runtime_config_hash_is_order_independent() {
        let first = RuntimeConfigSnapshot::new(
            "consensus",
            1_700_000_000,
            vec![
                RuntimeConfigItem::new("quorum", "67"),
                RuntimeConfigItem::new("block_ms", "500"),
            ],
        );
        let second = RuntimeConfigSnapshot::new(
            "consensus",
            1_700_000_999,
            vec![
                RuntimeConfigItem::new("block_ms", "500"),
                RuntimeConfigItem::new("quorum", "67"),
            ],
        );

        assert_eq!(first.config_hash(), second.config_hash());
    }

    #[test]
    fn drift_detector_reports_value_add_remove_and_critical_findings() {
        let baseline = RuntimeConfigSnapshot::new(
            "gateway",
            1_700_000_000,
            vec![
                RuntimeConfigItem::new("rate_limit.rps", "1000"),
                RuntimeConfigItem::new("tls.min_version", "1.3"),
                RuntimeConfigItem::new("feature.canary", "false"),
            ],
        );
        let observed = RuntimeConfigSnapshot::new(
            "gateway",
            1_700_000_100,
            vec![
                RuntimeConfigItem::new("rate_limit.rps", "750"),
                RuntimeConfigItem::new("feature.extra", "true"),
            ],
        );

        let detector = DriftDetector::new(baseline, vec!["tls.min_version"]);
        let report = detector.detect(&observed);

        assert_eq!(report.findings.len(), 4);
        assert!(report.has_critical_drift());
        assert!(report.findings.iter().any(|finding| {
            finding.key == "tls.min_version" && finding.severity == DriftSeverity::Critical
        }));
        assert!(report.findings.iter().any(|finding| {
            finding.key == "feature.extra" && finding.severity == DriftSeverity::Warning
        }));
    }

    #[test]
    fn runtime_config_snapshot_can_be_audited() {
        let snapshot = RuntimeConfigSnapshot::new(
            "identity",
            1_700_000_000,
            vec![RuntimeConfigItem::new("registry.confirmations", "3")],
        );
        let mut chain = AuditChain::new(None);

        let entry = snapshot.audit(&mut chain);

        assert_eq!(entry.action, "runtime_config_snapshot");
        assert_eq!(chain.len(), 1);
        assert!(chain.verify().is_ok());
    }

    #[test]
    fn audit_entry_hash_is_deterministic() {
        let first = AuditEntry::new(
            1,
            "consensus",
            "proposal_submitted",
            b"proposal=block123",
            1_700_000_000,
            [0u8; 32],
        );
        let second = AuditEntry::new(
            1,
            "consensus",
            "proposal_submitted",
            b"proposal=block123",
            1_700_000_000,
            [0u8; 32],
        );

        assert_eq!(first.entry_hash, second.entry_hash);
    }
}
