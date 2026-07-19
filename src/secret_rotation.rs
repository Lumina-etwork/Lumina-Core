//! Secret rotation primitives for database credentials and API keys.
//!
//! The service keeps rotation decisions deterministic and allocation-light so
//! critical paths can evaluate active/standby material without network calls.
//! Integrations are expected to persist [`RotationPlan`] records in their
//! backing secret manager and publish the exposed metrics to Prometheus.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use blake2::{Blake2s256, Digest};

/// Default overlap window for blue-green secret rollouts.
pub const DEFAULT_OVERLAP_SECS: u64 = 300;
/// Minimum canary success rate in basis points (99.90%).
pub const DEFAULT_CANARY_SUCCESS_BPS: u16 = 9_990;
/// P99 latency guardrail for secret lookup/validation critical paths.
pub const CRITICAL_PATH_P99_BUDGET_MS: u64 = 100;

/// Distinguishes supported secret classes for policy and alerting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretKind {
    DatabaseCredential,
    ApiKey,
}

/// Rotation lifecycle used by blue-green deployment and canary analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotationPhase {
    Planned,
    Canary,
    Promoted,
    RolledBack,
    Retired,
}

/// Stable identifier and policy for a managed secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretDescriptor {
    pub service: String,
    pub name: String,
    pub kind: SecretKind,
    pub max_age_secs: u64,
}

impl SecretDescriptor {
    pub fn new(service: &str, name: &str, kind: SecretKind, max_age_secs: u64) -> Self {
        Self {
            service: service.to_string(),
            name: name.to_string(),
            kind,
            max_age_secs,
        }
    }

    fn key(&self) -> String {
        let mut key = self.service.clone();
        key.push(':');
        key.push_str(&self.name);
        key
    }
}

/// A versioned secret pointer. `material_hash` stores only a digest; plaintext
/// must remain in the external secret manager.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretVersion {
    pub version: u64,
    pub material_hash: [u8; 32],
    pub activated_at_secs: u64,
    pub expires_at_secs: u64,
}

impl SecretVersion {
    pub fn from_material(
        version: u64,
        material: &[u8],
        activated_at_secs: u64,
        ttl_secs: u64,
    ) -> Self {
        let mut hasher = Blake2s256::new();
        hasher.update(material);
        let digest = hasher.finalize();
        let mut material_hash = [0u8; 32];
        material_hash.copy_from_slice(&digest);
        Self {
            version,
            material_hash,
            activated_at_secs,
            expires_at_secs: activated_at_secs.saturating_add(ttl_secs),
        }
    }

    pub fn is_active_at(&self, now_secs: u64) -> bool {
        now_secs >= self.activated_at_secs && now_secs < self.expires_at_secs
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RotationPlan {
    pub descriptor: SecretDescriptor,
    pub current: SecretVersion,
    pub candidate: SecretVersion,
    pub phase: RotationPhase,
    pub canary_percent: u8,
    pub overlap_ends_at_secs: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CanarySample {
    pub requests: u64,
    pub successes: u64,
    pub p99_latency_ms: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RotationMetrics {
    pub rotations_started: u64,
    pub rotations_promoted: u64,
    pub rotations_rolled_back: u64,
    pub policy_violations: u64,
    pub active_secret_age_secs: u64,
    pub last_p99_latency_ms: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SecretRotationError {
    AlreadyManaged,
    UnknownSecret,
    CandidateVersionNotNewer,
    CanaryBelowThreshold,
    LatencyBudgetExceeded,
    OverlapWindowActive,
}

#[derive(Default)]
pub struct SecretRotationService {
    plans: BTreeMap<String, RotationPlan>,
    metrics: RotationMetrics,
}

impl SecretRotationService {
    pub fn metrics(&self) -> RotationMetrics {
        self.metrics
    }

    pub fn plan_rotation(
        &mut self,
        descriptor: SecretDescriptor,
        current: SecretVersion,
        candidate: SecretVersion,
        now_secs: u64,
    ) -> Result<(), SecretRotationError> {
        if candidate.version <= current.version {
            self.metrics.policy_violations += 1;
            return Err(SecretRotationError::CandidateVersionNotNewer);
        }
        let key = descriptor.key();
        if self.plans.contains_key(&key) {
            return Err(SecretRotationError::AlreadyManaged);
        }
        self.metrics.rotations_started += 1;
        self.metrics.active_secret_age_secs = now_secs.saturating_sub(current.activated_at_secs);
        self.plans.insert(
            key,
            RotationPlan {
                descriptor,
                current,
                candidate,
                phase: RotationPhase::Planned,
                canary_percent: 0,
                overlap_ends_at_secs: now_secs.saturating_add(DEFAULT_OVERLAP_SECS),
            },
        );
        Ok(())
    }

    pub fn begin_canary(
        &mut self,
        service: &str,
        name: &str,
        percent: u8,
    ) -> Result<(), SecretRotationError> {
        let plan = self.plan_mut(service, name)?;
        plan.phase = RotationPhase::Canary;
        plan.canary_percent = percent.min(100);
        Ok(())
    }

    pub fn promote(
        &mut self,
        service: &str,
        name: &str,
        sample: CanarySample,
        now_secs: u64,
    ) -> Result<(), SecretRotationError> {
        self.metrics.last_p99_latency_ms = sample.p99_latency_ms;
        if sample.p99_latency_ms > CRITICAL_PATH_P99_BUDGET_MS {
            self.metrics.policy_violations += 1;
            return Err(SecretRotationError::LatencyBudgetExceeded);
        }
        if success_bps(sample) < DEFAULT_CANARY_SUCCESS_BPS {
            self.metrics.policy_violations += 1;
            return Err(SecretRotationError::CanaryBelowThreshold);
        }
        let plan = self.plan_mut(service, name)?;
        if now_secs < plan.overlap_ends_at_secs {
            return Err(SecretRotationError::OverlapWindowActive);
        }
        plan.phase = RotationPhase::Promoted;
        plan.current = plan.candidate.clone();
        plan.canary_percent = 100;
        self.metrics.rotations_promoted += 1;
        Ok(())
    }

    pub fn rollback(&mut self, service: &str, name: &str) -> Result<(), SecretRotationError> {
        let plan = self.plan_mut(service, name)?;
        plan.phase = RotationPhase::RolledBack;
        plan.canary_percent = 0;
        self.metrics.rotations_rolled_back += 1;
        Ok(())
    }

    pub fn active_versions(
        &self,
        service: &str,
        name: &str,
        now_secs: u64,
    ) -> Result<Vec<u64>, SecretRotationError> {
        let plan = self.plan(service, name)?;
        let mut versions = Vec::new();
        if plan.current.is_active_at(now_secs) {
            versions.push(plan.current.version);
        }
        if matches!(plan.phase, RotationPhase::Canary) && plan.candidate.is_active_at(now_secs) {
            versions.push(plan.candidate.version);
        }
        Ok(versions)
    }

    fn plan(&self, service: &str, name: &str) -> Result<&RotationPlan, SecretRotationError> {
        self.plans
            .get(&make_key(service, name))
            .ok_or(SecretRotationError::UnknownSecret)
    }

    fn plan_mut(
        &mut self,
        service: &str,
        name: &str,
    ) -> Result<&mut RotationPlan, SecretRotationError> {
        self.plans
            .get_mut(&make_key(service, name))
            .ok_or(SecretRotationError::UnknownSecret)
    }
}

fn make_key(service: &str, name: &str) -> String {
    let mut key = service.to_string();
    key.push(':');
    key.push_str(name);
    key
}

fn success_bps(sample: CanarySample) -> u16 {
    if sample.requests == 0 {
        return 0;
    }
    ((sample.successes.saturating_mul(10_000)) / sample.requests).min(10_000) as u16
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec;

    fn descriptor() -> SecretDescriptor {
        SecretDescriptor::new(
            "analytics",
            "postgres",
            SecretKind::DatabaseCredential,
            86_400,
        )
    }

    #[test]
    fn plans_canary_and_promotes_after_overlap() {
        let mut service = SecretRotationService::default();
        let current = SecretVersion::from_material(1, b"old", 0, 10_000);
        let candidate = SecretVersion::from_material(2, b"new", 100, 10_000);
        service
            .plan_rotation(descriptor(), current, candidate, 100)
            .unwrap();
        service.begin_canary("analytics", "postgres", 5).unwrap();
        assert_eq!(
            service
                .active_versions("analytics", "postgres", 150)
                .unwrap(),
            vec![1, 2]
        );
        service
            .promote(
                "analytics",
                "postgres",
                CanarySample {
                    requests: 10_000,
                    successes: 9_995,
                    p99_latency_ms: 42,
                },
                401,
            )
            .unwrap();
        assert_eq!(service.metrics().rotations_promoted, 1);
        assert_eq!(
            service
                .active_versions("analytics", "postgres", 402)
                .unwrap(),
            vec![2]
        );
    }

    #[test]
    fn rejects_stale_candidate_versions() {
        let mut service = SecretRotationService::default();
        let current = SecretVersion::from_material(7, b"old", 0, 100);
        let candidate = SecretVersion::from_material(7, b"same", 0, 100);
        assert_eq!(
            service.plan_rotation(descriptor(), current, candidate, 0),
            Err(SecretRotationError::CandidateVersionNotNewer)
        );
        assert_eq!(service.metrics().policy_violations, 1);
    }

    #[test]
    fn canary_enforces_security_and_latency_gates() {
        let mut service = SecretRotationService::default();
        service
            .plan_rotation(
                descriptor(),
                SecretVersion::from_material(1, b"old", 0, 1000),
                SecretVersion::from_material(2, b"new", 0, 1000),
                0,
            )
            .unwrap();
        assert_eq!(
            service.promote(
                "analytics",
                "postgres",
                CanarySample {
                    requests: 1000,
                    successes: 998,
                    p99_latency_ms: 20
                },
                301
            ),
            Err(SecretRotationError::CanaryBelowThreshold)
        );
        assert_eq!(
            service.promote(
                "analytics",
                "postgres",
                CanarySample {
                    requests: 1000,
                    successes: 1000,
                    p99_latency_ms: 101
                },
                301
            ),
            Err(SecretRotationError::LatencyBudgetExceeded)
        );
    }
}
