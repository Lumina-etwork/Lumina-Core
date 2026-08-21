pub struct GlobalCoordinator {
    pub consecutive_divergences: u32,
    pub global_capacity: u64,
}

impl GlobalCoordinator {
    pub fn new() -> Self {
        Self {
            consecutive_divergences: 0,
            global_capacity: 0,
        }
    }

    pub fn receive(&mut self, raw_measurements: u64, estimate_local: u64) {
        let estimate_linear = super::model_linear::compute_linear(raw_measurements);
        
        let abs_diff = (estimate_local as f64 - estimate_linear as f64).abs();
        let max_estimate = estimate_local.max(estimate_linear);
        let divergence = if max_estimate > 0 { abs_diff / max_estimate as f64 } else { 0.0 };
        
        let correction_factor = 1.0 - abs_diff / (if estimate_local > 0 { estimate_local as f64 } else { 1.0 });
        let mut capacity_global = (estimate_local as f64 * correction_factor) as u64;

        if divergence > 0.10 {
            self.consecutive_divergences += 1;
            if self.consecutive_divergences >= 3 {
                println!("CapacityModelDivergence: Warning!");
                capacity_global = estimate_local.min(estimate_linear);
            }
        } else {
            self.consecutive_divergences = 0;
        }
        
        self.global_capacity = capacity_global;
    }
}
