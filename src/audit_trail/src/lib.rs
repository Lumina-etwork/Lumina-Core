//! Tamper-evident audit trail with hash-chain verification.
//!
//! This crate provides a lightweight append-only audit chain implementation
//! suitable for Lumina services that require provable integrity of critical
//! operational events.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const AUDIT_TRAIL_PREFIX: &[u8] = b"LUMINA_AUDIT_V1";

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
        hasher.update(&self.payload_hash);
        hasher.update(&self.prev_hash);
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
