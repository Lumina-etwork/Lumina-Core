use ed25519_dalek::{Signer, Verifier, Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use chrono::Utc;
use rand::rngs::OsRng;
use thiserror::Error;

/// A signed ticket proving a relay node is authorized to advertise
/// an endpoint for a given target peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayTicket {
    /// Ed25519 public key of the relay node (base64)
    pub relay_id: String,
    /// Ed25519 public key of the target peer (base64)
    pub target_id: String,
    /// Monotonically increasing epoch counter (prevents replay)
    pub epoch: u64,
    /// Unix timestamp when this ticket expires
    pub expiry: u64,
    /// Ed25519 signature over (relay_id || target_id || epoch || expiry)
    pub signature: String, // hex-encoded
}

/// Errors that can occur during ticket operations
#[derive(Debug, Error)]
pub enum TicketError {
    #[error("signature verification failed")]
    InvalidSignature,
    #[error("ticket has expired")]
    Expired,
    #[error("epoch too old — possible replay attack")]
    ReplayDetected,
    #[error("Ed25519 key error: {0}")]
    KeyError(String),
}

impl RelayTicket {
    /// Create a new signed ticket.
    /// `signing_key` is the relay's Ed25519 private key.
    pub fn new(
        signing_key: &SigningKey,
        relay_id: &str,
        target_id: &str,
        current_epoch: u64,
        ttl_secs: u64,
    ) -> Self {
        let expiry = Utc::now().timestamp() as u64 + ttl_secs;
        let msg = Self::message_to_sign(relay_id, target_id, current_epoch, expiry);
        let sig = signing_key.sign(&msg);

        RelayTicket {
            relay_id: relay_id.to_string(),
            target_id: target_id.to_string(),
            epoch: current_epoch,
            expiry,
            signature: hex::encode(sig.to_bytes()),
        }
    }

    /// Verify the ticket against the relay's known public key.
    /// Also checks expiry and epoch freshness (`min_epoch` should be the
    /// last-seen epoch for this relay to prevent replay).
    pub fn verify(
        &self,
        relay_public_key: &VerifyingKey,
        min_epoch: u64,
    ) -> Result<(), TicketError> {
        // Check expiry
        let now = Utc::now().timestamp() as u64;
        if now > self.expiry {
            return Err(TicketError::Expired);
        }

        // Check epoch replay
        if self.epoch < min_epoch {
            return Err(TicketError::ReplayDetected);
        }

        // Verify signature
        let msg = Self::message_to_sign(&self.relay_id, &self.target_id, self.epoch, self.expiry);
        let sig_bytes = hex::decode(&self.signature)
            .map_err(|e| TicketError::KeyError(format!("hex decode: {}", e)))?;
        let sig = Signature::from_bytes(&sig_bytes.try_into()
            .map_err(|_| TicketError::KeyError("bad signature length".into()))?);

        relay_public_key.verify(&msg, &sig)
            .map_err(|_| TicketError::InvalidSignature)?;

        Ok(())
    }

    /// Build the canonical message to sign.
    fn message_to_sign(relay_id: &str, target_id: &str, epoch: u64, expiry: u64) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(relay_id.as_bytes());
        hasher.update(b"||");
        hasher.update(target_id.as_bytes());
        hasher.update(b"||");
        hasher.update(epoch.to_le_bytes());
        hasher.update(b"||");
        hasher.update(expiry.to_le_bytes());
        hasher.finalize().to_vec()
    }
}

/// Generate a new Ed25519 keypair for testing or initial setup.
pub fn generate_relay_keypair() -> (SigningKey, VerifyingKey) {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ticket_sign_and_verify() {
        let (sk, vk) = generate_relay_keypair();
        let relay_id = "relay_node_a";
        let target_id = "peer_12345";

        let ticket = RelayTicket::new(&sk, relay_id, target_id, 1, 300);
        assert!(ticket.verify(&vk, 0).is_ok());
    }

    #[test]
    fn test_expired_ticket_rejected() {
        let (sk, vk) = generate_relay_keypair();
        let mut ticket = RelayTicket::new(&sk, "relay", "peer", 1, 1);
        // Force expiry to the past
        ticket.expiry = Utc::now().timestamp() as u64 - 1;
        assert!(matches!(ticket.verify(&vk, 0), Err(TicketError::Expired)));
    }

    #[test]
    fn test_replay_attack_rejected() {
        let (sk, vk) = generate_relay_keypair();
        let ticket = RelayTicket::new(&sk, "relay", "peer", 1, 300);
        // min_epoch of 5 means epoch=1 is too old
        assert!(matches!(ticket.verify(&vk, 5), Err(TicketError::ReplayDetected)));
    }

    #[test]
    fn test_tampered_signature_rejected() {
        let (sk, vk) = generate_relay_keypair();
        let mut ticket = RelayTicket::new(&sk, "relay", "peer", 1, 300);
        // Tamper
        ticket.target_id = "attacker_peer".to_string();
        assert!(matches!(ticket.verify(&vk, 0), Err(TicketError::InvalidSignature)));
    }
}