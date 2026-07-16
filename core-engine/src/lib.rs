//! Core Engine — STUN/TURN relay and attestation primitives
//!
//! Provides authenticated relay endpoint caching with poison-penalty
//! detection to prevent cache poisoning via malicious peer relays.

pub mod attestation;
pub mod audit;
pub mod net;

/// Opaque peer/node identifier (base64-encoded Ed25519 public key)
pub type PeerId = String;
