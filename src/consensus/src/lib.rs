pub mod view_change;
pub mod pacemaker;
pub mod commit;
pub mod crypto;

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

/// A quorum certificate (QC) for BFT view-change.
///
/// Carries a `qc_epoch` monotonic counter for deterministic tie-breaking
/// when network partitions produce divergent QCs at the same view number.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuorumCertificate {
    /// Consensus view number during which this QC was produced.
    pub view: u64,

    /// Monotonic epoch counter. Incremented by the primary on proposal.
    /// Used as the primary tie-breaker in conflict resolution.
    pub qc_epoch: u64,

    /// Ed25519 multi-signature over the QC payload.
    pub signature: Vec<u8>,

    /// The set of public keys whose signatures are aggregated.
    pub signer_set: BTreeSet<Vec<u8>>,

    /// Hash of the proposed block / value this QC certifies.
    pub block_hash: [u8; 32],

    /// Timestamp of QC creation (Unix nanos).
    pub created_ns: u64,

    /// View number in which the next epoch begins (for garbage collection).
    pub expires_after_views: u64,
}

impl QuorumCertificate {
    /// Create a new QC with an incremented epoch.
    pub fn new(
        view: u64,
        qc_epoch: u64,
        block_hash: [u8; 32],
        signer_set: BTreeSet<Vec<u8>>,
        signature: Vec<u8>,
    ) -> Self {
        let created_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self {
            view,
            qc_epoch,
            signature,
            signer_set,
            block_hash,
            created_ns,
            expires_after_views: view + 2, // validity window: 2 epochs
        }
    }

    /// QC is still valid (within its certificate validity window).
    pub fn is_valid(&self, current_view: u64) -> bool {
        current_view <= self.expires_after_views
    }

    /// Deterministic hash of the aggregated public-key set for tie-breaking.
    pub fn pubkey_set_hash(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for pk in &self.signer_set {
            hasher.update(pk);
        }
        let result = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result);
        out
    }
}

/// Events emitted by the consensus engine for observability.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConsensusEvent {
    QcConflictDetected {
        view: u64,
        qc_epoch_a: u64,
        qc_epoch_b: u64,
        block_hash_a: [u8; 32],
        block_hash_b: [u8; 32],
    },
    ViewChangeStarted {
        view: u64,
        reason: String,
    },
    ViewChangeCompleted {
        view: u64,
        new_primary: Vec<u8>,
    },
}