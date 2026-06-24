pub struct RegistryClient;

impl RegistryClient {
    pub fn new() -> Self { Self }

    pub fn update_credential(&self, _node_id: &str, _credential: &[u8]) -> Result<(), RegistryError> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum RegistryError {
    ConnectionFailed,
    UpdateRejected,
}
