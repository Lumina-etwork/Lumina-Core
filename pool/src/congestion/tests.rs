use super::backpressure_sender::BackpressureSender;
use super::probe_scheduler::ProbeScheduler;
use super::types::{CongestionEvent, WindowSize, MAX_ZERO_WINDOW_COUNT};
use super::window_controller::WindowController;

#[test]
fn test_integration_forced_recovery_within_six_probes() {
    let mut controller = WindowController::new();
    let mut scheduler = ProbeScheduler::new();
    let mut sender = BackpressureSender::new();
    let tenant = "integration-tenant".to_string();

    let mut forced_open_detected = false;

    for i in 1..=10 {
        let event = controller.process_heartbeat(&tenant, WindowSize::SUSPENDED);

        match event {
            Some(CongestionEvent::WindowForcedOpen { new_window, .. }) => {
                assert_eq!(new_window, WindowSize::MIN);
                assert!(
                    i as u32 <= MAX_ZERO_WINDOW_COUNT,
                    "Forced open happened at probe {i}, exceeding max of {MAX_ZERO_WINDOW_COUNT}"
                );
                forced_open_detected = true;

                scheduler.on_non_zero_ack(&tenant);
                sender.reset(&tenant);
            }
            Some(CongestionEvent::ZeroWindowHeartbeat { .. }) => {
                scheduler.on_zero_window(&tenant);
                let suppressed = sender.should_suppress_ack(&tenant, WindowSize::SUSPENDED);
                if i > 3 {
                    assert!(suppressed, "ACK should be suppressed after heartbeat {i}");
                }
            }
            None => {}
            _ => {}
        }
    }

    assert!(
        forced_open_detected,
        "Window should have been forced open within {MAX_ZERO_WINDOW_COUNT} probes"
    );

    let metrics = controller.metrics();
    assert_eq!(
        metrics.zero_window_stalls_prevented, 1,
        "Exactly 1 stall should have been prevented"
    );
}
