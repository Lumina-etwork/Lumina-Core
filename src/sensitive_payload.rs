//! Field-level protection for sensitive payload data.
//!
//! The module intentionally keeps encryption at the payload edge: callers mark
//! the fields that must remain opaque to intermediaries, encrypt those fields
//! for the recipient, and leave the rest of the payload routable.  Envelopes
//! include authenticated metadata so services can reject tampering before any
//! decrypted value is used.

use alloc::{string::String, vec::Vec};
use blake2::{Blake2s256, Digest};

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const TAG_LEN: usize = 32;
const DOMAIN_KEY: &[u8] = b"lumina:sensitive-payload:v1:key";
const DOMAIN_STREAM: &[u8] = b"lumina:sensitive-payload:v1:stream";
const DOMAIN_TAG: &[u8] = b"lumina:sensitive-payload:v1:tag";

/// Classification used by routing and audit code to decide whether a field
/// must be encrypted before it leaves the producer trust boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldSensitivity {
    Public,
    Sensitive,
}

/// Metadata that is authenticated with every encrypted field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldContext {
    pub service: String,
    pub payload_type: String,
    pub field_path: String,
    pub recipient_id: String,
}

impl FieldContext {
    pub fn new(service: &str, payload_type: &str, field_path: &str, recipient_id: &str) -> Self {
        Self {
            service: String::from(service),
            payload_type: String::from(payload_type),
            field_path: String::from(field_path),
            recipient_id: String::from(recipient_id),
        }
    }

    fn absorb(&self, hasher: &mut Blake2s256) {
        absorb_bytes(hasher, self.service.as_bytes());
        absorb_bytes(hasher, self.payload_type.as_bytes());
        absorb_bytes(hasher, self.field_path.as_bytes());
        absorb_bytes(hasher, self.recipient_id.as_bytes());
    }
}

/// Encrypted representation of one sensitive payload field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedField {
    pub key_id: String,
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
    pub tag: [u8; TAG_LEN],
}

/// Errors returned by field encryption and authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadCryptoError {
    InvalidKeyMaterial,
    AuthenticationFailed,
}

/// Derive the recipient-specific content key used for a sensitive field.
///
/// In production the shared secret should come from an E2E key agreement layer
/// or KMS-wrapped data key that is only available to endpoints.  The derived
/// key binds that secret to the field context to prevent cross-field replay.
pub fn derive_field_key(
    shared_secret: &[u8],
    context: &FieldContext,
) -> Result<[u8; KEY_LEN], PayloadCryptoError> {
    if shared_secret.len() < KEY_LEN {
        return Err(PayloadCryptoError::InvalidKeyMaterial);
    }

    let mut hasher = Blake2s256::new();
    hasher.update(DOMAIN_KEY);
    absorb_bytes(&mut hasher, shared_secret);
    context.absorb(&mut hasher);
    Ok(hasher.finalize().into())
}

/// Encrypt and authenticate one sensitive field.
pub fn encrypt_field(
    key_id: &str,
    shared_secret: &[u8],
    nonce: [u8; NONCE_LEN],
    context: &FieldContext,
    plaintext: &[u8],
) -> Result<EncryptedField, PayloadCryptoError> {
    let key = derive_field_key(shared_secret, context)?;
    let ciphertext = xor_keystream(&key, &nonce, context, plaintext);
    let tag = authentication_tag(&key, &nonce, context, &ciphertext);

    Ok(EncryptedField {
        key_id: String::from(key_id),
        nonce,
        ciphertext,
        tag,
    })
}

/// Authenticate and decrypt one sensitive field.
pub fn decrypt_field(
    shared_secret: &[u8],
    context: &FieldContext,
    encrypted: &EncryptedField,
) -> Result<Vec<u8>, PayloadCryptoError> {
    let key = derive_field_key(shared_secret, context)?;
    let expected = authentication_tag(&key, &encrypted.nonce, context, &encrypted.ciphertext);
    if !constant_time_eq(&expected, &encrypted.tag) {
        return Err(PayloadCryptoError::AuthenticationFailed);
    }

    Ok(xor_keystream(
        &key,
        &encrypted.nonce,
        context,
        &encrypted.ciphertext,
    ))
}

fn xor_keystream(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    context: &FieldContext,
    input: &[u8],
) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len());
    let mut counter = 0u64;

    while output.len() < input.len() {
        let mut hasher = Blake2s256::new();
        hasher.update(DOMAIN_STREAM);
        hasher.update(key);
        hasher.update(nonce);
        context.absorb(&mut hasher);
        hasher.update(counter.to_le_bytes());
        let block: [u8; KEY_LEN] = hasher.finalize().into();

        for byte in block {
            if output.len() == input.len() {
                break;
            }
            output.push(input[output.len()] ^ byte);
        }
        counter += 1;
    }

    output
}

fn authentication_tag(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    context: &FieldContext,
    ciphertext: &[u8],
) -> [u8; TAG_LEN] {
    let mut hasher = Blake2s256::new();
    hasher.update(DOMAIN_TAG);
    hasher.update(key);
    hasher.update(nonce);
    context.absorb(&mut hasher);
    absorb_bytes(&mut hasher, ciphertext);
    hasher.finalize().into()
}

fn absorb_bytes(hasher: &mut Blake2s256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn constant_time_eq(left: &[u8; TAG_LEN], right: &[u8; TAG_LEN]) -> bool {
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8; 32] = b"0123456789abcdef0123456789abcdef";
    const NONCE: [u8; 24] = [7; 24];

    fn context() -> FieldContext {
        FieldContext::new("payments", "TransferRequest", "card.pan", "user-123")
    }

    #[test]
    fn sensitive_field_round_trips() {
        let plaintext = b"4111111111111111";
        let encrypted =
            encrypt_field("recipient-key-v3", SECRET, NONCE, &context(), plaintext).unwrap();

        assert_ne!(encrypted.ciphertext, plaintext);
        let decrypted = decrypt_field(SECRET, &context(), &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let mut encrypted =
            encrypt_field("recipient-key-v3", SECRET, NONCE, &context(), b"secret").unwrap();
        encrypted.ciphertext[0] ^= 0x80;

        assert_eq!(
            decrypt_field(SECRET, &context(), &encrypted),
            Err(PayloadCryptoError::AuthenticationFailed)
        );
    }

    #[test]
    fn context_replay_is_rejected() {
        let encrypted =
            encrypt_field("recipient-key-v3", SECRET, NONCE, &context(), b"secret").unwrap();
        let replay_context = FieldContext::new("payments", "TransferRequest", "ssn", "user-123");

        assert_eq!(
            decrypt_field(SECRET, &replay_context, &encrypted),
            Err(PayloadCryptoError::AuthenticationFailed)
        );
    }

    #[test]
    fn short_shared_secret_is_rejected() {
        assert_eq!(
            encrypt_field("recipient-key-v3", b"short", NONCE, &context(), b"secret"),
            Err(PayloadCryptoError::InvalidKeyMaterial)
        );
    }
}
