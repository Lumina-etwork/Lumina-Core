use std::collections::HashMap;

const DEFAULT_MTLS_PORT: u16 = 9443;
const PROXY_PROTOCOL_PORT: u16 = 9444;
const HEALTH_CHECK_PORT: u16 = 9445;

#[derive(Clone, Debug)]
pub struct ServiceCertificate {
    pub service_id: String,
    pub cert_fingerprint: Vec<u8>,
    pub allowed_tenants: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ServiceEndpoint {
    pub service_name: String,
    pub mesh_port: u16,
    pub mtls_enabled: bool,
    pub health_check_path: String,
}

pub struct ServiceMesh {
    trusted_certs: HashMap<String, ServiceCertificate>,
    endpoints: HashMap<String, ServiceEndpoint>,
    pub mtls_enforced: bool,
}

impl ServiceMesh {
    pub fn new() -> Self {
        let mut endpoints = HashMap::new();
        endpoints.insert(
            "api-gateway".to_string(),
            ServiceEndpoint {
                service_name: "api-gateway".to_string(),
                mesh_port: DEFAULT_MTLS_PORT,
                mtls_enabled: true,
                health_check_path: "/health".to_string(),
            },
        );
        endpoints.insert(
            "relay".to_string(),
            ServiceEndpoint {
                service_name: "relay".to_string(),
                mesh_port: PROXY_PROTOCOL_PORT,
                mtls_enabled: true,
                health_check_path: "/health".to_string(),
            },
        );

        Self {
            trusted_certs: HashMap::new(),
            endpoints,
            mtls_enforced: true,
        }
    }

    pub fn register_service_certificate(
        &mut self,
        service_id: &str,
        cert_fingerprint: Vec<u8>,
        allowed_tenants: Vec<String>,
    ) {
        self.trusted_certs.insert(
            service_id.to_string(),
            ServiceCertificate {
                service_id: service_id.to_string(),
                cert_fingerprint,
                allowed_tenants,
            },
        );
    }

    pub fn verify_client_certificate(
        &self,
        tenant_id: &str,
        cert_data: &[u8],
    ) -> Result<(), MeshError> {
        if !self.mtls_enforced {
            return Ok(());
        }

        let cert_fingerprint = Self::fingerprint(cert_data);

        for cert in self.trusted_certs.values() {
            if cert.cert_fingerprint == cert_fingerprint
                && cert.allowed_tenants.iter().any(|t| t == tenant_id)
            {
                return Ok(());
            }
        }

        Err(MeshError::CertificateNotTrusted)
    }

    pub fn register_endpoint(&mut self, endpoint: ServiceEndpoint) {
        self.endpoints
            .insert(endpoint.service_name.clone(), endpoint);
    }

    pub fn get_endpoint(&self, service_name: &str) -> Option<&ServiceEndpoint> {
        self.endpoints.get(service_name)
    }

    pub fn list_endpoints(&self) -> Vec<&ServiceEndpoint> {
        self.endpoints.values().collect()
    }

    pub fn health_check_port(&self) -> u16 {
        HEALTH_CHECK_PORT
    }

    fn fingerprint(data: &[u8]) -> Vec<u8> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish().to_le_bytes().to_vec()
    }
}

#[derive(Debug)]
pub enum MeshError {
    CertificateNotTrusted,
    ServiceNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_trusted_certificate() {
        let mut mesh = ServiceMesh::new();
        mesh.register_service_certificate(
            "svc-1",
            vec![1, 2, 3, 4, 5, 6, 7, 8],
            vec!["tenant-a".to_string()],
        );

        let cert_data = vec![0u8; 16];
        let fp = ServiceMesh::fingerprint(&cert_data);

        mesh.register_service_certificate(
            "svc-2",
            fp,
            vec!["tenant-a".to_string()],
        );

        let result = mesh.verify_client_certificate("tenant-a", &cert_data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_untrusted_certificate() {
        let mesh = ServiceMesh::new();
        let result = mesh.verify_client_certificate("tenant-a", b"unknown-cert");
        assert!(matches!(result, Err(MeshError::CertificateNotTrusted)));
    }

    #[test]
    fn test_mtls_disabled_allows_all() {
        let mut mesh = ServiceMesh::new();
        mesh.mtls_enforced = false;
        let result = mesh.verify_client_certificate("tenant-a", b"any-cert");
        assert!(result.is_ok());
    }

    #[test]
    fn test_register_and_find_endpoint() {
        let mut mesh = ServiceMesh::new();
        let ep = ServiceEndpoint {
            service_name: "custom-svc".to_string(),
            mesh_port: 9999,
            mtls_enabled: true,
            health_check_path: "/ping".to_string(),
        };
        mesh.register_endpoint(ep);
        assert!(mesh.get_endpoint("custom-svc").is_some());
        assert_eq!(mesh.get_endpoint("custom-svc").unwrap().mesh_port, 9999);
    }

    #[test]
    fn test_reject_certificate_wrong_tenant() {
        let mut mesh = ServiceMesh::new();
        let fp = ServiceMesh::fingerprint(b"my-cert");
        mesh.register_service_certificate(
            "svc-1",
            fp,
            vec!["tenant-a".to_string()],
        );

        let result = mesh.verify_client_certificate("tenant-b", b"my-cert");
        assert!(matches!(result, Err(MeshError::CertificateNotTrusted)));
    }

    #[test]
    fn test_default_endpoints_exist() {
        let mesh = ServiceMesh::new();
        assert!(mesh.get_endpoint("api-gateway").is_some());
        assert!(mesh.get_endpoint("relay").is_some());
        assert_eq!(mesh.list_endpoints().len(), 2);
    }
}
