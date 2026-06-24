use super::session_cache::SessionCache;
use super::sni_multiplexer::SniMultiplexer;

pub struct TlsHandshake { cache: SessionCache }

impl TlsHandshake {
    pub fn new() -> Self { Self { cache: SessionCache::new() } }
    pub fn handle_client_hello(&mut self, tenant_id: &str, original_sni: &str) -> Result<Vec<u8>, HandshakeError> {
        if let Some(s) = self.cache.lookup(tenant_id, original_sni) {
            if s.tenant_id == tenant_id { return Ok(s.session_data.clone()); }
            return Err(HandshakeError::TenantMismatch);
        }
        let data = vec![0u8; 32];
        self.cache.store(tenant_id, original_sni, data.clone(), None).map_err(|_| HandshakeError::CacheFull)?;
        Ok(data)
    }
    pub fn verify_session_resumption(&mut self, rewritten: &str, tenant: &str) -> Result<bool, HandshakeError> {
        if let Some((tid, sni)) = SniMultiplexer::parse_rewritten_sni(rewritten) {
            if tid != tenant { return Err(HandshakeError::TenantMismatch); }
            return Ok(self.cache.lookup(&tid, &sni).is_some());
        }
        Err(HandshakeError::InvalidSni)
    }
}

#[derive(Debug)]
pub enum HandshakeError { TenantMismatch, CacheFull, InvalidSni, SessionExpired }
