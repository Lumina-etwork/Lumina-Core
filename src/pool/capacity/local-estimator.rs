pub struct LocalEstimator {
    pub raw_measurements: u64,
    pub locally_computed_estimate: u64,
}

impl LocalEstimator {
    pub fn new() -> Self {
        Self {
            raw_measurements: 0,
            locally_computed_estimate: 0,
        }
    }

    pub fn send_to_coordinator(&self, coordinator: &mut super::global_coordinator::GlobalCoordinator) {
        coordinator.receive(self.raw_measurements, self.locally_computed_estimate);
    }
}
