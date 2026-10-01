//! Camera management — web_video_server process lifecycle.

use axum::{Json, extract::State, response::IntoResponse};
use serde::Serialize;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::Arc;

use super::{ApiResponse, AppState};
use crate::error::AppError;

// ─── Process Management ─────────────────────────────────────────────────────

/// Kills the running `web_video_server` child process tree.
///
/// Uses `killpg` to terminate the entire process group (ros2 spawns children).
/// Async — uses `tokio::time::sleep` instead of blocking the runtime.
pub(super) async fn kill_video_server(state: &AppState) {
    let mut proc = state.video_server_process.lock().await;
    if let Some(ref mut child) = *proc {
        let pid = child.id() as i32;
        // Kill entire process group — ros2 run spawns child processes
        unsafe {
            libc::killpg(pid, libc::SIGTERM);
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        unsafe {
            libc::killpg(pid, libc::SIGKILL);
        }
        let _ = child.wait();
        tracing::info!("Stopped web_video_server process tree (pgid={})", pid);
    }
    *proc = None;
}

// ─── Response Types ─────────────────────────────────────────────────────────

#[derive(Serialize)]
struct CameraStatusResponse {
    has_camera: bool,
    topic: Option<String>,
    server_running: bool,
    stream_url: Option<String>,
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// Returns the camera status for the active profile.
pub async fn status_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let (topic, has_camera) = {
        let ctx = state.robot_ctx.lock().await;
        let t = ctx.profile().camera_topic.clone();
        let has = t.is_some();
        (t, has)
    };

    let server_running = state.video_server_process.lock().await.is_some();

    let stream_url = if server_running {
        topic.as_ref().map(|t| {
            format!(
                "http://{{host}}:{}/stream?topic={}",
                state.video_server_port, t
            )
        })
    } else {
        None
    };

    Ok(Json(CameraStatusResponse {
        has_camera,
        topic,
        server_running,
        stream_url,
    }))
}

/// Spawns the `web_video_server` process for the active profile's camera topic.
pub async fn start_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let topic = state.robot_ctx.lock().await.profile().camera_topic.clone();

    let Some(_topic) = topic else {
        return Ok(Json(ApiResponse {
            status: "error".into(),
            message: "Active profile has no camera topic".into(),
        }));
    };

    // Kill any existing instance first
    kill_video_server(&state).await;

    let port = state.video_server_port;
    // SAFETY: setsid() creates a new process group so killpg can reach all children
    let child = unsafe {
        Command::new("ros2")
            .args([
                "run",
                "web_video_server",
                "web_video_server",
                "--ros-args",
                "-p",
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
            tracing::info!("Started web_video_server on port {}", port);
            let mut proc = state.video_server_process.lock().await;
            *proc = Some(c);
            Ok(Json(ApiResponse {
                status: "ok".into(),
                message: format!("web_video_server started on port {}", port),
            }))
        }
        Err(e) => {
            tracing::error!("Failed to start web_video_server: {}", e);
            Ok(Json(ApiResponse {
                status: "error".into(),
                message: format!("Failed to start: {}", e),
            }))
        }
    }
}

/// Stops the running `web_video_server` process.
pub async fn stop_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    kill_video_server(&state).await;
    Ok(Json(ApiResponse {
        status: "ok".into(),
        message: "web_video_server stopped".into(),
    }))
}
