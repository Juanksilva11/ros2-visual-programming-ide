use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

/// Unified Application Error for routing
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Internal Engine Error: {0}")]
    EngineError(#[from] EngineError),

    #[error("ROS 2 Communication Error: {0}")]
    RosError(String),

    #[error("Internal Server Error")]
    InternalError(#[from] anyhow::Error),
}

/// Unified Engine Error for Blocks and Execution paths
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("Failed to connect to ROS Topic: {0}")]
    ConnectionError(String),

    #[error("Invalid Argument: {0}")]
    InvalidArgument(String),

    #[error("Execution Failed: {0}")]
    ExecutionFailed(String),

    #[error("Execution Aborted")]
    Aborted,
}

// How AppError translates to an Axum HTTP Response
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::EngineError(ref e) => (StatusCode::BAD_REQUEST, e.to_string()),
            AppError::RosError(ref e) => (StatusCode::SERVICE_UNAVAILABLE, e.to_string()),
            AppError::InternalError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
        };

        // Log the exact error via tracing
        tracing::error!("Request failed: {:?}", self);

        let body = Json(json!({
            "status": "error",
            "message": error_message,
        }));

        (status, body).into_response()
    }
}
