pub struct FlowController {
    pub window_size: u64,
    pub initial_window_size: u64,
    pub ordered_only_active: bool,
}

impl FlowController {
    pub fn new() -> Self {
        Self {
            window_size: 64 * 1024, // 64 KB initial window
            initial_window_size: 64 * 1024,
            ordered_only_active: false,
        }
    }

    // doubling per RTT
    pub fn on_rtt_completed(&mut self) {
        if !self.ordered_only_active {
            self.window_size *= 2;
        } else {
            // If ordered-only mode is active, limit the window growth to prevent head-of-line blocking
            self.window_size = (self.window_size * 2).min(self.initial_window_size / 2);
        }
    }

    // reduce the stream's window by 50% when ordered-only mode is active to prevent head-of-line blocking
    pub fn on_ordered_only_mode_activated(&mut self) {
        if !self.ordered_only_active {
            self.ordered_only_active = true;
            self.window_size /= 2;
        }
    }
}
