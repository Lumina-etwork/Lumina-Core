#[path = "../../pool/tenant-registry.rs"]
pub mod tenant_registry;
use tenant_registry::TenantId;

pub fn verify_and_extract_sni(authenticated_tenant: &TenantId, rewritten_sni: &str) -> Result<String, &'static str> {
    let prefix = format!("{}.", authenticated_tenant.hex());
    if rewritten_sni.starts_with(&prefix) {
        Ok(rewritten_sni[prefix.len()..].to_string())
    } else {
        Err("Tenant ID prefix mismatch. Possible unauthorized access attempt.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[path = "session-cache.rs"]
    mod session_cache;
    use session_cache::SessionCache;

    #[test]
    fn test_session_isolation() {
        let tenant_a = TenantId("tenant_a".to_string());
        let tenant_b = TenantId("tenant_b".to_string());
        let base_sni = "example.com";
        
        let mut cache = SessionCache::new();
        
        cache.put(&tenant_a, base_sni, vec![1, 2, 3]);
        
        // Tenant B tries to get it with their ID, should fail
        assert!(cache.get(&tenant_b, base_sni).is_none());
        
        // Track the collision
        cache.increment_collision_attempts();
        assert_eq!(cache.session_cache_collision_attempts, 1);
        
        // Tenant A should succeed in resuming
        assert!(cache.get(&tenant_a, base_sni).is_some());
    }
}
