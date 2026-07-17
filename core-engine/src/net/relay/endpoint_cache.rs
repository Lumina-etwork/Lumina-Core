use std::collections::HashMap;
use std::time::{Duration, Instant};
use thiserror::Error;

use crate::attestation::relay_ticket::{RelayTicket, TicketError};
use ed25519_dalek::VerifyingKey;

/// Configuration for the endpoint cache.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum seconds a cache entry lives (60–600, default 300)
    pub entry_ttl_secs: u64,
    /// Maximum cached endpoints per peer (default 16)
    pub max_endpoints_per_peer: usize,
    /// Poison threshold: >N incorrect relay claims within `penalty_window`
    /// triggers blacklist (default 5)
    pub poison_threshold: u32,
    /// Window for poison detection in seconds (default 60)
    pub penalty_window_secs: u64,
    /// Total max cache entries (default 10_000)
    pub max_cache_entries: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            entry_ttl_secs: 300,
            max_endpoints_per_peer: 16,
            poison_threshold: 5,
            penalty_window_secs: 60,
            max_cache_entries: 10_000,
        }
    }
}

/// A cached relay endpoint mapping.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The relay node claiming to serve this target peer
    pub relay_id: String,
    /// When this entry was inserted
    pub inserted_at: Instant,
    /// TTL for this entry
    pub ttl: Duration,
    /// The ticket that authorized this entry
    pub ticket: RelayTicket,
}

impl CacheEntry {
    pub fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }
}

/// Per-peer poison penalty tracker.
#[derive(Debug)]
struct PeerPenalty {
    /// Count of failed verification attempts within the current window
    failed_attempts: u32,
    /// When the current penalty window started
    window_start: Instant,
    /// Whether this peer is blacklisted
    blacklisted: bool,
}

impl Default for PeerPenalty {
    fn default() -> Self {
        Self {
            failed_attempts: 0,
            window_start: Instant::now(),
            blacklisted: false,
        }
    }
}

/// Errors from cache operations.
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("ticket verification failed: {0}")]
    TicketInvalid(#[from] TicketError),
    #[error("target_id mismatch: ticket target does not match cache key")]
    TargetMismatch,
    #[error("relay peer is blacklisted due to poison attempts")]
    PeerBlacklisted,
    #[error("cache capacity exceeded")]
    CacheFull,
}

/// Endpoint cache with ticket verification and poison-penalty detection.
pub struct EndpointCache {
    /// Cache key: target_peer_id -> list of cache entries
    entries: HashMap<String, Vec<CacheEntry>>,
    /// Per-relay penalty tracking
    penalties: HashMap<String, PeerPenalty>,
    /// Relay public keys (populated from relay-registry)
    relay_keys: HashMap<String, VerifyingKey>,
    /// Per-relay last-seen epoch (prevents ticket replay)
    relay_epochs: HashMap<String, u64>,
    config: CacheConfig,
}

impl EndpointCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            entries: HashMap::new(),
            penalties: HashMap::new(),
            relay_keys: HashMap::new(),
            relay_epochs: HashMap::new(),
            config,
        }
    }

    /// Register a relay's public key (called from relay-registry on join/update).
    pub fn register_relay(&mut self, relay_id: String, public_key: VerifyingKey) {
        self.relay_epochs.entry(relay_id.clone()).or_insert(0);
        self.relay_keys.insert(relay_id, public_key);
    }

    /// Remove a relay's registration (called on relay leave/ban).
    pub fn unregister_relay(&mut self, relay_id: &str) {
        self.relay_keys.remove(relay_id);
        self.relay_epochs.remove(relay_id);
        self.entries.retain(|_, vec| {
            vec.retain(|e| e.relay_id != relay_id);
            !vec.is_empty()
        });
    }

    /// Attempt to PUT an endpoint mapping. Validates the ticket before inserting.
    pub fn put_endpoint(&mut self, target_id: &str, ticket: RelayTicket) -> Result<(), CacheError> {
        // 1. Check blacklist
        if self.is_blacklisted(&ticket.relay_id) {
            return Err(CacheError::PeerBlacklisted);
        }

        // 2. Target must match cache key
        if ticket.target_id != target_id {
            return Err(CacheError::TargetMismatch);
        }

        // 3. Verify ticket signature + expiry + epoch
        let public_key = self
            .relay_keys
            .get(&ticket.relay_id)
            .ok_or(TicketError::KeyError(format!(
                "relay {} not registered",
                ticket.relay_id
            )))?;

        let min_epoch = self
            .relay_epochs
            .get(&ticket.relay_id)
            .copied()
            .unwrap_or(0);
        ticket.verify(public_key, min_epoch)?;

        // 4. Update relay epoch (prevent replay of same ticket)
        let epoch_entry = self
            .relay_epochs
            .entry(ticket.relay_id.clone())
            .or_insert(0);
        if ticket.epoch > *epoch_entry {
            *epoch_entry = ticket.epoch;
        }

        // 5. Check total capacity
        let total_entries: usize = self.entries.values().map(|v| v.len()).sum();
        if total_entries >= self.config.max_cache_entries {
            return Err(CacheError::CacheFull);
        }

        // 6. Insert into target's endpoint list
        let target_list = self.entries.entry(target_id.to_string()).or_default();
        if target_list.len() >= self.config.max_endpoints_per_peer {
            // Evict oldest entry
            target_list.sort_by_key(|e| e.inserted_at);
            target_list.remove(0);
        }
        target_list.push(CacheEntry {
            relay_id: ticket.relay_id.clone(),
            inserted_at: Instant::now(),
            ttl: Duration::from_secs(self.config.entry_ttl_secs),
            ticket,
        });

        Ok(())
    }

    /// Look up cached relay endpoints for a peer.
    pub fn get_endpoints(&mut self, target_id: &str) -> Vec<&CacheEntry> {
        if let Some(entries) = self.entries.get_mut(target_id) {
            // Evict expired
            entries.retain(|e| !e.is_expired());
            entries.iter().collect()
        } else {
            vec![]
        }
    }

    /// Report a failed verification attempt for a relay (called when
    /// a STUN binding response ticket fails verification).
    pub fn report_poison_attempt(&mut self, relay_id: &str) -> bool {
        let penalty = self.penalties.entry(relay_id.to_string()).or_default();

        // Reset window if expired
        if penalty.window_start.elapsed() > Duration::from_secs(self.config.penalty_window_secs) {
            penalty.window_start = Instant::now();
            penalty.failed_attempts = 0;
        }

        penalty.failed_attempts += 1;

        if penalty.failed_attempts >= self.config.poison_threshold {
            penalty.blacklisted = true;
            // Purge all entries from this relay
            for entries in self.entries.values_mut() {
                entries.retain(|e| e.relay_id != relay_id);
            }
            true // blacklisted
        } else {
            false
        }
    }

    /// Check if a relay is blacklisted.
    pub fn is_blacklisted(&self, relay_id: &str) -> bool {
        self.penalties
            .get(relay_id)
            .map(|p| p.blacklisted)
            .unwrap_or(false)
    }

    /// Get cache stats for monitoring.
    pub fn stats(&self) -> CacheStats {
        let total_entries = self.entries.values().map(|v| v.len()).sum();
        let blacklisted_count = self.penalties.values().filter(|p| p.blacklisted).count();
        CacheStats {
            total_entries,
            unique_peers: self.entries.len(),
            registered_relays: self.relay_keys.len(),
            blacklisted_relays: blacklisted_count,
        }
    }
}

/// Cache statistics snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub unique_peers: usize,
    pub registered_relays: usize,
    pub blacklisted_relays: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::relay_ticket::generate_relay_keypair;

    fn setup_cache() -> (EndpointCache, ed25519_dalek::SigningKey) {
        let (sk, vk) = generate_relay_keypair();
        let mut cache = EndpointCache::new(CacheConfig {
            poison_threshold: 3,
            penalty_window_secs: 60,
            ..Default::default()
        });
        cache.register_relay("relay_a".to_string(), vk);
        (cache, sk)
    }

    #[test]
    fn test_put_and_get_endpoint() {
        let (mut cache, sk) = setup_cache();
        let ticket = RelayTicket::new(&sk, "relay_a", "peer_1", 1, 300);
        assert!(cache.put_endpoint("peer_1", ticket).is_ok());

        let endpoints = cache.get_endpoints("peer_1");
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].relay_id, "relay_a");
    }

    #[test]
    fn test_target_mismatch_rejected() {
        let (mut cache, sk) = setup_cache();
        let ticket = RelayTicket::new(&sk, "relay_a", "peer_1", 1, 300);
        // Try to insert under wrong target
        let result = cache.put_endpoint("peer_WRONG", ticket);
        assert!(matches!(result, Err(CacheError::TargetMismatch)));
    }

    #[test]
    fn test_unregistered_relay_rejected() {
        let (cache_sk, _cache_vk) = generate_relay_keypair();
        let mut cache = EndpointCache::new(CacheConfig::default());
        // Don't register relay_b
        let ticket = RelayTicket::new(&cache_sk, "relay_b", "peer_1", 1, 300);
        let result = cache.put_endpoint("peer_1", ticket);
        assert!(result.is_err());
    }

    #[test]
    fn test_poison_blacklist_triggers() {
        let (mut cache, sk) = setup_cache();
        // Submit 3 bad tickets to trigger blacklist (threshold=3)
        for _ in 0..3 {
            let mut ticket = RelayTicket::new(&sk, "relay_a", "peer_1", 1, 300);
            ticket.target_id = "tampered".to_string(); // mismatch
                                                       // report manually (simulating stun_bind failure path)
            cache.report_poison_attempt("relay_a");
        }
        assert!(cache.is_blacklisted("relay_a"));
        // Now valid ticket should also be rejected
        let good_ticket = RelayTicket::new(&sk, "relay_a", "peer_1", 2, 300);
        assert!(matches!(
            cache.put_endpoint("peer_1", good_ticket),
            Err(CacheError::PeerBlacklisted)
        ));
    }

    #[test]
    fn test_max_endpoints_per_peer_eviction() {
        let (mut cache, sk) = setup_cache();
        // Insert 17 endpoints (max is 16)
        for i in 1..=17 {
            let ticket = RelayTicket::new(&sk, "relay_a", "peer_1", i, 300);
            cache.put_endpoint("peer_1", ticket).unwrap();
        }
        let endpoints = cache.get_endpoints("peer_1");
        assert!(endpoints.len() <= 16);
    }

    #[test]
    fn test_cache_full_error() {
        let (mut cache, sk) = setup_cache();
        // Override to tiny capacity
        cache.config.max_cache_entries = 2;
        cache
            .put_endpoint("p1", RelayTicket::new(&sk, "relay_a", "p1", 1, 300))
            .unwrap();
        cache
            .put_endpoint("p2", RelayTicket::new(&sk, "relay_a", "p2", 2, 300))
            .unwrap();
        let result = cache.put_endpoint("p3", RelayTicket::new(&sk, "relay_a", "p3", 3, 300));
        assert!(matches!(result, Err(CacheError::CacheFull)));
    }

    #[test]
    fn test_stats() {
        let (mut cache, sk) = setup_cache();
        cache
            .put_endpoint("peer_1", RelayTicket::new(&sk, "relay_a", "peer_1", 1, 300))
            .unwrap();
        let stats = cache.stats();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.unique_peers, 1);
        assert_eq!(stats.registered_relays, 1);
    }
}
