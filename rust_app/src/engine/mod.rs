use crate::blocks::{BlockRunnable, BlockType};
use crate::error::EngineError;
use crate::events::{EventType, SystemEvent};
use crate::hal::RobotProfile;
use crate::robot::RobotContext;
use serde::Deserialize;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize, Debug, Clone)]
pub struct ProgramSequence {
    pub name: String,
    pub steps: Vec<BlockType>,
}

/// Pre-flight semantic validation of a whole program against a robot
/// profile. Free function (no `RobotContext` required) so it can be
/// unit-tested without a live ROS node.
///
/// Returns the first violation, annotated with its step number and the
/// block's human-readable label.
pub fn validate_program(steps: &[BlockType], profile: &RobotProfile) -> Result<(), String> {
    let total = steps.len();
    for (i, block) in steps.iter().enumerate() {
        if let Err(reason) = block.validate(profile) {
            return Err(format!(
                "Validation failed at step {}/{} ({}): {}",
                i + 1,
                total,
                block.label(),
                reason
            ));
        }
    }
    Ok(())
}

pub struct ExecutionEngine;

impl ExecutionEngine {
    pub async fn run_program(
        program: ProgramSequence,
        mut ctx: RobotContext,
        event_bus: broadcast::Sender<SystemEvent>,
        cancel: CancellationToken,
    ) -> Result<String, EngineError> {
        tracing::info!("Engine starting execution of program: {}", program.name);

        let total_steps = program.steps.len();

        // Pre-flight semantic validation of the WHOLE program against the
        // active robot profile. Runs before any velocity command is
        // published, so a bad parameter in step N can never leave the robot
        // stranded mid-trajectory at step N-1.
        if let Err(msg) = validate_program(&program.steps, ctx.profile()) {
            tracing::warn!("{}", msg);
            let _ = event_bus.send(SystemEvent::new(EventType::Error, msg.clone()));
            return Err(EngineError::InvalidArgument(msg));
        }

        // Notify program start
        let _ = event_bus.send(SystemEvent::new(
            EventType::Info,
            format!("Program Started: {}", program.name),
        ));

        for (i, block) in program.steps.iter().enumerate() {
            // Check cancellation before each step
            if cancel.is_cancelled() {
                ctx.stop().ok();
                let _ = event_bus.send(SystemEvent::new(
                    EventType::ProgramAbort,
                    "Program Aborted".to_string(),
                ));
                return Err(EngineError::Aborted);
            }

            let step_num = i + 1;
            let label = block.label();
            tracing::debug!("Executing Step {}/{}: {}", step_num, total_steps, label);

            // Consolidated step start event with metadata
            let mut event = SystemEvent::new(
                EventType::StepStart,
                format!("Step {}/{} — {}", step_num, total_steps, label),
            );
            event.metadata = Some(serde_json::json!({
                "step": step_num,
                "total": total_steps,
            }));
            let _ = event_bus.send(event);

            // Execute the block with cancellation support
            match block.execute(&mut ctx, &event_bus, &cancel).await {
                Ok(_) => {
                    tracing::debug!("Step {} completed successfully", step_num);
                    let _ = event_bus.send(SystemEvent::new(
                        EventType::StepFinish,
                        format!("Step {} Completed", step_num),
                    ));
                }
                Err(EngineError::Aborted) => {
                    tracing::warn!("Program aborted during step {}", step_num);
                    ctx.stop().ok();
                    let _ = event_bus.send(SystemEvent::new(
                        EventType::ProgramAbort,
                        "Program Aborted".to_string(),
                    ));
                    return Err(EngineError::Aborted);
                }
                Err(e) => {
                    tracing::error!("Step {} failed with error: {}", step_num, e);
                    let _ = event_bus.send(SystemEvent::new(
                        EventType::Error,
                        format!("Step {} Failed: {}", step_num, e),
                    ));
                    return Err(e);
                }
            }
        }

        tracing::info!("Engine successfully finished program: {}", program.name);
        let _ = event_bus.send(SystemEvent::new(
            EventType::ProgramFinish,
            "Sequence Completed".to_string(),
        ));
        Ok("Sequence completed successfully".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hal::test_profile;

    // ─── ProgramSequence deserialization (corrupt-JSON prevention) ──────

    #[test]
    fn deserializes_a_valid_program() {
        let json = r#"{
            "name": "Test Program",
            "steps": [
                { "type": "move_odometry", "target_distance": 1.0, "speed": 0.2 },
                { "type": "rotate", "angle_degrees": 90.0, "speed": 1.0 },
                { "type": "wait", "duration_ms": 500 },
                { "type": "open_loop", "linear_x": 0.1, "angular_z": 0.0, "duration_ms": 1000 },
                { "type": "log", "message": "done" }
            ]
        }"#;
        let program: ProgramSequence = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(program.steps.len(), 5);
        assert!(matches!(program.steps[0], BlockType::SmartMove(_)));
        assert!(matches!(program.steps[1], BlockType::Rotate(_)));
        assert!(matches!(program.steps[2], BlockType::Wait(_)));
        assert!(matches!(program.steps[3], BlockType::OpenLoop(_)));
        assert!(matches!(program.steps[4], BlockType::Log(_)));
    }

    #[test]
    fn rejects_unknown_block_type_as_typed_error_not_panic() {
        let json = r#"{ "name": "Bad", "steps": [ { "type": "teleport", "x": 1.0 } ] }"#;
        assert!(serde_json::from_str::<ProgramSequence>(json).is_err());
    }

    #[test]
    fn rejects_missing_required_fields() {
        // move_odometry without speed
        let json = r#"{ "name": "Bad", "steps": [ { "type": "move_odometry", "target_distance": 1.0 } ] }"#;
        assert!(serde_json::from_str::<ProgramSequence>(json).is_err());
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(serde_json::from_str::<ProgramSequence>("{ not json").is_err());
    }

    // ─── Pre-flight program validation ──────────────────────────────────

    fn valid_steps() -> Vec<BlockType> {
        serde_json::from_str::<ProgramSequence>(
            r#"{
                "name": "ok",
                "steps": [
                    { "type": "move_odometry", "target_distance": 1.0, "speed": 0.2 },
                    { "type": "wait", "duration_ms": 100 },
                    { "type": "rotate", "angle_degrees": -90.0, "speed": 1.0 }
                ]
            }"#,
        )
        .unwrap()
        .steps
    }

    #[test]
    fn validates_a_correct_program() {
        assert!(validate_program(&valid_steps(), &test_profile()).is_ok());
    }

    #[test]
    fn empty_program_is_semantically_valid_at_engine_level() {
        // The frontend compiler rejects empty programs (EMPTY_PROGRAM);
        // at this layer an empty list simply has nothing to invalidate.
        assert!(validate_program(&[], &test_profile()).is_ok());
    }

    #[test]
    fn reports_the_failing_step_number_and_label() {
        let mut steps = valid_steps();
        // Corrupt the SECOND step (wait is always valid, swap in a bad move)
        steps[1] = serde_json::from_str(
            r#"{ "type": "move_odometry", "target_distance": 1.0, "speed": 0.0 }"#,
        )
        .unwrap();
        let err = validate_program(&steps, &test_profile()).unwrap_err();
        assert!(err.contains("step 2/3"), "unexpected message: {err}");
        assert!(err.contains("Linear Move"), "unexpected message: {err}");
    }

    #[test]
    fn rejects_speed_above_profile_limit_anywhere_in_the_program() {
        let mut steps = valid_steps();
        steps.push(
            serde_json::from_str(
                r#"{ "type": "open_loop", "linear_x": 5.0, "angular_z": 0.0, "duration_ms": 500 }"#,
            )
            .unwrap(),
        );
        let err = validate_program(&steps, &test_profile()).unwrap_err();
        assert!(err.contains("exceeds"), "unexpected message: {err}");
    }
}
