use super::reorder_buffer::RetransmitRequest;

pub struct RetransmissionRequestor {
    pub pending_requests: Vec<RetransmitRequest>,
}

impl RetransmissionRequestor {
    pub fn new() -> Self {
        Self {
            pending_requests: Vec::new(),
        }
    }

    pub fn queue_request(&mut self, request: RetransmitRequest) {
        self.pending_requests.push(request);
    }

    pub fn dispatch_requests(&mut self) -> Vec<RetransmitRequest> {
        std::mem::take(&mut self.pending_requests)
    }
}
