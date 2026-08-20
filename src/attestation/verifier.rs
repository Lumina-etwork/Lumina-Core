#[path = "../identity/key-rotation.rs"]
pub mod key_rotation;
#[path = "../identity/registry-client.rs"]
pub mod registry_client;

use key_rotation::KeyEpoch;
use registry_client::RegistryClient;

pub static mut GRACEFUL_ROTATION_SUCCESS: u64 = 0;

pub fn verify_attestation(
    node_id: &str,
    key_used: &[u8],
    current_epoch: u64,
    registry: &RegistryClient,
) -> Result<(), String> {
    let keys = registry.get_keys(node_id).ok_or("Node not found")?;
    
    // Most recent key is the last one in the cache
    if let Some(latest) = keys.last() {
        if latest.key == key_used {
            return Ok(());
        }
    }
    
    // Grace period lookup: retry with keys from previous 2 epochs
    for old_key in keys.iter().rev().skip(1) {
        if current_epoch.saturating_sub(old_key.activation_epoch) <= 2 {
            if old_key.key == key_used {
                unsafe { GRACEFUL_ROTATION_SUCCESS += 1; }
                return Ok(());
            }
        }
    }
    
    Err("Verification failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate_key_mid_flight() {
        let mut registry = RegistryClient::new();
        let node_id = "node_1".to_string();
        
        let old_key = KeyEpoch::new(vec![1, 2, 3], 10);
        registry.update_key(node_id.clone(), old_key.clone());
        
        let new_key = KeyEpoch::new(vec![4, 5, 6], 11);
        registry.update_key(node_id.clone(), new_key.clone());
        
        // Submit attestation with old key, verify acceptance within grace window (current epoch 12)
        let result = verify_attestation(&node_id, &old_key.key, 12, &registry);
        assert!(result.is_ok(), "Should be accepted within grace window");
    }
}
