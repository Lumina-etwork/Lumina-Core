#[path = "../../pool/tenant-registry.rs"]
pub mod tenant_registry;
use tenant_registry::TenantId;

pub fn rewrite_sni(tenant_id: &TenantId, original_sni: &str) -> String {
    format!("{}.{}", tenant_id.hex(), original_sni)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewrite_sni() {
        let tenant = TenantId("tenant-abc".to_string());
        let original_sni = "node1.lumina.network";
        let rewritten = rewrite_sni(&tenant, original_sni);
        let expected_prefix = format!("{}.", tenant.hex());
        assert!(rewritten.starts_with(&expected_prefix));
        assert_eq!(rewritten, format!("{}{}", expected_prefix, original_sni));
    }
}

