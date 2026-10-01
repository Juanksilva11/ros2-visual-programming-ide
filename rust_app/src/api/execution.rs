//! Execution handlers — program execution and emergency stop.

use axum::{Json, extract::State, response::IntoResponse};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use super::{ApiResponse, AppState, EngineCommand};
use crate::engine::ProgramSequence;
use crate::error::{AppError, EngineError};

/// Receives a compiled block sequence from the frontend and sends it
/// to the engine actor for execution.
#[tracing::instrument(skip(state, payload))]
pub async fn execute_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ProgramSequence>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("Received execution request for program: {}", payload.name);

    // Marker T1 of the latency metric: request arrival, stamped by the
    // bag in the DDS clock domain (see metrics.rs)
    if let Some(m) = &state.markers {
        m.emit(
            "exec_request",
            serde_json::json!({ "name": payload.name, "steps": payload.steps.len() }),
        );
    }

    let (tx, rx) = tokio::sync::oneshot::channel();
    let cancel = CancellationToken::new();

    // Store the cancel token so the abort endpoint can trigger it
    {
        let mut guard = state.active_cancel.lock().unwrap();
        *guard = Some(cancel.clone());
    }

    // Grab the CURRENT robot context (reflects the active profile)
    let ctx = {
        let guard = state.robot_ctx.lock().await;
        guard.clone_ref()
    };

    let cmd = EngineCommand::Run {
        program: payload,
        ctx,
        responder: tx,
        cancel,
    };

    if state.engine_tx.send(cmd).await.is_err() {
        return Err(AppError::InternalError(anyhow::anyhow!(
            "Failed to send command to Engine"
        )));
    }

    let result = match rx.await {
        Ok(Ok(success_msg)) => {
            tracing::info!("Execution finished successfully: {}", success_msg);
            Ok(Json(ApiResponse {
                status: "success".to_string(),
                message: success_msg,
            }))
        }
        Ok(Err(EngineError::Aborted)) => {
            tracing::info!("Execution was aborted by user");
            Ok(Json(ApiResponse {
                status: "aborted".to_string(),
                message: "Program aborted by user".to_string(),
            }))
        }
        Ok(Err(engine_error)) => {
            tracing::warn!("Execution failed with engine error: {:?}", engine_error);
            Err(AppError::EngineError(engine_error))
        }
        Err(_) => {
            tracing::error!("Engine responder channel closed unexpectedly");
            Err(AppError::InternalError(anyhow::anyhow!(
                "Engine responder dropped"
            )))
        }
    };

    // Clear active cancel token
    {
        let mut guard = state.active_cancel.lock().unwrap();
        *guard = None;
    }

    result
}

/// Emergency stop — cancels the running program and sends zero velocity.
pub async fn abort_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("Abort requested by user");

    // Marker T1 of the E-STOP determinism metric: abort request arrival.
    // T2 is the first zero-velocity /cmd_vel (or hold trajectory) in the bag.
    if let Some(m) = &state.markers {
        m.emit("abort_request", serde_json::json!({}));
    }

    // Cancel the active execution
    let cancelled = {
        let guard = state.active_cancel.lock().unwrap();
        if let Some(token) = guard.as_ref() {
            token.cancel();
            true
        } else {
            false
        }
    };

    // Send zero velocity via the HAL as a safety measure
    {
        let mut ctx = state.robot_ctx.lock().await;
        let _ = ctx.stop();
    }

    if cancelled {
        Ok(Json(ApiResponse {
            status: "ok".to_string(),
            message: "Execution aborted, robot stopped".to_string(),
        }))
    } else {
        Ok(Json(ApiResponse {
            status: "ok".to_string(),
            message: "No active execution to abort".to_string(),
        }))
    }
}
