//! OpenTelemetry-aligned structured logging setup for the social service.

use tracing_subscriber::{fmt, EnvFilter};

/// OpenTelemetry semantic convention attribute for service name.
pub const SERVICE_NAME: &str = "service.name";
/// OpenTelemetry semantic convention attribute for service version.
pub const SERVICE_VERSION: &str = "service.version";
/// OpenTelemetry semantic convention attribute for deployment environment.
pub const DEPLOYMENT_ENVIRONMENT: &str = "deployment.environment.name";

/// Service identity attached to every structured log event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogResource {
    pub service_name: &'static str,
    pub service_version: &'static str,
    pub deployment_environment: String,
}

impl LogResource {
    pub fn from_env(service_name: &'static str, service_version: &'static str) -> Self {
        Self {
            service_name,
            service_version,
            deployment_environment: std::env::var("DEPLOYMENT_ENVIRONMENT")
                .or_else(|_| std::env::var("ENVIRONMENT"))
                .unwrap_or_else(|_| "development".to_string()),
        }
    }
}

/// Initialize non-blocking JSON logs with OpenTelemetry semantic resource fields.
pub fn init(service_name: &'static str, service_version: &'static str) -> LogResource {
    let resource = LogResource::from_env(service_name, service_version);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .json()
        .with_env_filter(filter)
        .flatten_event(true)
        .with_current_span(true)
        .with_span_list(true)
        .with_target(true)
        .init();

    tracing::info!(
        service.name = resource.service_name,
        service.version = resource.service_version,
        deployment.environment.name = %resource.deployment_environment,
        "structured logging initialized"
    );

    resource
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_resource_defaults_environment() {
        std::env::remove_var("DEPLOYMENT_ENVIRONMENT");
        std::env::remove_var("ENVIRONMENT");

        let resource = LogResource::from_env("social-backend", "1.2.3");

        assert_eq!(resource.service_name, "social-backend");
        assert_eq!(resource.service_version, "1.2.3");
        assert_eq!(resource.deployment_environment, "development");
    }
}
