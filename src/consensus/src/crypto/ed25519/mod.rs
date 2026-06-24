/// Ed25519 batch verification utilities.
///
/// Re-exports from the parent crypto module with ergonomic helpers
/// for multi-signature aggregation on quorum certificates.
use crate::crypto::MultiSignature;
use std::collections::BTreeSet;

/// Verify a batch of Ed25519 signatures against their messages and public keys.
///
/// This is the low-level entry point called by `qc-validator.rs` during
/// QC validation. The batch API can verify up to 128 signatures in a single
/// aggregate operation.
pub fn verify_batch(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), BatchVerifyError> {
    if messages.len() != signatures.len() || signatures.len() != public_keys.len() {
        return Err(BatchVerifyError::MismatchedBatchSizes {
            messages: messages.len(),
            signatures: signatures.len(),
            keys: public_keys.len(),
        });
    }

    if messages.is_empty() {
        return Err(BatchVerifyError::EmptyBatch);
    }

    if messages.len() > 128 {
        return Err(BatchVerifyError::BatchTooLarge {
            size: messages.len(),
            max: 128,
        });
    }

    // Production: use ed25519_dalek::verify_batch
    // For now, verify each signature individually
    for (i, (msg, sig)) in messages.iter().zip(signatures.iter()).enumerate() {
        if sig.len() < 64 {
            return Err(BatchVerifyError::InvalidSignature { index: i });
        }
        let mut keys = BTreeSet::new();
        if let Some(key) = public_keys.get(i) {
            keys.insert(key.to_vec());
        }
        let msig = MultiSignature {
            message: msg.to_vec(),
            signature: sig.to_vec(),
            public_keys: keys,
            signature_count: 1,
        };
        if !super::batch_verify(&msig) {
            return Err(BatchVerifyError::VerificationFailed { index: i });
        }
    }

    Ok(())
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum BatchVerifyError {
    #[error("Mismatched batch sizes: {messages} messages, {signatures} signatures, {keys} keys")]
    MismatchedBatchSizes {
        messages: usize,
        signatures: usize,
        keys: usize,
    },
    #[error("Empty batch")]
    EmptyBatch,
    #[error("Batch too large: {size} items, max {max}")]
    BatchTooLarge { size: usize, max: usize },
    #[error("Invalid signature at index {index}")]
    InvalidSignature { index: usize },
    #[error("Verification failed at index {index}")]
    VerificationFailed { index: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_batch_empty() {
        let result = verify_batch(&[], &[], &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_batch_mismatched() {
        let result = verify_batch(&[b"msg"], &[b"sig"], &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_batch_too_large() {
        let mut msgs = vec![];
        let mut sigs = vec![];
        let mut keys = vec![];
        for i in 0..129 {
            msgs.push(vec![i as u8; 32]);
            sigs.push(vec![0u8; 64]);
            keys.push(vec![0u8; 32]);
        }
        let msg_refs: Vec<&[u8]> = msgs.iter().map(|v| v.as_slice()).collect();
        let sig_refs: Vec<&[u8]> = sigs.iter().map(|v| v.as_slice()).collect();
        let key_refs: Vec<&[u8]> = keys.iter().map(|v| v.as_slice()).collect();
        let result = verify_batch(&msg_refs, &sig_refs, &key_refs);
        assert!(result.is_err());
    }
}