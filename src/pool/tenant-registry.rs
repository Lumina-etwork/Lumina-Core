#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantId(pub String);

impl TenantId {
    pub fn hex(&self) -> String {
        self.0.as_bytes().iter().map(|b| format!("{:02x}", b)).collect()
    }
}
