//! Robot profile management — list, query, and switch active robot profiles.

use axum::{Json, extract::State, response::IntoResponse};
use serde::Deserialize;
use std::sync::Arc;

use super::{ApiResponse, AppState};
use crate::error::AppError;
use crate::hal;

/// Lists all registered robot profiles.
pub async fn list_handler() -> Result<impl IntoResponse, AppError> {
    Ok(Json(hal::get_all_profiles()))
}

/// Returns the currently active robot profile.
pub async fn active_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let ctx = state.robot_ctx.lock().await;
    Ok(Json(ctx.profile().clone()))
}

#[derive(Deserialize)]
pub struct SelectProfileRequest {
    pub id: String,
}

/// Switches the active robot profile.
///
/// Auto-stops the camera and LiDAR servers if the new profile
/// does not support those sensors.
pub async fn select_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SelectProfileRequest>,
) -> Result<impl IntoResponse, AppError> {
    let profile = hal::get_profile_by_id(&body.id).ok_or_else(|| {
        AppError::EngineError(crate::error::EngineError::ExecutionFailed(format!(
            "Unknown profile: {}",
            body.id
        )))
    })?;

    // Auto-stop video server if the new profile has no camera
    if profile.camera_topic.is_none() {
        super::camera::kill_video_server(&state).await;
    }
    // Auto-stop rosbridge if the new profile has no LiDAR
    if profile.lidar_topic.is_none() {
        super::lidar::kill_rosbridge(&state).await;
    }

    let node = state.shared_node.clone();
    {
        let mut ctx = state.robot_ctx.lock().await;
        ctx.switch_profile(profile.clone(), node);

        // Manipulators: create the trajectory publisher NOW so DDS
        // discovery completes long before the first RUN (JointTrajectory
        // is a one-shot message and would be lost against an unmatched
        // subscriber).
        if profile.control_mode == crate::hal::ControlMode::JointPosition {
            let matched = ctx
                .wait_for_joint_subscribers(std::time::Duration::from_secs(2))
                .await;
            if !matched {
                tracing::warn!(
                    "Joint trajectory controller not discovered yet for {}",
                    profile.name
                );
            }
        }
    }

    tracing::info!("Switched active robot profile to: {}", profile.name);

    Ok(Json(ApiResponse {
        status: "ok".into(),
        message: format!("Switched to {}", profile.name),
    }))
}
