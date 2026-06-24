use std::collections::HashMap;

const MAX_FANOUT: usize = 20;
const MIN_FANOUT: usize = 3;
const MAX_LOG2_PEERS: usize = 20;

pub struct FanoutController {
    pub amplification_ratios: HashMap<Vec<u8>, f64>,
    pub peer_message_counts: HashMap<Vec<u8>, (u64, u64)>,
}

impl FanoutController {
    pub fn new() -> Self {
        Self {
            amplification_ratios: HashMap::new(),
            peer_message_counts: HashMap::new(),
        }
    }

    pub fn compute_fanout(&self, total_peers: usize) -> usize {
        let log2 = ((total_peers as f64).log2().ceil() as usize).max(1);
        let base_fanout = log2.min(MAX_LOG2_PEERS);

        if self.amplification_ratios.is_empty() {
            return base_fanout.clamp(MIN_FANOUT, MAX_FANOUT);
        }

        let avg_ratio: f64 = self
            .amplification_ratios
            .values()
            .copied()
            .sum::<f64>()
            / self.amplification_ratios.len() as f64;

        let adaptive = (MAX_FANOUT as f64 - avg_ratio / 10.0).round() as usize;
        adaptive.max(MIN_FANOUT).min(base_fanout)
    }

    pub fn compute_peer_fanout(&self, peer_id: &[u8], total_peers: usize) -> usize {
        let log2 = ((total_peers as f64).log2().ceil() as usize).max(1);

        let ratio = self.amplification_ratios.get(peer_id).copied().unwrap_or(0.0);

        let base = (MAX_FANOUT as f64 - ratio / 10.0).round() as usize;
        base.max(MIN_FANOUT).min(log2)
    }

    pub fn record_amplification_ratio(&mut self, peer_id: Vec<u8>, ratio: f64) {
        self.amplification_ratios.insert(peer_id, ratio);
    }

    pub fn record_message(&mut self, peer_id: Vec<u8>, sent: u64, received: u64) {
        let entry = self
            .peer_message_counts
            .entry(peer_id.clone())
            .or_insert((0, 0));
        entry.0 = entry.0.wrapping_add(sent);
        entry.1 = entry.1.wrapping_add(received);

        if entry.1 > 0 {
            let ratio = entry.0 as f64 / entry.1 as f64;
            self.amplification_ratios.insert(peer_id.clone(), ratio);
        }
    }

    pub fn get_amplification_ratio(&self, peer_id: &[u8]) -> f64 {
        self.amplification_ratios
            .get(peer_id)
            .copied()
            .unwrap_or(0.0)
    }

    pub fn select_peers(&self, candidates: &[Vec<u8>], fanout: usize) -> Vec<Vec<u8>> {
        let mut scored: Vec<(f64, &Vec<u8>)> = candidates
            .iter()
            .map(|p| {
                let ratio = self.amplification_ratios.get(p).copied().unwrap_or(0.0);
                (ratio, p)
            })
            .collect();

        scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        scored.into_iter().take(fanout).map(|(_, p)| p.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fanout_with_no_amplification() {
        let controller = FanoutController::new();
        let fanout = controller.compute_fanout(100);
        assert!(fanout >= MIN_FANOUT);
        assert!(fanout <= MAX_FANOUT);
    }

    #[test]
    fn test_adaptive_fanout_high_amplification() {
        let mut controller = FanoutController::new();
        controller.record_amplification_ratio(b"malicious".to_vec(), 150.0);
        let fanout = controller.compute_fanout(100);
        assert!(fanout <= MAX_FANOUT);
        assert!(fanout >= MIN_FANOUT);
    }

    #[test]
    fn test_peer_fanout_decreases_with_high_ratio() {
        let mut controller = FanoutController::new();
        controller.record_amplification_ratio(b"bad-peer".to_vec(), 200.0);
        let peer_fanout = controller.compute_peer_fanout(b"bad-peer", 100);
        let normal_fanout = controller.compute_peer_fanout(b"good-peer", 100);
        assert!(peer_fanout <= normal_fanout);
    }

    #[test]
    fn test_select_peers_prefers_low_ratio() {
        let mut controller = FanoutController::new();
        controller.record_amplification_ratio(b"high".to_vec(), 100.0);
        controller.record_amplification_ratio(b"low".to_vec(), 1.0);

        let candidates = vec![b"high".to_vec(), b"low".to_vec(), b"medium".to_vec()];
        let selected = controller.select_peers(&candidates, 2);
        assert!(selected.contains(&b"low".to_vec()));
    }

    #[test]
    fn test_fanout_clamped_to_bounds() {
        let controller = FanoutController::new();
        let tiny = controller.compute_fanout(2);
        assert!(tiny >= MIN_FANOUT);
        let huge = controller.compute_fanout(10_000);
        assert!(huge <= MAX_FANOUT);
    }

    #[test]
    fn test_record_message_updates_ratio() {
        let mut controller = FanoutController::new();
        controller.record_message(b"peer".to_vec(), 100, 1);
        let ratio = controller.get_amplification_ratio(b"peer");
        assert!((ratio - 100.0).abs() < 0.001);
    }
}
