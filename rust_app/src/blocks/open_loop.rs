use crate::blocks::BlockRunnable;
use crate::error::EngineError;
use crate::events::{EventType, SystemEvent};
use crate::hal::{ControlMode, RobotProfile};
use crate::robot::RobotContext;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// Open-loop motion block.
///
/// Sends a constant linear and angular velocity for a fixed duration.
/// No odometry feedback — works on any robot regardless of sensor
/// configuration. Useful for simple manoeuvres or robots without
/// odometry support.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OpenLoopBlock {
    pub linear_x: f64,
    pub angular_z: f64,
    pub duration_ms: u64,
}

#[async_trait]
impl BlockRunnable for OpenLoopBlock {
    fn validate(&self, profile: &RobotProfile) -> Result<(), String> {
        if profile.control_mode != ControlMode::Velocity {
            return Err(format!(
                "Open-Loop requires a velocity-controlled robot; the active profile '{}' is joint-position controlled",
                profile.name
            ));
        }
        if self.duration_ms == 0 {
            return Err("duration_ms must be greater than zero".to_string());
        }
        // Negative velocities are valid (reverse / clockwise) — only the
        // magnitude is checked against the actuator limits.
        if !self.linear_x.is_finite() || self.linear_x.abs() > profile.max_linear_speed {
            return Err(format!(
                "linear_x {} m/s exceeds the {} limit of ±{} m/s",
                self.linear_x, profile.name, profile.max_linear_speed
            ));
        }
        if !self.angular_z.is_finite() || self.angular_z.abs() > profile.max_angular_speed {
            return Err(format!(
                "angular_z {} rad/s exceeds the {} limit of ±{} rad/s",
                self.angular_z, profile.name, profile.max_angular_speed
            ));
        }
        Ok(())
    }

    async fn execute(
        &self,
        ctx: &mut RobotContext,
        bus: &broadcast::Sender<SystemEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), EngineError> {
        let duration_secs = self.duration_ms as f64 / 1000.0;
        let start = std::time::Instant::now();
        let total = Duration::from_millis(self.duration_ms);

        while start.elapsed() < total {
            if cancel.is_cancelled() {
                ctx.stop().map_err(EngineError::ConnectionError)?;
                return Err(EngineError::Aborted);
            }

            ctx.publish_velocity(self.linear_x, self.angular_z)
                .map_err(EngineError::ConnectionError)?;

            let elapsed = start.elapsed().as_secs_f64();
            let _ = bus.send(SystemEvent::with_progress(
                "Open-Loop Moving...".into(),
                elapsed,
                duration_secs,
            ));

            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        ctx.stop().map_err(EngineError::ConnectionError)?;
        let _ = bus.send(SystemEvent::new(
            EventType::Info,
            "Open-Loop Completed".into(),
        ));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hal::test_profile;

    fn block(linear_x: f64, angular_z: f64, duration_ms: u64) -> OpenLoopBlock {
        OpenLoopBlock {
            linear_x,
            angular_z,
            duration_ms,
        }
    }

    #[test]
    fn accepts_velocities_within_limits() {
        assert!(block(0.2, 1.0, 1000).validate(&test_profile()).is_ok());
    }

    #[test]
    fn accepts_negative_velocities_reverse_and_clockwise() {
        assert!(block(-0.2, -1.0, 1000).validate(&test_profile()).is_ok());
    }

    #[test]
    fn rejects_zero_duration() {
        assert!(block(0.2, 0.0, 0).validate(&test_profile()).is_err());
    }

    #[test]
    fn rejects_magnitudes_above_actuator_limits_in_both_signs() {
        // TestBot limits: 0.22 m/s linear, 2.84 rad/s angular
        assert!(block(0.5, 0.0, 1000).validate(&test_profile()).is_err());
        assert!(block(-0.5, 0.0, 1000).validate(&test_profile()).is_err());
        assert!(block(0.0, 3.0, 1000).validate(&test_profile()).is_err());
        assert!(block(0.0, -3.0, 1000).validate(&test_profile()).is_err());
    }

    #[test]
    fn rejects_non_finite_velocities() {
        assert!(block(f64::NAN, 0.0, 1000).validate(&test_profile()).is_err());
        assert!(block(0.0, f64::NEG_INFINITY, 1000).validate(&test_profile()).is_err());
    }
}
