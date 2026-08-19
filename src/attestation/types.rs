use crate::crypto::types::{Hash, PublicKey};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitteeSet {
    pub signers: Vec<PublicKey>,
    pub threshold: u32,
    pub set_hash: Hash,
    pub committee_epoch: u64, // NEW: Monotonically increasing on every committee change
}

impl CommitteeSet {
    pub fn new(signers: Vec<PublicKey>, threshold: u32, set_hash: Hash) -> Self {
        Self {
            signers,
            threshold,
            set_hash,
            committee_epoch: 0, // starts at epoch 0
        }
    }

    pub fn advance_epoch(&mut self) {
        self.committee_epoch = self.committee_epoch.saturating_add(1);
    }
}
