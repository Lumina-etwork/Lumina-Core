use std::time::Duration;

pub struct ConflictDetector;

impl ConflictDetector {
    pub fn detect_conflict(batch1_epoch: u64, batch1_ranges: &[(u64, u64)], batch2_epoch: u64, batch2_ranges: &[(u64, u64)]) -> Option<(u64, Duration)> {
        let mut overlap = false;
        for r1 in batch1_ranges {
            for r2 in batch2_ranges {
                if r1.0 <= r2.1 && r2.0 <= r1.1 {
                    overlap = true;
                    break;
                }
            }
        }

        if overlap {
            let abort_epoch = std::cmp::max(batch1_epoch, batch2_epoch);
            let retries = 1;
            let backoff = Duration::from_millis(100 * (2_u64.pow(retries as u32)));
            Some((abort_epoch, backoff))
        } else {
            None
        }
    }
}
