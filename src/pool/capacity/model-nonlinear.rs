pub struct NonLinearModel {
    pub gc_pause_ms: u64,
}

impl NonLinearModel {
    pub fn new() -> Self {
        Self { gc_pause_ms: 0 }
    }

    pub fn apply_gc_pause(&mut self, pause_ms: u64) {
        self.gc_pause_ms = pause_ms;
    }

    pub fn compute_capacity(&self, base_capacity: u64) -> u64 {
        let fraction = self.gc_pause_ms as f64 / 1000.0;
        let fraction = fraction.clamp(0.0, 1.0);
        let reduction = (base_capacity as f64 * fraction) as u64;
        base_capacity.saturating_sub(reduction)
    }
}
