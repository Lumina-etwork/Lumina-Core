use std::collections::{HashMap, VecDeque};
use std::time::{Instant, Duration};
use super::reorder_buffer::{StreamFrame, RetransmitRequest, ReorderBuffer};
use super::flow_controller::FlowController;

pub struct StreamState {
    pub stream_id: u32,
    pub send_buffer: Vec<u8>,
    pub reorder_buffer: ReorderBuffer,
    pub flow_controller: FlowController,
    pub ordered_only_mode: bool,
    pub timeout_timestamps: VecDeque<Instant>,
}

pub struct StreamMultiplexer {
    pub streams: HashMap<u32, StreamState>,
    pub max_concurrent_streams: usize,
    pub single_stream_mode: bool,
}

impl StreamMultiplexer {
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
            max_concurrent_streams: 100,
            single_stream_mode: false,
        }
    }

    pub fn create_stream(&mut self, stream_id: u32) -> bool {
        if self.streams.len() >= self.max_concurrent_streams {
            return false;
        }
        self.streams.insert(stream_id, StreamState {
            stream_id,
            send_buffer: Vec::new(),
            reorder_buffer: ReorderBuffer::new(stream_id),
            flow_controller: FlowController::new(),
            ordered_only_mode: false,
            timeout_timestamps: VecDeque::new(),
        });
        true
    }

    pub fn handle_retransmit_request(&mut self, request: RetransmitRequest) -> Option<Vec<u8>> {
        if let Some(stream) = self.streams.get_mut(&request.stream_id) {
            let offset = request.offset as usize;
            let length = request.length as usize;
            if offset + length <= stream.send_buffer.len() {
                // immediately resend the requested range from the send buffer
                return Some(stream.send_buffer[offset..(offset + length)].to_vec());
            }
        }
        None
    }

    pub fn record_timeout(&mut self, stream_id: u32) {
        if let Some(stream) = self.streams.get_mut(&stream_id) {
            let now = Instant::now();
            stream.timeout_timestamps.push_back(now);
            
            // Retain only timeouts within the last 1s
            while let Some(&time) = stream.timeout_timestamps.front() {
                if now.duration_since(time) > Duration::from_secs(1) {
                    stream.timeout_timestamps.pop_front();
                } else {
                    break;
                }
            }

            // after 3 timeouts within 1s, set the stream to ordered-only mode (no reordering)
            // and downgrade to single-stream mode
            if stream.timeout_timestamps.len() >= 3 {
                stream.ordered_only_mode = true;
                self.single_stream_mode = true;
                
                // Reduce window by 50% in flow controller
                stream.flow_controller.on_ordered_only_mode_activated();
            }
        }
    }

    pub fn receive_frame(&mut self, stream_id: u32, frame: StreamFrame) -> Vec<RetransmitRequest> {
        if let Some(stream) = self.streams.get_mut(&stream_id) {
            if stream.ordered_only_mode {
                // In ordered-only mode, we don't buffer out-of-order packets.
                // We expect exact next expected offset, otherwise it is dropped/ignored
                if frame.offset == stream.reorder_buffer.next_expected_offset {
                    stream.reorder_buffer.insert_frame(frame)
                } else {
                    // Out-of-order frames are ignored in ordered-only mode
                    Vec::new()
                }
            } else {
                stream.reorder_buffer.insert_frame(frame)
            }
        } else {
            Vec::new()
        }
    }
}
