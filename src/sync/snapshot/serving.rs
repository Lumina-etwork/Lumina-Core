use alloc::vec::Vec;

pub struct RangeRequest {
    pub start: usize,
    pub end: usize,
}

pub struct RangeResponse {
    pub data: Vec<u8>,
    pub total_size: usize,
}

pub fn serve_snapshot_range(compressed_state: &[u8], range: RangeRequest) -> Option<RangeResponse> {
    if range.start > compressed_state.len() {
        return None;
    }
    
    let end = core::cmp::min(range.end, compressed_state.len());
    let data = compressed_state[range.start..end].to_vec();
    
    Some(RangeResponse {
        data,
        total_size: compressed_state.len(),
    })
}
