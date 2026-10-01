use crate::blocks::BlockRunnable;
use crate::error::EngineError;
use crate::events::{EventType, SystemEvent};
use crate::robot::RobotContext;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// Log block — sends a user-defined message to the WebSocket terminal.
/// No physical robot commands are issued.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogBlock {
    pub message: String,
}

#[async_trait]
impl BlockRunnable for LogBlock {
    async fn execute(
        &self,
        _ctx: &mut RobotContext,
        event_bus: &broadcast::Sender<SystemEvent>,
        _cancel: &CancellationToken,
    ) -> Result<(), EngineError> {
        tracing::info!("SYSTEM LOG: {}", self.message);
        let _ = event_bus.send(SystemEvent::new(EventType::Info, self.message.clone()));
        Ok(())
    }
}
