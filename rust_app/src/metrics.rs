//! Experiment markers for the measurement campaign (thesis Ch. 6).
//!
//! Publishes small JSON markers on a ROS topic so rosbag2 records
//! platform events (request received, step start/finish, abort) in the
//! SAME clock domain as `/cmd_vel` and odometry. Every latency metric
//! then becomes a subtraction between messages of one bag — comparing
//! timestamps from different clocks (browser vs DDS) would invalidate
//! the measurement.
//!
//! Markers are fire-and-forget and never disturb the control flow: if
//! the publisher cannot be created the platform runs without metrics.

use r2r::Publisher;
use r2r::std_msgs::msg::String as RosString;
use std::sync::{Arc, Mutex};

use crate::events::SystemEvent;

pub const MARKER_TOPIC: &str = "/webapp_ros/markers";

/// Thin wrapper around a `std_msgs/String` publisher carrying JSON
/// payloads: `{"event": "...", "payload": {...}}`.
///
/// The publisher lives behind a `Mutex` because `r2r::Publisher` is
/// `Send` but not `Sync` (raw rcl pointer), and `Arc<T>` requires
/// `T: Sync` to cross threads — same pattern as `RobotContext`. The
/// lock is held only for the non-blocking `publish()` call.
#[derive(Clone)]
pub struct ExperimentMarkers {
    publisher: Arc<Mutex<Publisher<RosString>>>,
}

impl ExperimentMarkers {
    pub fn new(node: &Arc<Mutex<r2r::Node>>) -> Option<Self> {
        let mut guard = node.lock().ok()?;
        match guard.create_publisher::<RosString>(MARKER_TOPIC, r2r::QosProfile::default()) {
            Ok(p) => {
                tracing::info!("Experiment markers enabled on {}", MARKER_TOPIC);
                Some(Self {
                    publisher: Arc::new(Mutex::new(p)),
                })
            }
            Err(e) => {
                tracing::warn!("Experiment markers disabled: {}", e);
                None
            }
        }
    }

    /// Publish a named marker with an arbitrary JSON payload.
    pub fn emit(&self, event: &str, payload: serde_json::Value) {
        let msg = RosString {
            data: serde_json::json!({ "event": event, "payload": payload }).to_string(),
        };
        if let Ok(publisher) = self.publisher.lock() {
            let _ = publisher.publish(&msg);
        }
    }

    /// Republish a system event (STEP_START, PROGRAM_ABORT, …) as a
    /// marker. Used by the event-bus bridge so every event the engine
    /// already emits lands in the bag without touching the engine.
    pub fn emit_system_event(&self, event: &SystemEvent) {
        let payload = serde_json::to_value(event).unwrap_or_default();
        self.emit("system_event", payload);
    }
}
