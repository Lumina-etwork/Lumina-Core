use std::path::PathBuf;

use super::blacklist::BlacklistManager;
use super::rate_limiter::RateLimiterManager;

pub struct ConsensusMessage {
    pub sender: String,
    pub view_number: u64,
    pub payload: Vec<u8>,
    pub signature: Vec<u8>,
}

pub struct ValidatorSet {
    pub validators: std::collections::HashSet<String>,
}

impl ValidatorSet {
    pub fn contains(&self, sender: &str) -> bool {
        self.validators.contains(sender)
    }
}

pub struct Gatekeeper {
    current_view: u64,
    validator_set: ValidatorSet,
    rate_limiter: RateLimiterManager,
    blacklist: BlacklistManager,
    // Metrics
    pub invalid_messages_total: u64,
    pub rate_limited_senders_total: u64,
    pub blacklisted_senders_total: u64,
}

impl Gatekeeper {
    pub fn new(current_view: u64, validator_set: ValidatorSet, blacklist_path: PathBuf) -> Self {
        Self {
            current_view,
            validator_set,
            rate_limiter: RateLimiterManager::new(),
            blacklist: BlacklistManager::new(blacklist_path),
            invalid_messages_total: 0,
            rate_limited_senders_total: 0,
            blacklisted_senders_total: 0,
        }
    }

    pub fn set_view(&mut self, view: u64) {
        self.current_view = view;
    }

    pub fn set_validator_set(&mut self, set: ValidatorSet) {
        self.validator_set = set;
    }

    fn verify_signature(&self, _msg: &ConsensusMessage) -> bool {
        // Mock fast Ed25519 signature verification target < 5us
        // For chaos test and compilation purposes, this is a mock.
        true
    }

    pub fn process_message(&mut self, msg: ConsensusMessage) -> Option<ConsensusMessage> {
        if self.blacklist.is_blacklisted(&msg.sender) {
            return None;
        }

        if !self.rate_limiter.check(&msg.sender) {
            self.rate_limited_senders_total += 1;
            return None;
        }

        if msg.payload.len() > 1024 * 1024 {
            self.handle_invalid(&msg.sender);
            return None; // > 1MB
        }

        if !self.validator_set.contains(&msg.sender) {
            self.handle_invalid(&msg.sender);
            return None;
        }

        if msg.view_number < self.current_view {
            self.handle_invalid(&msg.sender);
            return None;
        }

        if !self.verify_signature(&msg) {
            self.handle_invalid(&msg.sender);
            return None;
        }

        // Valid, forward to consensus engine
        Some(msg)
    }

    fn handle_invalid(&mut self, sender: &str) {
        self.invalid_messages_total += 1;
        if self.blacklist.record_invalid(sender) {
            self.blacklisted_senders_total += 1;
        }
    }
}
