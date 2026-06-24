use std::collections::HashMap;

const RENEWAL_WINDOW_HOURS: f64 = 2.4;
const COHORT_COUNT: u64 = 10;

pub struct RenewalCohort {
    pub cohort_id: u64,
    pub sub_window_start: f64,
    pub sub_window_end: f64,
}

pub struct ExpirationWatcher {
    cohorts: HashMap<u64, RenewalCohort>,
}

impl ExpirationWatcher {
    pub fn new() -> Self {
        let mut cohorts = HashMap::new();
        let sub_window = RENEWAL_WINDOW_HOURS / COHORT_COUNT as f64;
        for i in 0..COHORT_COUNT {
            cohorts.insert(i, RenewalCohort {
                cohort_id: i,
                sub_window_start: i as f64 * sub_window,
                sub_window_end: (i + 1) as f64 * sub_window,
            });
        }
        Self { cohorts }
    }

    pub fn assign_cohort(&self, node_id: &str) -> u64 {
        let hash = node_id.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        hash % COHORT_COUNT
    }

    pub fn get_cohort(&self, node_id: &str) -> Option<&RenewalCohort> {
        let id = self.assign_cohort(node_id);
        self.cohorts.get(&id)
    }

    pub fn cohort_count(&self) -> usize {
        self.cohorts.len()
    }
}
