pub mod joint_move;
pub mod log;
pub mod movement;
pub mod open_loop;
pub mod rotate;
pub mod wait;

use crate::error::EngineError;
use crate::events::SystemEvent;
use crate::hal::RobotProfile;
use crate::robot::RobotContext;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use joint_move::JointMoveBlock;
use log::LogBlock;
use movement::OdometryMoveBlock;
use open_loop::OpenLoopBlock;
use rotate::RotateBlock;
use wait::WaitBlock;

/// Trait implemented by every execution block.
///
/// Each block receives the shared robot context, an event bus for
/// real-time WebSocket feedback, and a cancellation token for
/// cooperative emergency stop.
#[async_trait]
pub trait BlockRunnable {
    /// Semantic validation of block parameters against the active robot
    /// profile. Called by the engine for EVERY step before ANY motion
    /// starts, so an invalid parameter can never stop a program halfway
    /// through a trajectory. Returns a human-readable reason on failure.
    ///
    /// This is the authoritative validation layer: the frontend compiler
    /// performs the same generic checks for fast feedback, but the API
    /// can be called directly (e.g. curl), so the backend cannot trust it.
    fn validate(&self, _profile: &RobotProfile) -> Result<(), String> {
        Ok(())
    }

    async fn execute(
        &self,
        ctx: &mut RobotContext,
        event_bus: &broadcast::Sender<SystemEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), EngineError>;
}

/// Tagged enum for JSON deserialization of block payloads.
///
/// The `type` field in the incoming JSON determines which variant is
/// deserialized — the frontend compiler sets this tag automatically.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum BlockType {
    #[serde(rename = "move_odometry")]
    SmartMove(OdometryMoveBlock),
    #[serde(rename = "wait")]
    Wait(WaitBlock),
    #[serde(rename = "log")]
    Log(LogBlock),
    #[serde(rename = "rotate")]
    Rotate(RotateBlock),
    #[serde(rename = "open_loop")]
    OpenLoop(OpenLoopBlock),
    #[serde(rename = "joint_move")]
    JointMove(JointMoveBlock),
}

impl BlockType {
    /// Human-readable label used in consolidated terminal output.
    pub fn label(&self) -> String {
        match self {
            BlockType::SmartMove(b) => format!("Linear Move: {}m", b.target_distance),
            BlockType::Wait(b) => format!("Wait: {:.1}s", b.duration_ms as f64 / 1000.0),
            BlockType::Log(b) => format!("Log: {}", b.message),
            BlockType::Rotate(b) => format!("Rotation: {} deg", b.angle_degrees),
            BlockType::OpenLoop(b) => {
                format!(
                    "Open-Loop: lin={:.2}, ang={:.2}, {:.1}s",
                    b.linear_x,
                    b.angular_z,
                    b.duration_ms as f64 / 1000.0
                )
            }
            BlockType::JointMove(b) => {
                let positions = b
                    .positions
                    .iter()
                    .map(|p| format!("{:.2}", p))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "Joint Move: [{}] rad, {:.1}s",
                    positions,
                    b.duration_ms as f64 / 1000.0
                )
            }
        }
    }
}

#[async_trait]
impl BlockRunnable for BlockType {
    fn validate(&self, profile: &RobotProfile) -> Result<(), String> {
        match self {
            BlockType::SmartMove(b) => b.validate(profile),
            BlockType::Wait(b) => b.validate(profile),
            BlockType::Log(b) => b.validate(profile),
            BlockType::Rotate(b) => b.validate(profile),
            BlockType::OpenLoop(b) => b.validate(profile),
            BlockType::JointMove(b) => b.validate(profile),
        }
    }

    async fn execute(
        &self,
        ctx: &mut RobotContext,
        event_bus: &broadcast::Sender<SystemEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), EngineError> {
        match self {
            BlockType::SmartMove(b) => b.execute(ctx, event_bus, cancel).await,
            BlockType::Wait(b) => b.execute(ctx, event_bus, cancel).await,
            BlockType::Log(b) => b.execute(ctx, event_bus, cancel).await,
            BlockType::Rotate(b) => b.execute(ctx, event_bus, cancel).await,
            BlockType::OpenLoop(b) => b.execute(ctx, event_bus, cancel).await,
            BlockType::JointMove(b) => b.execute(ctx, event_bus, cancel).await,
        }
    }
}
