use chrono::Utc;
use lumina_audit::{AuditChain, AuditEntry};

/// Lightweight audit trail wrapper for the core engine.
#[derive(Debug)]
pub struct CoreAuditTrail {
    chain: AuditChain,
}

impl Default for CoreAuditTrail {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreAuditTrail {
    /// Create a new audit trail with an optional anchor hash.
    pub fn new() -> Self {
        Self {
            chain: AuditChain::new(None),
        }
    }

    /// Record a new audit event for the core engine.
    pub fn record(&mut self, service: &str, action: &str, payload: &[u8]) -> &AuditEntry {
        let timestamp_ms = Utc::now().timestamp_millis() as u64;
        self.chain.append(service, action, payload, timestamp_ms)
    }

    /// Verify the integrity of the recorded audit trail.
    pub fn verify(&self) -> Result<[u8; 32], lumina_audit::AuditError> {
        self.chain.verify()
    }

    /// Current chain head hash.
    pub fn root_hash(&self) -> [u8; 32] {
        self.chain.current_root_hash()
    }

    /// Return the underlying entries for observability.
    pub fn entries(&self) -> &[AuditEntry] {
        &self.chain.entries
    }

    /// Return a mutable reference to the underlying entries for testing or repair.
    pub fn entries_mut(&mut self) -> &mut [AuditEntry] {
        &mut self.chain.entries
    }
}
