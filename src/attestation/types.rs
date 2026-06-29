
//! Core types for proof-of-connectivity protocol.

use alloc::string::String;
use alloc::vec::Vec;

/// A challenge issued to a node to prove connectivity.
///
/// Contains a nonce scoped to an epoch to prevent replay attacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Challenge {
    /// Epoch ID for which this challenge is valid.
    pub epoch_id: u32,
    /// Node ID this challenge is issued to.
    pub node_id: String,
    /// 256-bit nonce generated using BLAKE2b(epoch_id || seed || node_id).
    pub nonce: [u8; 32],
    /// Timestamp when the challenge was issued (for timeout checks).
    pub issued_at: u64,
}

/// Response from a node to a challenge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeResponse {
    /// The challenge this response is for.
    pub challenge: Challenge,
    /// Signature over the challenge nonce.
    pub signature: Vec<u8>,
}

/// Errors that can occur during proof-of-connectivity operations.
#[derive(Debug, PartialEq, Eq)]
pub enum PocError {
    /// Challenge epoch is too old (>2 epochs behind current).
    EpochTooOld,
    /// Nonce has already been used for this node and epoch.
    NonceAlreadyUsed,
    /// Signature is invalid.
    SignatureInvalid,
    /// Challenge has timed out.
    ChallengeTimedOut,
    /// Node has exceeded max failed challenges and is blacklisted.
    NodeBlacklisted,
}
