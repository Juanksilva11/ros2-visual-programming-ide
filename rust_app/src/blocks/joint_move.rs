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

fn default_tolerance() -> f64 {
    0.05 // rad (~2.9°)
}

/// Joint-space motion block for manipulators (closed-loop via /joint_states).
///
/// Publishes a single-point JointTrajectory that the ros2_control
/// JointTrajectoryController interpolates over `duration_ms`, then waits
/// until every joint is within `tolerance` of its target. The motion
/// duration is part of the command, so the user controls the speed of
/// the gesture.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JointMoveBlock {
    /// Target joint positions in radians, ordered like `profile.joints`
    pub positions: Vec<f64>,
    /// Time the controller takes to interpolate to the target
    pub duration_ms: u64,
    /// Per-joint convergence tolerance in radians
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,
}

/// Extra time granted beyond the commanded duration before declaring
/// that the arm failed to converge.
const CONVERGENCE_GRACE: Duration = Duration::from_secs(3);

#[async_trait]
impl BlockRunnable for JointMoveBlock {
    fn validate(&self, profile: &RobotProfile) -> Result<(), String> {
        if profile.control_mode != ControlMode::JointPosition {
            return Err(format!(
                "Joint Move requires a joint-position controlled robot; the active profile '{}' is velocity-controlled",
                profile.name
            ));
        }
        if self.positions.len() != profile.joints.len() {
            return Err(format!(
                "expected {} joint positions for {}, got {}",
                profile.joints.len(),
                profile.name,
                self.positions.len()
            ));
        }
        // Mechanical limits per joint (mirrors the URDF <limit> tag)
        for (target, joint) in self.positions.iter().zip(profile.joints.iter()) {
            if !target.is_finite() || *target < joint.min_position || *target > joint.max_position {
                return Err(format!(
                    "{} target {} rad is outside its mechanical range [{}, {}] rad",
                    joint.label, target, joint.min_position, joint.max_position
                ));
            }
        }
        if self.duration_ms == 0 {
            return Err("duration_ms must be greater than zero".to_string());
        }
        if !self.tolerance.is_finite() || self.tolerance <= 0.0 {
            return Err(format!(
                "tolerance must be a positive number (got {})",
                self.tolerance
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
        // Trajectory commands are one-shot: make sure the controller has
        // discovered our publisher before sending, or the message is lost.
        if !ctx
            .wait_for_joint_subscribers(Duration::from_secs(2))
            .await
        {
            return Err(EngineError::ConnectionError(
                "No subscriber on the joint trajectory topic — is the arm controller running?"
                    .into(),
            ));
        }

        // Wait for an initial joint-state reading (max ~1 s)
        let mut attempts = 0;
        let start_joints = loop {
            if cancel.is_cancelled() {
                return Err(EngineError::Aborted);
            }
            if let Some(joints) = ctx.get_current_joints() {
                break joints;
            }
            if attempts > 10 {
                return Err(EngineError::ExecutionFailed("Sensor Timeout".into()));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            attempts += 1;
        };

        // Runtime velocity feasibility: now that the current position is
        // known, reject motions whose required joint speed exceeds the
        // URDF velocity limit (the controller would either violate it or
        // lag behind the requested duration).
        let duration_s = self.duration_ms as f64 / 1000.0;
        for ((target, current), joint) in self
            .positions
            .iter()
            .zip(start_joints.iter())
            .zip(ctx.profile().joints.iter())
        {
            let required_speed = (target - current).abs() / duration_s;
            if required_speed > joint.max_velocity {
                return Err(EngineError::InvalidArgument(format!(
                    "{}: moving {:.2} rad in {:.1}s requires {:.2} rad/s, above its {:.2} rad/s limit — increase the duration",
                    joint.label,
                    (target - current).abs(),
                    duration_s,
                    required_speed,
                    joint.max_velocity
                )));
            }
        }

        ctx.publish_joint_trajectory(self.positions.clone(), self.duration_ms)
            .map_err(EngineError::ConnectionError)?;

        // Closed-loop convergence watch
        let started = std::time::Instant::now();
        let deadline = Duration::from_millis(self.duration_ms) + CONVERGENCE_GRACE;
        loop {
            if cancel.is_cancelled() {
                // stop() publishes an empty trajectory: cancels the active
                // goal and holds the current position
                ctx.stop().map_err(EngineError::ConnectionError)?;
                return Err(EngineError::Aborted);
            }

            let current = ctx
                .get_current_joints()
                .ok_or_else(|| EngineError::ExecutionFailed("Lost Sensor Signal".into()))?;

            let worst_error = self
                .positions
                .iter()
                .zip(current.iter())
                .map(|(t, c)| (t - c).abs())
                .fold(0.0_f64, f64::max);

            let _ = bus.send(SystemEvent::with_progress(
                "Moving Joints...".into(),
                started.elapsed().as_secs_f64().min(duration_s),
                duration_s,
            ));

            if worst_error < self.tolerance {
                break;
            }

            if started.elapsed() > deadline {
                return Err(EngineError::ExecutionFailed(format!(
                    "Arm did not reach the target ({:.3} rad off after {:.1}s)",
                    worst_error,
                    started.elapsed().as_secs_f64()
                )));
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let _ = bus.send(SystemEvent::new(EventType::Info, "Joints In Position".into()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hal::{test_arm_profile, test_profile};

    fn block(positions: Vec<f64>, duration_ms: u64) -> JointMoveBlock {
        JointMoveBlock {
            positions,
            duration_ms,
            tolerance: default_tolerance(),
        }
    }

    #[test]
    #[allow(clippy::approx_constant)] // -3.14 is the literal URDF limit
    fn accepts_targets_within_mechanical_limits() {
        // elbow [0, 2.44], wrist [-3.14, 3.14]
        assert!(block(vec![1.2, 1.57], 3000).validate(&test_arm_profile()).is_ok());
        assert!(block(vec![0.0, -3.14], 2000).validate(&test_arm_profile()).is_ok());
    }

    #[test]
    fn rejects_targets_beyond_joint_limits() {
        // Elbow above its 2.44 rad flexion limit
        let err = block(vec![3.0, 0.0], 3000).validate(&test_arm_profile()).unwrap_err();
        assert!(err.contains("Elbow"), "unexpected message: {err}");
        // Elbow below its 0.0 rad extension limit
        assert!(block(vec![-0.5, 0.0], 3000).validate(&test_arm_profile()).is_err());
        // Wrist beyond pronation limit
        assert!(block(vec![1.0, 4.0], 3000).validate(&test_arm_profile()).is_err());
    }

    #[test]
    fn rejects_wrong_number_of_positions() {
        assert!(block(vec![1.0], 3000).validate(&test_arm_profile()).is_err());
        assert!(block(vec![1.0, 1.0, 1.0], 3000).validate(&test_arm_profile()).is_err());
    }

    #[test]
    fn rejects_zero_duration_and_bad_tolerance() {
        assert!(block(vec![1.0, 0.0], 0).validate(&test_arm_profile()).is_err());
        let mut b = block(vec![1.0, 0.0], 3000);
        b.tolerance = 0.0;
        assert!(b.validate(&test_arm_profile()).is_err());
    }

    #[test]
    fn rejects_non_finite_targets() {
        assert!(block(vec![f64::NAN, 0.0], 3000).validate(&test_arm_profile()).is_err());
    }

    #[test]
    fn rejects_velocity_controlled_profiles() {
        // Cross-mode guard: Joint Move on a mobile base makes no sense
        let err = block(vec![1.0, 0.0], 3000).validate(&test_profile()).unwrap_err();
        assert!(err.contains("velocity-controlled"), "unexpected message: {err}");
    }
}
