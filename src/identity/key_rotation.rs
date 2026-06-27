/// Key rotation orchestration.
///
/// Invariants enforced:
///   - Max 1 rotation per node per epoch.
///   - New key activates 2 epochs after the rotation is committed
///     (activation_epoch = rotation_epoch + 2).
///   - Old key is valid through activation_epoch + 2 (3-epoch grace window).
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// The grace period: old key remains valid for this many epochs after a new
/// key's activation epoch.
pub const GRACE_EPOCHS: u64 = 2;

/// Tags a public key with its validity window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEpoch {
    pub public_key: Vec<u8>,
    /// Epoch at which this key becomes the authoritative key.
    pub activation_epoch: u64,
    /// Epoch after which this key is no longer valid (exclusive).
    /// Set to u64::MAX for keys that have not been superseded.
    pub expiry_epoch: u64,
}

impl KeyEpoch {
    pub fn new(public_key: Vec<u8>, activation_epoch: u64) -> Self {
        Self {
            public_key,
            activation_epoch,
            expiry_epoch: u64::MAX,
        }
    }

    /// Returns true if this key is valid at `epoch` (within grace window).
    pub fn is_valid_at(&self, epoch: u64) -> bool {
        epoch >= self.activation_epoch && epoch < self.expiry_epoch
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RotationError {
    /// A rotation already occurred in this epoch for this node.
    AlreadyRotatedThisEpoch,
}

/// Orchestrates key rotations and produces KeyEpoch records.
#[derive(Default)]
pub struct RotationOrchestrator {
    /// Last epoch a rotation was committed per node.
    last_rotation_epoch: BTreeMap<String, u64>,
}

impl RotationOrchestrator {
    /// Commit a rotation for `node_id` at `current_epoch`.
    ///
    /// Returns the new `KeyEpoch` whose `activation_epoch = current_epoch + 2`
    /// and the expiry epoch for the *old* key (`activation_epoch + GRACE_EPOCHS`).
    pub fn rotate(
        &mut self,
        node_id: &str,
        new_public_key: Vec<u8>,
        current_epoch: u64,
    ) -> Result<(KeyEpoch, u64), RotationError> {
        if let Some(&last) = self.last_rotation_epoch.get(node_id) {
            if last == current_epoch {
                return Err(RotationError::AlreadyRotatedThisEpoch);
            }
        }
        self.last_rotation_epoch.insert(node_id.to_string(), current_epoch);

        let activation_epoch = current_epoch + 2;
        // Old key expires when the new key activates plus the grace window.
        let old_key_expiry = activation_epoch + GRACE_EPOCHS;

        let new_key = KeyEpoch::new(new_public_key, activation_epoch);
        Ok((new_key, old_key_expiry))
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec;
    use super::*;

    #[test]
    fn rotation_sets_activation_two_epochs_ahead() {
        let mut orch = RotationOrchestrator::default();
        let (key, _) = orch.rotate("node-1", vec![1, 2, 3], 5).unwrap();
        assert_eq!(key.activation_epoch, 7);
    }

    #[test]
    fn old_key_expiry_covers_grace_window() {
        let mut orch = RotationOrchestrator::default();
        let (key, old_expiry) = orch.rotate("node-1", vec![1], 5).unwrap();
        // new key activates at 7; old key expires at 7+2=9
        assert_eq!(old_expiry, key.activation_epoch + GRACE_EPOCHS);
    }

    #[test]
    fn only_one_rotation_per_epoch() {
        let mut orch = RotationOrchestrator::default();
        orch.rotate("node-1", vec![1], 5).unwrap();
        assert_eq!(
            orch.rotate("node-1", vec![2], 5),
            Err(RotationError::AlreadyRotatedThisEpoch)
        );
    }

    #[test]
    fn rotation_allowed_in_next_epoch() {
        let mut orch = RotationOrchestrator::default();
        orch.rotate("node-1", vec![1], 5).unwrap();
        assert!(orch.rotate("node-1", vec![2], 6).is_ok());
    }
}
