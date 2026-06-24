use std::collections::HashMap;

pub struct TenantEntry { pub tenant_id: String, pub pool_id: String, pub max_sessions: usize }
pub struct TenantRegistry { tenants: HashMap<String, TenantEntry> }

impl TenantRegistry {
    pub fn new() -> Self { Self { tenants: HashMap::new() } }
    pub fn register(&mut self, tid: &str, pid: &str, max: usize) {
        self.tenants.insert(tid.to_string(), TenantEntry { tenant_id: tid.to_string(), pool_id: pid.to_string(), max_sessions: max });
    }
    pub fn lookup(&self, tid: &str) -> Option<&TenantEntry> { self.tenants.get(tid) }
    pub fn verify_tenant(&self, tid: &str) -> bool { self.tenants.contains_key(tid) }
}
