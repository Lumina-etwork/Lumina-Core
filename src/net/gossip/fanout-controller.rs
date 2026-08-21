use std::cmp;

pub struct FanoutController {
    network_size: usize,
}

impl FanoutController {
    pub fn new(network_size: usize) -> Self {
        Self { network_size }
    }

    pub fn compute_fanout(&self, amplification_ratio: u32) -> u32 {
        let n = self.network_size as f64;
        let log2_n = n.log2().ceil() as u32;
        
        // fanout = min(log2(N), max(3, 20 - amplification_ratio / 10))
        let adaptive_term = 20u32.saturating_sub(amplification_ratio / 10);
        let max_term = cmp::max(3, adaptive_term);
        
        cmp::min(log2_n, max_term)
    }
}
