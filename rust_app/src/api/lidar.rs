//! LiDAR management — rosbridge_server process lifecycle.

use axum::{Json, extract::State, response::IntoResponse};
use serde::Serialize;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::Arc;

use super::{ApiResponse, AppState};
use crate::error::AppError;

// ─── Process Management ─────────────────────────────────────────────────────

/// Kills the running `rosbridge_server` child process tree.
///
/// Uses `killpg` to terminate the entire process group (ros2 launch spawns children).
/// Async — uses `tokio::time::sleep` instead of blocking the runtime.
pub(super) async fn kill_rosbridge(state: &AppState) {
    let mut proc = state.rosbridge_process.lock().await;
    if let Some(ref mut child) = *proc {
        let pid = child.id() as i32;
        unsafe {
            libc::killpg(pid, libc::SIGTERM);
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        unsafe {
            libc::killpg(pid, libc::SIGKILL);
        }
        let _ = child.wait();
        tracing::info!("Stopped rosbridge_server process tree (pgid={})", pid);
    }
    *proc = None;
}

// ─── Response Types ─────────────────────────────────────────────────────────

#[derive(Serialize)]
struct LidarStatusResponse {
    has_lidar: bool,
    topic: Option<String>,
    server_running: bool,
    rosbridge_url: Option<String>,
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// Returns the LiDAR status for the active profile.
pub async fn status_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let (topic, has_lidar) = {
        let ctx = state.robot_ctx.lock().await;
        let t = ctx.profile().lidar_topic.clone();
        let has = t.is_some();
        (t, has)
    };

    let server_running = state.rosbridge_process.lock().await.is_some();

    let rosbridge_url = if server_running {
        Some(format!("ws://{{host}}:{}", state.rosbridge_port))
    } else {
        None
    };

    Ok(Json(LidarStatusResponse {
        has_lidar,
        topic,
        server_running,
        rosbridge_url,
    }))
}

/// Spawns the `rosbridge_server` for the active profile's LiDAR topic.
pub async fn start_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let topic = state.robot_ctx.lock().await.profile().lidar_topic.clone();

    if topic.is_none() {
        return Ok(Json(ApiResponse {
            status: "error".into(),
            message: "Active profile has no LiDAR topic".into(),
        }));
    }

    // Kill any existing instance first
    kill_rosbridge(&state).await;

    let port = state.rosbridge_port;
    // SAFETY: setsid() creates a new process group so killpg can reach all children
    let child = unsafe {
        Command::new("ros2")
            .args([
                "launch",
                "rosbridge_server",
                "rosbridge_websocket_launch.xml",
                &format!("port:={}", port),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .pre_exec(|| {
                libc::setsid();
                Ok(())
            })
            .spawn()
    };

    match child {
        Ok(c) => {
            tracing::info!("Started rosbridge_server on port {}", port);
            let mut proc = state.rosbridge_process.lock().await;
            *proc = Some(c);
            Ok(Json(ApiResponse {
                status: "ok".into(),
                message: format!("rosbridge_server started on port {}", port),
            }))
        }
        Err(e) => {
            tracing::error!("Failed to start rosbridge_server: {}", e);
            Ok(Json(ApiResponse {
                status: "error".into(),
                message: format!("Failed to start: {}", e),
            }))
        }
    }
}

/// Stops the running `rosbridge_server` process.
pub async fn stop_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    kill_rosbridge(&state).await;
    Ok(Json(ApiResponse {
        status: "ok".into(),
        message: "rosbridge_server stopped".into(),
    }))
}
