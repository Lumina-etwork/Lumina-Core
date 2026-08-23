use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum SignerError {
    RateLimited { retry_after: Duration },
    Other(String),
}

pub struct CredentialSigner {
    pub capacity: f64,
    pub tokens: f64,
    pub last_refill: Instant,
    pub rate_limited_renewals: u64,
}

impl CredentialSigner {
    pub fn new() -> Self {
        Self {
            capacity: 60.0,
            tokens: 60.0,
            last_refill: Instant::now(),
            rate_limited_renewals: 0,
        }
    }

    pub fn sign_credential(&mut self, now: Instant) -> Result<String, SignerError> {
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        // refill 1 per second
        self.tokens += elapsed * 1.0;
        if self.tokens > self.capacity {
            self.tokens = self.capacity;
        }
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok("signed_credential".to_string())
        } else {
            self.rate_limited_renewals += 1;
            let retry_after_secs = 1.0 - self.tokens;
            Err(SignerError::RateLimited {
                retry_after: Duration::from_secs_f64(retry_after_secs),
            })
        }
    }
}
