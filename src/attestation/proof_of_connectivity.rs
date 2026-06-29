
//! Proof-of-connectivity protocol implementation with epoch scoping to prevent replay attacks.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use crate::attestation::types::{Challenge, ChallengeResponse, PocError};
use crate::attestation::nonce_generator::NonceGenerator;
use crate::attestation::nonce_cache::NonceCache;
use crate::attestation::verifier::SignatureVerifier;

/// Configuration for the proof-of-connectivity protocol.
pub struct PocConfig {
    /// Challenge timeout in seconds.
    pub challenge_timeout_secs: u64,
    /// Max failed challenges before a node is blacklisted.
    pub max_failed_challenges: u32,
    /// Duration a node stays blacklisted (in epochs).
    pub blacklist_duration_epochs: u32,
    /// Epoch length in seconds.
    pub epoch_length_secs: u64,
}

impl Default for PocConfig {
    fn default() -> Self {
        Self {
            challenge_timeout_secs: 5,
            max_failed_challenges: 3,
            blacklist_duration_epochs: 60,
            epoch_length_secs: 30,
        }
    }
}

/// Tracks state for failed challenges and blacklisting.
struct NodeState {
    failed_challenges: u32,
    blacklisted_until_epoch: u32,
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            failed_challenges: 0,
            blacklisted_until_epoch: 0,
        }
    }
}

/// Main proof-of-connectivity manager.
pub struct ProofOfConnectivity<V> {
    config: PocConfig,
    nonce_gen: NonceGenerator,
    nonce_cache: NonceCache,
    node_states: BTreeMap<String, NodeState>,
    signature_verifier: V,
}

impl<V: SignatureVerifier> ProofOfConnectivity<V> {
    /// Create a new ProofOfConnectivity manager.
    pub fn new(config: PocConfig, nonce_gen: NonceGenerator, signature_verifier: V) -> Self {
        Self {
            config,
            nonce_gen,
            nonce_cache: NonceCache::new(),
            node_states: BTreeMap::new(),
            signature_verifier,
        }
    }

    /// Generate a new challenge for a node.
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to challenge
    /// * `current_epoch` - The current epoch ID
    /// * `current_time` - Current timestamp (seconds since epoch)
    ///
    /// # Returns
    /// A new Challenge instance
    pub fn generate_challenge(&mut self, node_id: &str, current_epoch: u32, current_time: u64) -> Result<Challenge, PocError> {
        let state = self.node_states.entry(node_id.to_string()).or_default();
        if current_epoch < state.blacklisted_until_epoch {
            return Err(PocError::NodeBlacklisted);
        }

        let nonce = self.nonce_gen.generate(current_epoch, node_id);
        Ok(Challenge {
            epoch_id: current_epoch,
            node_id: node_id.to_string(),
            nonce,
            issued_at: current_time,
        })
    }

    /// Verify a challenge response from a node.
    ///
    /// # Arguments
    /// * `response` - The challenge response to verify
    /// * `current_epoch` - The current epoch ID
    /// * `current_time` - Current timestamp (seconds since epoch)
    /// * `public_key` - The node's public key
    ///
    /// # Returns
    /// Ok(()) if verification succeeds, Err(PocError) otherwise
    pub fn verify_response(&mut self, response: &ChallengeResponse, current_epoch: u32, current_time: u64, public_key: &[u8]) -> Result<(), PocError> {
        let challenge = &response.challenge;
        let node_id = &challenge.node_id;

        let state = self.node_states.entry(node_id.to_string()).or_default();
        if current_epoch < state.blacklisted_until_epoch {
            return Err(PocError::NodeBlacklisted);
        }

        // Check epoch validity
        if current_epoch >= 2 && challenge.epoch_id < current_epoch - 2 {
            self.record_failure(node_id, current_epoch);
            return Err(PocError::EpochTooOld);
        }

        // Check timeout
        if current_time > challenge.issued_at + self.config.challenge_timeout_secs {
            self.record_failure(node_id, current_epoch);
            return Err(PocError::ChallengeTimedOut);
        }

        // Check nonce reuse
        if self.nonce_cache.is_used(node_id, challenge.epoch_id, &challenge.nonce, current_epoch) {
            self.record_failure(node_id, current_epoch);
            return Err(PocError::NonceAlreadyUsed);
        }

        // Verify signature
        if !self.signature_verifier.verify(public_key, &challenge.nonce, &response.signature) {
            self.record_failure(node_id, current_epoch);
            return Err(PocError::SignatureInvalid);
        }

        // Mark nonce as used
        self.nonce_cache.mark_used(node_id, challenge.epoch_id, challenge.nonce, current_epoch);

        // Reset failure count on success
        state.failed_challenges = 0;

        Ok(())
    }

    fn record_failure(&mut self, node_id: &str, current_epoch: u32) {
        let state = self.node_states.entry(node_id.to_string()).or_default();
        state.failed_challenges += 1;
        if state.failed_challenges >= self.config.max_failed_challenges {
            state.blacklisted_until_epoch = current_epoch + self.config.blacklist_duration_epochs;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubVerifier;
    impl SignatureVerifier for StubVerifier {
        fn verify(&self, public_key: &[u8], _message: &[u8], signature: &[u8]) -> bool {
            signature == public_key
        }
    }

    #[test]
    fn generate_and_verify_challenge() {
        let config = PocConfig::default();
        let nonce_gen = NonceGenerator::new([1u8; 32]);
        let mut poc = ProofOfConnectivity::new(config, nonce_gen, StubVerifier);

        let challenge = poc.generate_challenge("node-1", 1, 1000).unwrap();
        let response = ChallengeResponse {
            challenge: challenge.clone(),
            signature: vec![2u8; 32],
        };

        let result = poc.verify_response(&response, 1, 1002, &[2u8; 32]);
        assert!(result.is_ok());
    }

    #[test]
    fn reject_old_epoch_challenge() {
        let config = PocConfig::default();
        let nonce_gen = NonceGenerator::new([1u8; 32]);
        let mut poc = ProofOfConnectivity::new(config, nonce_gen, StubVerifier);

        let challenge = poc.generate_challenge("node-1", 1, 1000).unwrap();
        let response = ChallengeResponse {
            challenge,
            signature: vec![2u8; 32],
        };

        let result = poc.verify_response(&response, 4, 1002, &[2u8; 32]);
        assert_eq!(result, Err(PocError::EpochTooOld));
    }
}
