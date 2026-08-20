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

#[path = "session-cache.rs"]
pub mod session_cache;
#[path = "sni-multiplexer.rs"]
pub mod sni_multiplexer;

#[cfg(test)]
mod tests {
    use super::*;
    use session_cache::SessionCache;
    use sni_multiplexer::rewrite_sni;

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

    #[test]
    fn test_handshake_sni_rewrite_and_extract_success() {
        let tenant = TenantId("tenant_123".to_string());
        let original_sni = "pool.lumina.network";

        let rewritten = rewrite_sni(&tenant, original_sni);
        let extracted = verify_and_extract_sni(&tenant, &rewritten);

        assert_eq!(extracted, Ok(original_sni.to_string()));
    }

    #[test]
    fn test_handshake_sni_extract_mismatched_tenant_fails() {
        let tenant_a = TenantId("tenant_a".to_string());
        let tenant_b = TenantId("tenant_b".to_string());
        let original_sni = "pool.lumina.network";

        // SNI rewritten for tenant A
        let rewritten_a = rewrite_sni(&tenant_a, original_sni);

        // Tenant B tries to authenticate with SNI rewritten for tenant A
        let result = verify_and_extract_sni(&tenant_b, &rewritten_a);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Tenant ID prefix mismatch. Possible unauthorized access attempt."
        );
    }
}
