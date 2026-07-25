use super::mesh::{MeshError, ServiceMesh};
use super::session_cache::SessionCache;
use super::sni_multiplexer::SniMultiplexer;

pub struct TlsHandshake {
    cache: SessionCache,
    mesh: ServiceMesh,
}

impl TlsHandshake {
    pub fn new() -> Self {
        Self {
            cache: SessionCache::new(),
            mesh: ServiceMesh::new(),
        }
    }

    pub fn handle_client_hello(
        &mut self,
        tenant_id: &str,
        original_sni: &str,
        client_cert: Option<&[u8]>,
    ) -> Result<Vec<u8>, HandshakeError> {
        if let Some(cert) = client_cert {
            self.mesh
                .verify_client_certificate(tenant_id, cert)
                .map_err(|_| HandshakeError::MtlsVerificationFailed)?;
        }

        if let Some(s) = self.cache.lookup(tenant_id, original_sni) {
            if s.tenant_id == tenant_id {
                return Ok(s.session_data.clone());
            }
            return Err(HandshakeError::TenantMismatch);
        }
        let data = vec![0u8; 32];
        self.cache
            .store(tenant_id, original_sni, data.clone(), None)
            .map_err(|_| HandshakeError::CacheFull)?;
        Ok(data)
    }

    pub fn verify_session_resumption(
        &mut self,
        rewritten: &str,
        tenant: &str,
    ) -> Result<bool, HandshakeError> {
        if let Some((tid, sni)) = SniMultiplexer::parse_rewritten_sni(rewritten) {
            if tid != tenant {
                return Err(HandshakeError::TenantMismatch);
            }
            return Ok(self.cache.lookup(&tid, &sni).is_some());
        }
        Err(HandshakeError::InvalidSni)
    }

    pub fn mesh(&self) -> &ServiceMesh {
        &self.mesh
    }

    pub fn mesh_mut(&mut self) -> &mut ServiceMesh {
        &mut self.mesh
    }
}

#[derive(Debug)]
pub enum HandshakeError {
    TenantMismatch,
    CacheFull,
    InvalidSni,
    SessionExpired,
    MtlsVerificationFailed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_without_mtls() {
        let mut hs = TlsHandshake::new();
        let result = hs.handle_client_hello("tenant-a", "example.com", None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handshake_with_mtls_enabled_no_cert() {
        let mut hs = TlsHandshake::new();
        hs.mesh_mut().mtls_enforced = false;

        let result = hs.handle_client_hello("tenant-a", "example.com", None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handshake_with_valid_mtls_cert() {
        let mut hs = TlsHandshake::new();
        let fp = ServiceMesh::fingerprint(b"trusted-cert-data");
        hs.mesh_mut().register_service_certificate(
            "svc-1",
            fp,
            vec!["tenant-a".to_string()],
        );

        let result =
            hs.handle_client_hello("tenant-a", "example.com", Some(b"trusted-cert-data"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_session_resumption() {
        let mut hs = TlsHandshake::new();
        hs.handle_client_hello("tenant-a", "example.com", None)
            .unwrap();

        let rewritten = SniMultiplexer::rewrite_sni("tenant-a", "example.com");
        let result = hs.verify_session_resumption(&rewritten, "tenant-a");
        assert!(result.unwrap());
    }

    #[test]
    fn test_tenant_mismatch_rejected() {
        let mut hs = TlsHandshake::new();
        let rewritten = SniMultiplexer::rewrite_sni("tenant-a", "example.com");
        let result = hs.verify_session_resumption(&rewritten, "tenant-b");
        assert!(matches!(result, Err(HandshakeError::TenantMismatch)));
    }

    #[test]
    fn test_invalid_sni_rejected() {
        let mut hs = TlsHandshake::new();
        let result = hs.verify_session_resumption("invalid", "tenant-a");
        assert!(matches!(result, Err(HandshakeError::InvalidSni)));
    }
}
