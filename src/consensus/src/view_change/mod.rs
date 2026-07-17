/// View-change quorum certificate validation logic.
///
/// Implements:
/// - Monotonic `qc_epoch` counter incremented by the primary on proposal
/// - Quarantine buffer for conflicting QCs (hold for 2 view-change rounds)
/// - Certificate validity checks (quorum size, signature aggregation threshold)
use crate::QuorumCertificate;
use std::collections::HashMap;

/// Configuration for QC validation.
#[derive(Clone, Debug)]
pub struct QcValidatorConfig {
    /// Total replicas (3f + 1).
    pub n: usize,
    /// Max byzantine fault threshold n = 3f + 1.
    pub f: usize,
    /// Signature aggregation threshold (max sigs per cert).
    pub max_sigs_per_cert: usize,
    /// Number of view-change rounds to hold quarantined QCs.
    pub quarantine_rounds: u64,
}

impl Default for QcValidatorConfig {
    fn default() -> Self {
        Self {
            n: 7, // 3f+1 with f=2
            f: 2,
            max_sigs_per_cert: 128,
            quarantine_rounds: 2,
        }
    }
}

/// A quarantine buffer that holds conflicting QCs for a fixed number
/// of view-change rounds before garbage collection.
#[derive(Clone, Debug)]
pub struct QuarantineBuffer {
    /// Map from (view, block_hash) -> (qc, expiration_view)
    entries: HashMap<(u64, [u8; 32]), (QuorumCertificate, u64)>,
    config: QcValidatorConfig,
}

impl QuarantineBuffer {
    pub fn new(config: QcValidatorConfig) -> Self {
        Self {
            entries: HashMap::new(),
            config,
        }
    }

    /// Insert a conflicting QC into the quarantine buffer.
    pub fn insert(&mut self, qc: QuorumCertificate, current_view: u64) {
        let expires = current_view + self.config.quarantine_rounds;
        self.entries.insert((qc.view, qc.block_hash), (qc, expires));
    }

    /// Garbage-collect expired entries.
    pub fn gc(&mut self, current_view: u64) {
        self.entries
            .retain(|_, (_, expires)| *expires > current_view);
    }

    /// Get all quarantined QCs for a given view.
    pub fn get_for_view(&self, view: u64) -> Vec<&QuorumCertificate> {
        self.entries
            .iter()
            .filter(|((v, _), _)| *v == view)
            .map(|(_, (qc, _))| qc)
            .collect()
    }

    /// Number of currently quarantined entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Validator that checks quorum certificates for correctness.
#[derive(Clone, Debug)]
pub struct QcValidator {
    config: QcValidatorConfig,
    quarantine: QuarantineBuffer,
    /// Events emitted during validation.
    pub events: Vec<crate::ConsensusEvent>,
}

impl QcValidator {
    pub fn new(config: QcValidatorConfig) -> Self {
        Self {
            quarantine: QuarantineBuffer::new(config.clone()),
            config,
            events: Vec::new(),
        }
    }

    /// Validate a single QC.
    pub fn validate(
        &self,
        qc: &QuorumCertificate,
        current_view: u64,
    ) -> Result<(), QcValidationError> {
        // Check certificate validity window
        if !qc.is_valid(current_view) {
            return Err(QcValidationError::ExpiredCertificate {
                view: qc.view,
                expires_after: qc.expires_after_views,
                current_view,
            });
        }

        // Check quorum size: need at least 2f+1 signers
        let quorum_size = 2 * self.config.f + 1;
        if qc.signer_set.len() < quorum_size {
            return Err(QcValidationError::InsufficientSigners {
                got: qc.signer_set.len(),
                required: quorum_size,
            });
        }

        // Check signature aggregation limit
        if qc.signer_set.len() > self.config.max_sigs_per_cert {
            return Err(QcValidationError::SignatureLimitExceeded {
                sigs: qc.signer_set.len(),
                max: self.config.max_sigs_per_cert,
            });
        }

        Ok(())
    }

    /// Validate and merge a new QC against existing QCs.
    pub fn validate_and_merge(
        &mut self,
        qc: QuorumCertificate,
        existing: &mut Vec<QuorumCertificate>,
        current_view: u64,
    ) -> Result<(), QcValidationError> {
        // Run basic validation
        self.validate(&qc, current_view)?;

        // Check for conflicts with existing QCs
        for existing_qc in existing.iter() {
            if existing_qc.view == qc.view
                && existing_qc.qc_epoch == qc.qc_epoch
                && existing_qc.block_hash != qc.block_hash
            {
                // Conflicting QCs detected — quarantine
                self.events.push(crate::ConsensusEvent::QcConflictDetected {
                    view: qc.view,
                    qc_epoch_a: existing_qc.qc_epoch,
                    qc_epoch_b: qc.qc_epoch,
                    block_hash_a: existing_qc.block_hash,
                    block_hash_b: qc.block_hash,
                });

                self.quarantine.insert(qc.clone(), current_view);
                return Err(QcValidationError::ConflictDetected {
                    view: qc.view,
                    qc_epoch: qc.qc_epoch,
                });
            }
        }

        // No conflict — add to active set
        existing.push(qc);
        Ok(())
    }

    /// Run garbage collection on the quarantine buffer.
    pub fn gc(&mut self, current_view: u64) {
        self.quarantine.gc(current_view);
    }

    /// Access the quarantine buffer (for debugging / testing).
    pub fn quarantine(&self) -> &QuarantineBuffer {
        &self.quarantine
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum QcValidationError {
    #[error("QC for view {view} expired: expires_after={expires_after}, current={current_view}")]
    ExpiredCertificate {
        view: u64,
        expires_after: u64,
        current_view: u64,
    },
    #[error("Insufficient signers: got {got}, required {required}")]
    InsufficientSigners { got: usize, required: usize },
    #[error("Signature limit exceeded: {sigs} sigs, max {max}")]
    SignatureLimitExceeded { sigs: usize, max: usize },
    #[error("Conflict detected at view {view}, epoch {qc_epoch}")]
    ConflictDetected { view: u64, qc_epoch: u64 },
}
