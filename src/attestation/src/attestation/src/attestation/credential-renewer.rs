use rand::Rng;
use super::credential_signer::{RateLimitedSigner, RENEWAL_METRICS};
use super::expiration_watcher::ExpirationWatcher;

pub struct CredentialRenewer {
    signer: RateLimitedSigner,
    watcher: ExpirationWatcher,
}

impl CredentialRenewer {
    pub fn new() -> Self {
        Self {
            signer: RateLimitedSigner::new(),
            watcher: ExpirationWatcher::new(),
        }
    }

    pub fn compute_delay_secs(&self, current_time: f64, window_start: f64, window_duration: f64) -> f64 {
        let mut rng = rand::thread_rng();
        let base = rng.gen_range(0.0..300.0);
        let proximity = (current_time - window_start) / window_duration;
        let adaptive = proximity * 60.0;
        base + adaptive
    }

    pub fn try_renew(&self, node_id: &str, _credential: &[u8]) -> Result<[u8; 64], RenewError> {
        let cohort = self.watcher.get_cohort(node_id)
            .ok_or(RenewError::CohortNotFound)?;

        let result = self.signer.try_sign(_credential);
        if result.rate_limited {
            RENEWAL_METRICS.record_rate_limited();
            return Err(RenewError::RateLimited {
                retry_after_secs: result.retry_after_secs,
            });
        }

        Ok(result.signature.unwrap_or([0u8; 64]))
    }

    pub fn renew_batch(&self, nodes: &[(String, Vec<u8>)]) -> Vec<Result<[u8; 64], RenewError>> {
        let mut results = Vec::new();
        for (node_id, credential) in nodes {
            RENEWAL_METRICS.set_queue_depth(nodes.len() as u64);
            results.push(self.try_renew(node_id, credential));
        }
        results
    }
}

#[derive(Debug)]
pub enum RenewError {
    RateLimited { retry_after_secs: u64 },
    CohortNotFound,
    SigningFailed,
}
