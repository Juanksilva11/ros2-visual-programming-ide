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

/// Odometry-based linear move block.
///
/// Drives the robot forward at a constant speed until the target distance
/// is reached, using closed-loop feedback from the HAL's odometry stream.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OdometryMoveBlock {
    pub target_distance: f64,
    pub speed: f64,
}

#[async_trait]
impl BlockRunnable for OdometryMoveBlock {
    fn validate(&self, profile: &RobotProfile) -> Result<(), String> {
        if profile.control_mode != ControlMode::Velocity {
            return Err(format!(
                "Linear Move requires a velocity-controlled robot; the active profile '{}' is joint-position controlled",
                profile.name
            ));
        }
        // Direction comes from the sign of target_distance (negative =
        // reverse), mirroring RotateBlock where the angle's sign sets the
        // direction. Zero distance would never be covered.
        if !self.target_distance.is_finite() || self.target_distance == 0.0 {
            return Err(format!(
                "target_distance must be a non-zero number (got {})",
                self.target_distance
            ));
        }
        // Speed is a magnitude: always positive, bounded by the actuator.
        if !self.speed.is_finite() || self.speed <= 0.0 {
            return Err(format!(
                "speed must be a positive number (got {})",
                self.speed
            ));
        }
        if self.speed > profile.max_linear_speed {
            return Err(format!(
                "speed {} m/s exceeds the {} limit of {} m/s",
                self.speed, profile.name, profile.max_linear_speed
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
        // Wait for an initial odometry reading (max ~1 s)
        let mut attempts = 0;
        let start = loop {
            if cancel.is_cancelled() {
                return Err(EngineError::Aborted);
            }
            if let Some(odom) = ctx.get_current_odom() {
                break odom;
            }
            if attempts > 10 {
                return Err(EngineError::ExecutionFailed("Sensor Timeout".into()));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            attempts += 1;
        };

        // The sign of target_distance sets the direction; the closed loop
        // tracks the Euclidean displacement magnitude, which grows the same
        // way whether the robot drives forward or in reverse.
        let target = self.target_distance.abs();
        let direction = self.target_distance.signum();

        // Closed-loop drive
        loop {
            if cancel.is_cancelled() {
                ctx.stop().map_err(EngineError::ConnectionError)?;
                return Err(EngineError::Aborted);
            }

            let current = ctx
                .get_current_odom()
                .ok_or_else(|| EngineError::ExecutionFailed("Lost Sensor Signal".into()))?;

            let dx = current.x - start.x;
            let dy = current.y - start.y;
            let dist = (dx * dx + dy * dy).sqrt();

            let _ = bus.send(SystemEvent::with_progress("Moving...".into(), dist, target));

            if dist >= target {
                break;
            }

            ctx.publish_velocity(direction * self.speed, 0.0)
                .map_err(EngineError::ConnectionError)?;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        ctx.stop().map_err(EngineError::ConnectionError)?;
        let _ = bus.send(SystemEvent::new(EventType::Info, "Target Reached".into()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hal::test_profile;

    fn block(target_distance: f64, speed: f64) -> OdometryMoveBlock {
        OdometryMoveBlock {
            target_distance,
            speed,
        }
    }

    #[test]
    fn accepts_forward_move_within_limits() {
        assert!(block(1.0, 0.2).validate(&test_profile()).is_ok());
    }

    #[test]
    fn accepts_reverse_move_negative_distance() {
        // Direction comes from the distance sign — reverse is valid
        assert!(block(-1.0, 0.2).validate(&test_profile()).is_ok());
    }

    #[test]
    fn rejects_zero_distance() {
        assert!(block(0.0, 0.2).validate(&test_profile()).is_err());
    }

    #[test]
    fn rejects_zero_speed_would_loop_forever() {
        assert!(block(1.0, 0.0).validate(&test_profile()).is_err());
    }

    #[test]
    fn rejects_negative_speed_speed_is_a_magnitude() {
        assert!(block(1.0, -0.2).validate(&test_profile()).is_err());
    }

    #[test]
    fn rejects_speed_above_actuator_limit() {
        let err = block(1.0, 0.5).validate(&test_profile()).unwrap_err();
        assert!(err.contains("exceeds"), "unexpected message: {err}");
    }

    #[test]
    fn rejects_non_finite_parameters() {
        assert!(block(f64::NAN, 0.2).validate(&test_profile()).is_err());
        assert!(block(1.0, f64::INFINITY).validate(&test_profile()).is_err());
    }

    #[test]
    fn rejects_joint_position_profiles() {
        // Cross-mode guard: odometry moves make no sense on a manipulator
        let err = block(1.0, 0.2)
            .validate(&crate::hal::test_arm_profile())
            .unwrap_err();
        assert!(err.contains("joint-position"), "unexpected message: {err}");
    }
}
