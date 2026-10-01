use crate::blocks::BlockRunnable;
use crate::error::EngineError;
use crate::events::SystemEvent;
use crate::robot::RobotContext;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// Wait block — pauses execution for a specified duration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WaitBlock {
    pub duration_ms: u64,
}

#[async_trait]
impl BlockRunnable for WaitBlock {
    async fn execute(
        &self,
        _ctx: &mut RobotContext,
        _event_bus: &broadcast::Sender<SystemEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), EngineError> {
        tracing::debug!("Waiting {}ms...", self.duration_ms);
        tokio::select! {
            _ = cancel.cancelled() => {
                return Err(EngineError::Aborted);
            }
            _ = tokio::time::sleep(Duration::from_millis(self.duration_ms)) => {}
        }
        Ok(())
    }
}
