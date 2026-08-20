use super::types::CommitteeSet;
use crate::crypto::types::Signature;

pub fn validate_attestation(
    sig: &Signature, 
    committee: &CommitteeSet,
    expected_epoch: u64
) -> Result<(), String> {
    // CRITICAL FIX: Reject attestations from old committee epochs
    if committee.committee_epoch != expected_epoch {
        return Err(format!(
            "Committee epoch mismatch: got {}, expected {}", 
            committee.committee_epoch, 
            expected_epoch
        ));
    }
    
    // Verify signature is from current committee members
    if !committee.signers.contains(&sig.public_key) {
        return Err("Signature not from current committee".to_string());
    }
    
    // TODO: Add actual signature verification logic here
    Ok(())
}
