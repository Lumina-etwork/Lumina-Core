use crate::attestation::relay_ticket::RelayTicket;
use crate::net::relay::endpoint_cache::{CacheError, EndpointCache};
use ed25519_dalek::SigningKey;
use thiserror::Error;

/// STUN binding request with authentication context.
#[derive(Debug, Clone)]
pub struct StunBindingRequest {
    /// The target peer that the client wants to reach
    pub target_id: String,
    /// The relay node that should serve this binding
    pub relay_id: String,
}

impl StunBindingRequest {
    pub fn new(target_id: &str, relay_id: &str) -> Self {
        Self {
            target_id: target_id.to_string(),
            relay_id: relay_id.to_string(),
        }
    }
}

/// STUN binding response with attached ticket.
#[derive(Debug, Clone)]
pub struct StunBindingResponse {
    /// The relay endpoint address (e.g., "turn://relay_a.example.com:3478")
    pub relay_endpoint: String,
    /// The signed ticket authenticating this binding
    pub ticket: RelayTicket,
}

/// Errors from STUN binding operations.
#[derive(Debug, Error)]
pub enum StunBindError {
    #[error("relay not found in registry")]
    RelayNotFound,
    #[error("cache error: {0}")]
    Cache(#[from] CacheError),
    #[error("ticket verification failed during binding")]
    TicketVerificationFailed,
}

/// Handler for STUN binding requests.
/// Generates tickets, validates responses, and forwards to the cache.
pub struct StunBindHandler {
    /// Current global epoch counter (monotonically increasing)
    current_epoch: u64,
    /// Default ticket TTL in seconds
    default_ticket_ttl: u64,
}

impl StunBindHandler {
    pub fn new(default_ticket_ttl: u64) -> Self {
        Self {
            current_epoch: 0,
            default_ticket_ttl,
        }
    }

    /// Process an incoming STUN binding request.
    /// The relay generates a signed ticket and creates a binding response.
    pub fn handle_request(
        &mut self,
        request: &StunBindingRequest,
        relay_signing_key: &SigningKey,
    ) -> StunBindingResponse {
        self.current_epoch += 1;

        let ticket = RelayTicket::new(
            relay_signing_key,
            &request.relay_id,
            &request.target_id,
            self.current_epoch,
            self.default_ticket_ttl,
        );

        StunBindingResponse {
            relay_endpoint: format!("turn://{}.example.com:3478", request.relay_id),
            ticket,
        }
    }

    /// Validate an incoming STUN binding response before forwarding to cache.
    /// Returns Ok if the ticket is valid and the binding should be cached.
    /// Returns Err if the ticket is invalid — triggers poison-penalty.
    pub fn validate_response(
        &mut self,
        response: &StunBindingResponse,
        cache: &mut EndpointCache,
    ) -> Result<(), StunBindError> {
        // Try to put the endpoint in cache — this validates the ticket
        let result = cache.put_endpoint(&response.ticket.target_id, response.ticket.clone());

        match result {
            Ok(()) => Ok(()),
            Err(CacheError::TicketInvalid(_)) => {
                // Report poison attempt
                cache.report_poison_attempt(&response.ticket.relay_id);
                Err(StunBindError::TicketVerificationFailed)
            }
            Err(CacheError::PeerBlacklisted) => Err(StunBindError::TicketVerificationFailed),
            Err(e) => Err(StunBindError::Cache(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::relay_ticket::generate_relay_keypair;
    use crate::net::relay::endpoint_cache::CacheConfig;
    use crate::net::relay::relay_registry::RelayRegistry;

    #[test]
    fn test_handle_request_generates_ticket() {
        let (sk, vk) = generate_relay_keypair();
        let mut handler = StunBindHandler::new(300);

        let request = StunBindingRequest::new("peer_123", "relay_alpha");

        let response = handler.handle_request(&request, &sk);
        assert!(response.ticket.verify(&vk, 0).is_ok());
        assert_eq!(response.ticket.target_id, "peer_123");
        assert_eq!(response.ticket.relay_id, "relay_alpha");
    }

    #[test]
    fn test_validate_good_response() {
        let (sk, vk) = generate_relay_keypair();
        let mut handler = StunBindHandler::new(300);
        let mut cache = EndpointCache::new(CacheConfig::default());
        let mut registry = RelayRegistry::new();

        registry.register_relay("relay_alpha", &vk);
        registry.sync_to_cache(&mut cache);

        let request = StunBindingRequest::new("peer_123", "relay_alpha");
        let response = handler.handle_request(&request, &sk);

        let result = handler.validate_response(&response, &mut cache);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_bad_response_reports_poison() {
        let (sk, vk) = generate_relay_keypair();
        let mut handler = StunBindHandler::new(300);
        let mut cache = EndpointCache::new(CacheConfig {
            poison_threshold: 2,
            penalty_window_secs: 60,
            ..Default::default()
        });
        let mut registry = RelayRegistry::new();

        registry.register_relay("relay_bad", &vk);
        registry.sync_to_cache(&mut cache);

        // Create a forged response (wrong target)
        let request = StunBindingRequest::new("peer_123", "relay_bad");
        let mut response = handler.handle_request(&request, &sk);
        response.ticket.target_id = "tampered_target".to_string();

        let result = handler.validate_response(&response, &mut cache);
        assert!(result.is_err());

        // After 2 poison attempts, should be blacklisted
        cache.report_poison_attempt("relay_bad");
        assert!(cache.is_blacklisted("relay_bad"));
    }
}