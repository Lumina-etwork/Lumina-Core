#[path = "../../pool/tenant-registry.rs"]
pub mod tenant_registry;
use tenant_registry::TenantId;

pub fn rewrite_sni(tenant_id: &TenantId, original_sni: &str) -> String {
    format!("{}.{}", tenant_id.hex(), original_sni)
}
