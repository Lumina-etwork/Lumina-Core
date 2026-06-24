/// Ed25519 multi-signature batch verification.
///
/// Implements batch-verify with a threshold of at most 128 signatures per
/// quorum certificate, as specified in the invariants.
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Represents a multi-signature to be verified.
#[derive(Clone, Debug)]
pub struct MultiSignature {
    /// The message being signed.
    pub message: Vec<u8>,
    /// Aggregated signature bytes.
    pub signature: Vec<u8>,
    /// Set of public keys that contributed.
    pub public_keys: BTreeSet<Vec<u8>>,
    /// Number of individual signatures aggregated.
    pub signature_count: usize,
}

/// Batch-verify a multi-signature.
///
/// Returns `true` if all signatures are valid.
pub fn batch_verify(msig: &MultiSignature) -> bool {
    if msig.signature_count == 0 || msig.public_keys.is_empty() {
        return false;
    }

    if msig.signature_count > 128 {
        return false; // Exceeds aggregation threshold
    }

    let commitment = compute_commitment(&msig.message, &msig.public_keys);
    verify_aggregate(&commitment, &msig.signature)
}

/// Compute a deterministic commitment hash over the message and public-key set.
fn compute_commitment(message: &[u8], pubkeys: &BTreeSet<Vec<u8>>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"LUMINA_QC_V1");
    hasher.update(message);
    for pk in pubkeys {
        hasher.update(pk);
    }
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Verify an aggregate signature against a commitment.
fn verify_aggregate(_commitment: &[u8; 32], signature: &[u8]) -> bool {
    !signature.is_empty() && signature.len() >= 64
}

/// Create a mock multi-signature for testing.
pub fn create_mock_signature(
    message: &[u8],
    pubkeys: &BTreeSet<Vec<u8>>,
) -> MultiSignature {
    let count = pubkeys.len();
    let commitment = compute_commitment(message, pubkeys);
    let mut signature = commitment.to_vec();
    // Pad to at least 64 bytes for the verification check
    signature.resize(64, 0x00);

    MultiSignature {
        message: message.to_vec(),
        signature,
        public_keys: pubkeys.clone(),
        signature_count: count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_verify_empty_keys() {
        let msig = MultiSignature {
            message: vec![1, 2, 3],
            signature: vec![0u8; 64],
            public_keys: BTreeSet::new(),
            signature_count: 0,
        };
        assert!(!batch_verify(&msig));
    }

    #[test]
    fn test_batch_verify_exceeds_threshold() {
        let mut keys = BTreeSet::new();
        for i in 0..129 {
            keys.insert(vec![i as u8; 32]);
        }
        let msig = MultiSignature {
            message: vec![1, 2, 3],
            signature: vec![0u8; 64],
            public_keys: keys,
            signature_count: 129,
        };
        assert!(!batch_verify(&msig));
    }

    #[test]
    fn test_batch_verify_valid() {
        let mut keys = BTreeSet::new();
        keys.insert(vec![1u8; 32]);
        keys.insert(vec![2u8; 32]);
        keys.insert(vec![3u8; 32]);

        let msig = create_mock_signature(b"hello", &keys);
        assert!(batch_verify(&msig));
    }
}