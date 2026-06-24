use std::time::{Instant, Duration};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Debug)]
pub struct StreamFrame {
    pub offset: u64,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct RetransmitRequest {
    pub stream_id: u32,
    pub offset: u64,
    pub length: u64,
}

pub struct MissingRange {
    pub offset: u64,
    pub length: u64,
    pub created_at: Instant,
}

pub struct ReorderBuffer {
    pub stream_id: u32,
    pub buffer: BTreeMap<u64, Vec<u8>>, // offset -> data
    pub next_expected_offset: u64,
    pub missing_ranges: VecDeque<MissingRange>,
    pub max_buffer_size: usize,
    pub current_buffer_size: usize,
}

impl ReorderBuffer {
    pub fn new(stream_id: u32) -> Self {
        Self {
            stream_id,
            buffer: BTreeMap::new(),
            next_expected_offset: 0,
            missing_ranges: VecDeque::new(),
            max_buffer_size: 256 * 1024, // 256 KB per stream
            current_buffer_size: 0,
        }
    }

    pub fn insert_frame(&mut self, frame: StreamFrame) -> Vec<RetransmitRequest> {
        let mut requests = Vec::new();
        let frame_len = frame.data.len();
        
        if frame.offset > self.next_expected_offset {
            // There is a gap!
            let missing_offset = self.next_expected_offset;
            let missing_len = frame.offset - self.next_expected_offset;
            
            // Check if buffer size limit will be exceeded
            if self.current_buffer_size + frame_len <= self.max_buffer_size {
                self.buffer.insert(frame.offset, frame.data);
                self.current_buffer_size += frame_len;
                
                // Add to missing ranges if not already tracked
                let already_tracked = self.missing_ranges.iter().any(|r| r.offset == missing_offset);
                if !already_tracked {
                    self.missing_ranges.push_back(MissingRange {
                        offset: missing_offset,
                        length: missing_len,
                        created_at: Instant::now(),
                    });
                }
            }
        } else if frame.offset == self.next_expected_offset {
            // Deliver in-order data
            self.next_expected_offset += frame_len as u64;
            
            // Deliver subsequently buffered frames that are now in-order
            while let Some(data) = self.buffer.remove(&self.next_expected_offset) {
                let data_len = data.len() as u64;
                self.current_buffer_size -= data.len();
                self.next_expected_offset += data_len;
            }
            
            // Remove any missing range that is now filled
            self.missing_ranges.retain(|r| r.offset >= self.next_expected_offset);
        }
        
        requests
    }

    // timer-driven retransmission request: for each missing offset range, start a 100ms timer; on expiry, send a RetransmitRequest
    pub fn check_timeouts(&mut self) -> Vec<RetransmitRequest> {
        let mut requests = Vec::new();
        let now = Instant::now();
        let timeout = Duration::from_millis(100);

        for range in &self.missing_ranges {
            if now.duration_since(range.created_at) >= timeout {
                requests.push(RetransmitRequest {
                    stream_id: self.stream_id,
                    offset: range.offset,
                    length: range.length,
                });
            }
        }
        
        requests
    }
}
