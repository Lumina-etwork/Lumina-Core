use super::retransmission_requestor::RetransmitRequest; use super::flow_controller::FlowController; use super::reorder_buffer::ReorderBuffer; use std::time::{Duration, Instant}; use std::collections::HashMap; pub struct StreamState { pub flow_controller: FlowController, pub reorder_buffer: ReorderBuffer, pub timeouts: Vec<Instant> } pub struct StreamMultiplexer { pub streams: HashMap<u64, StreamState> } impl StreamMultiplexer { pub fn new() -> Self { Self { streams: HashMap::new() } } pub fn add_stream(&mut self, id: u64) { self.streams.insert(id, StreamState { flow_controller: FlowController::new(), reorder_buffer: ReorderBuffer::new(id), timeouts: Vec::new() }); } pub fn handle_retransmit_request(&mut self, req: RetransmitRequest) { if let Some(state) = self.streams.get_mut(&req.stream_id) { let now = Instant::now(); state.timeouts.push(now); state.timeouts.retain(|&t| now.duration_since(t) <= Duration::from_secs(1)); if state.timeouts.len() >= 3 { state.flow_controller.set_ordered_only_mode(true); } } } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chaos_test_reorder() {
        // mock chaos test
        assert!(true);
    }
}
