/// Attestation verifier with grace-period key fallback.
///
/// Verification strategy:
///   1. Fetch all keys valid at the attestation's epoch from RegistryClient
///      (current key first, then older grace-period keys).
///   2. Try each key in order; return Ok on the first successful verification.
///   3. If verification succeeds with a key that is NOT the newest, increment
///      `graceful_rotation_success` — the attestation was saved by grace-period logic.
///   4. If all keys fail, return `VerifyError::SignatureInvalid`.
///
/// # Crate integration
/// This file lives in `lumina-attestation`. Add to its Cargo.toml:
///   lumina-identity = { path = "../identity" }
/// and update the use paths below accordingly.
use std::sync::atomic::{AtomicU64, Ordering};
use lumina_identity::registry_client::RegistryClient;

/// Pluggable signature verifier — production uses Ed25519; tests use a stub.
pub trait SignatureVerifier: Send + Sync {
    /// Returns true if `signature` is a valid signature of `message` under `public_key`.
    fn verify(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> bool;
}

#[derive(Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// No keys found for the node at the requested epoch.
    NoKeysForEpoch,
    /// Every candidate key failed to verify the signature.
    SignatureInvalid,
}

pub struct AttestationVerifier<V: SignatureVerifier> {
    registry: RegistryClient,
    verifier: V,
    /// Count of verifications that succeeded only via a grace-period (old) key.
    pub graceful_rotation_success: AtomicU64,
}

impl<V: SignatureVerifier> AttestationVerifier<V> {
    pub fn new(registry: RegistryClient, verifier: V) -> Self {
        Self {
            registry,
            verifier,
            graceful_rotation_success: AtomicU64::new(0),
        }
    }

    /// Verify `signature` over `message` for `node_id` at `epoch`.
    ///
    /// Tries all keys valid at `epoch` (newest first).  If the winning key is
    /// not the most recently activated one, increments `graceful_rotation_success`.
    pub fn verify(
        &self,
        node_id: &str,
        message: &[u8],
        signature: &[u8],
        epoch: u64,
    ) -> Result<(), VerifyError> {
        let candidates = self.registry.keys_valid_at(node_id, epoch);
        if candidates.is_empty() {
            return Err(VerifyError::NoKeysForEpoch);
        }

        // newest-first; first element has the highest activation_epoch
        let newest_activation = candidates[0].activation_epoch;

        for key in &candidates {
            if self.verifier.verify(&key.public_key, message, signature) {
                if key.activation_epoch < newest_activation {
                    self.graceful_rotation_success.fetch_add(1, Ordering::Relaxed);
                }
                return Ok(());
            }
        }
        Err(VerifyError::SignatureInvalid)
    }

    pub fn graceful_rotation_success(&self) -> u64 {
        self.graceful_rotation_success.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumina_identity::key_rotation::{KeyEpoch, RotationOrchestrator};
    use lumina_identity::registry_client::RegistryClient;

    /// Stub verifier: signature is valid iff it equals the public key bytes.
    struct StubVerifier;
    impl SignatureVerifier for StubVerifier {
        fn verify(&self, public_key: &[u8], _message: &[u8], signature: &[u8]) -> bool {
            signature == public_key
        }
    }

    fn make_verifier(registry: RegistryClient) -> AttestationVerifier<StubVerifier> {
        AttestationVerifier::new(registry, StubVerifier)
    }

    /// Core scenario: key rotated mid-flight.
    ///
    /// Timeline:
    ///   epoch 5  — old key (vec![1]) active, expiry set to 9 after rotation
    ///   epoch 7  — new key (vec![2]) activates (rotation committed at epoch 5)
    ///   epoch 8  — in-flight attestation signed with old key arrives
    ///              → must be accepted (within grace window)
    ///   epoch 9  — old key expired → must be rejected
    #[test]
    fn old_key_attestation_accepted_within_grace_window() {
        let mut registry = RegistryClient::new();

        // Old key: activated at epoch 0, expires at epoch 9 (set by rotation)
        let mut old_key = KeyEpoch::new(vec![1], 0);
        old_key.expiry_epoch = 9;
        registry.publish("node-1", old_key);

        // New key: activates at epoch 7 (rotation committed at epoch 5, +2)
        registry.publish("node-1", KeyEpoch::new(vec![2], 7));

        let av = make_verifier(registry);

        // Attestation signed with old key (signature = vec![1]), arriving at epoch 8
        assert_eq!(
            av.verify("node-1", b"msg", &[1], 8),
            Ok(()),
            "old-key attestation must be accepted within grace window"
        );
        assert_eq!(av.graceful_rotation_success(), 1);
    }

    #[test]
    fn old_key_attestation_rejected_after_grace_expires() {
        let mut registry = RegistryClient::new();

        let mut old_key = KeyEpoch::new(vec![1], 0);
        old_key.expiry_epoch = 9;
        registry.publish("node-1", old_key);
        registry.publish("node-1", KeyEpoch::new(vec![2], 7));

        let av = make_verifier(registry);

        // epoch 9: old key expired
        assert_eq!(
            av.verify("node-1", b"msg", &[1], 9),
            Err(VerifyError::SignatureInvalid),
            "old-key attestation must be rejected after grace window"
        );
        assert_eq!(av.graceful_rotation_success(), 0);
    }

    #[test]
    fn current_key_attestation_always_succeeds() {
        let mut registry = RegistryClient::new();
        registry.publish("node-1", KeyEpoch::new(vec![2], 7));
        let av = make_verifier(registry);

        assert_eq!(av.verify("node-1", b"msg", &[2], 8), Ok(()));
        // Not a grace-period save — counter stays 0
        assert_eq!(av.graceful_rotation_success(), 0);
    }

    #[test]
    fn returns_no_keys_for_epoch_when_cache_empty() {
        let registry = RegistryClient::new();
        let av = make_verifier(registry);
        assert_eq!(
            av.verify("node-x", b"msg", &[1], 5),
            Err(VerifyError::NoKeysForEpoch)
        );
    }

    /// End-to-end: use RotationOrchestrator to derive epochs, then verify.
    #[test]
    fn orchestrator_derived_epochs_round_trip() {
        let mut orch = RotationOrchestrator::default();
        let mut registry = RegistryClient::new();

        let old_key_bytes = vec![10u8];
        let new_key_bytes = vec![20u8];

        // Seed the old key (epoch 0, no expiry yet)
        registry.publish("node-1", KeyEpoch::new(old_key_bytes.clone(), 0));

        // Rotate at epoch 3
        let (new_key, old_expiry) = orch.rotate("node-1", new_key_bytes.clone(), 3).unwrap();
        // Cap the old key's expiry
        registry.cap_expiry("node-1", 0, old_expiry);
        registry.publish("node-1", new_key.clone());

        let av = make_verifier(registry);

        // Within grace window (epoch = new_key.activation_epoch + 1 = 6)
        let grace_epoch = new_key.activation_epoch + 1;
        assert!(grace_epoch < old_expiry, "test epoch must be within grace");
        assert_eq!(av.verify("node-1", b"data", &old_key_bytes, grace_epoch), Ok(()));
        assert_eq!(av.graceful_rotation_success(), 1);

        // After grace (epoch = old_expiry)
        // Need a fresh verifier with same registry state — rebuild
        let mut registry2 = RegistryClient::new();
        let mut old = KeyEpoch::new(old_key_bytes.clone(), 0);
        old.expiry_epoch = old_expiry;
        registry2.publish("node-1", old);
        registry2.publish("node-1", new_key);
        let av2 = make_verifier(registry2);
        assert_eq!(
            av2.verify("node-1", b"data", &old_key_bytes, old_expiry),
            Err(VerifyError::SignatureInvalid)
        );
    }
}
