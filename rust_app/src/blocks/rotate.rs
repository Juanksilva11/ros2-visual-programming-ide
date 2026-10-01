use crate::blocks::BlockRunnable;
use crate::error::EngineError;
use crate::events::{EventType, SystemEvent};
use crate::hal::{ControlMode, RobotProfile};
use crate::robot::RobotContext;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// Rotation block (closed-loop via odometry).
///
/// Uses a proportional controller to rotate the robot by the specified
/// angle in degrees. Feedback comes from the HAL's odometry stream.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RotateBlock {
    pub angle_degrees: f64,
    pub speed: f64,
}

impl RotateBlock {
    /// Normalizes an angle to the range [-PI, PI].
    fn normalize_angle(angle: f64) -> f64 {
        let mut a = angle;
        while a > PI {
            a -= 2.0 * PI;
        }
        while a < -PI {
            a += 2.0 * PI;
        }
        a
    }
}

#[async_trait]
impl BlockRunnable for RotateBlock {
    fn validate(&self, profile: &RobotProfile) -> Result<(), String> {
        if profile.control_mode != ControlMode::Velocity {
            return Err(format!(
                "Rotation requires a velocity-controlled robot; the active profile '{}' is joint-position controlled",
                profile.name
            ));
        }
        if !self.angle_degrees.is_finite() || self.angle_degrees == 0.0 {
            return Err(format!(
                "angle_degrees must be a non-zero number (got {})",
                self.angle_degrees
            ));
        }
        if !self.speed.is_finite() || self.speed <= 0.0 {
            return Err(format!(
                "speed must be a positive number (got {})",
                self.speed
            ));
        }
        if self.speed > profile.max_angular_speed {
            return Err(format!(
                "speed {} rad/s exceeds the {} limit of {} rad/s",
                self.speed, profile.name, profile.max_angular_speed
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
        let target_rad = self.angle_degrees.to_radians();

        // Wait for an initial orientation reading (max ~1 s)
        let start_odom = {
            let mut attempts = 0;
            loop {
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
            }
        };

        let target_yaw = Self::normalize_angle(start_odom.yaw + target_rad);

        // P-controller loop
        loop {
            if cancel.is_cancelled() {
                ctx.stop().map_err(EngineError::ConnectionError)?;
                return Err(EngineError::Aborted);
            }

            let current = ctx
                .get_current_odom()
                .ok_or_else(|| EngineError::ExecutionFailed("Lost Sensor Signal".into()))?;

            let error = Self::normalize_angle(target_yaw - current.yaw);

            let progress = 1.0 - (error.abs() / target_rad.abs()).min(1.0);
            let _ = bus.send(SystemEvent::with_progress(
                "Rotating...".into(),
                progress,
                1.0,
            ));

            // Tolerance: ~1 degree
            if error.abs() < 0.02 {
                break;
            }

            let k_p = 1.5;
            let cmd_speed = (error * k_p).clamp(-self.speed, self.speed);

            let min_speed = 0.2;
            let final_speed = if cmd_speed.abs() < min_speed {
                min_speed * cmd_speed.signum()
            } else {
                cmd_speed
            };

            ctx.publish_velocity(0.0, final_speed)
                .map_err(EngineError::ConnectionError)?;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        ctx.stop().map_err(EngineError::ConnectionError)?;
        let _ = bus.send(SystemEvent::new(
            EventType::Info,
            "Rotation Completed".into(),
        ));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hal::test_profile;

    const EPS: f64 = 1e-9;

    fn block(angle_degrees: f64, speed: f64) -> RotateBlock {
        RotateBlock {
            angle_degrees,
            speed,
        }
    }

    // ─── normalize_angle ────────────────────────────────────────────────

    #[test]
    fn normalize_keeps_angles_already_in_range() {
        assert!((RotateBlock::normalize_angle(0.0)).abs() < EPS);
        assert!((RotateBlock::normalize_angle(1.0) - 1.0).abs() < EPS);
        assert!((RotateBlock::normalize_angle(-1.0) + 1.0).abs() < EPS);
    }

    #[test]
    fn normalize_wraps_angles_beyond_pi() {
        // 3π/2 wraps to -π/2
        let wrapped = RotateBlock::normalize_angle(1.5 * PI);
        assert!((wrapped + 0.5 * PI).abs() < EPS, "got {wrapped}");
        // -3π/2 wraps to +π/2
        let wrapped = RotateBlock::normalize_angle(-1.5 * PI);
        assert!((wrapped - 0.5 * PI).abs() < EPS, "got {wrapped}");
    }

    #[test]
    fn normalize_handles_multiple_turns() {
        // 4π is two full turns → 0
        assert!(RotateBlock::normalize_angle(4.0 * PI).abs() < 1e-6);
        // 2π + π/4 → π/4
        let wrapped = RotateBlock::normalize_angle(2.0 * PI + 0.25 * PI);
        assert!((wrapped - 0.25 * PI).abs() < 1e-6, "got {wrapped}");
    }

    #[test]
    fn normalize_output_is_always_within_pi() {
        for i in -20..=20 {
            let angle = i as f64 * 0.7 * PI;
            let wrapped = RotateBlock::normalize_angle(angle);
            assert!((-PI..=PI).contains(&wrapped), "{angle} → {wrapped}");
        }
    }

    // ─── validate ───────────────────────────────────────────────────────

    #[test]
    fn accepts_both_rotation_directions() {
        assert!(block(90.0, 1.0).validate(&test_profile()).is_ok());
        // Negative angle = clockwise, valid
        assert!(block(-90.0, 1.0).validate(&test_profile()).is_ok());
    }

    #[test]
    fn rejects_zero_angle_division_by_zero_in_progress() {
        assert!(block(0.0, 1.0).validate(&test_profile()).is_err());
    }

    #[test]
    fn rejects_non_positive_speed() {
        assert!(block(90.0, 0.0).validate(&test_profile()).is_err());
        assert!(block(90.0, -1.0).validate(&test_profile()).is_err());
    }

    #[test]
    fn rejects_speed_above_actuator_limit() {
        // TestBot limit is 2.84 rad/s
        assert!(block(90.0, 3.0).validate(&test_profile()).is_err());
    }
}
