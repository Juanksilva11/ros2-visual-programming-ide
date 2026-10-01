//! API module — route handlers organized by domain.
//!
//! Each sub-module owns a single responsibility:
//! - `ws`            — WebSocket event stream
//! - `execution`     — program execution & emergency stop
//! - `introspection` — live ROS 2 graph discovery
//! - `profiles`      — robot profile management
//! - `camera`        — web_video_server process lifecycle
//! - `lidar`         — rosbridge_server process lifecycle

pub mod camera;
pub mod execution;
pub mod introspection;
pub mod lidar;
pub mod profiles;
pub mod ws;

use axum::{
    Router,
    routing::{get, post},
};
use serde::Serialize;
use std::process::Child;
use std::sync::Arc;
use tokio::sync::{Mutex as AsyncMutex, broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::error::EngineError;
use crate::engine::ProgramSequence;
use crate::events::SystemEvent;
use crate::robot::RobotContext;

// ─── Shared Types ───────────────────────────────────────────────────────────

/// Command sent from the API layer to the engine actor.
pub enum EngineCommand {
    Run {
        program: ProgramSequence,
        ctx: RobotContext,
        responder: tokio::sync::oneshot::Sender<Result<String, EngineError>>,
        cancel: CancellationToken,
    },
}

/// Generic JSON response body used across multiple handlers.
#[derive(Serialize)]
pub(crate) struct ApiResponse {
    pub status: String,
    pub message: String,
}

/// Shared application state, accessible by all handlers via `State<Arc<AppState>>`.
pub struct AppState {
    pub engine_tx: mpsc::Sender<EngineCommand>,
    pub event_bus: broadcast::Sender<SystemEvent>,
    /// Experiment markers for the measurement campaign (None if the
    /// publisher could not be created — the platform runs without metrics).
    pub markers: Option<crate::metrics::ExperimentMarkers>,
    /// Robot context — uses `tokio::sync::Mutex` because handlers may hold
    /// the guard across `.await` points as the codebase grows.
    pub robot_ctx: AsyncMutex<RobotContext>,
    pub shared_node: Arc<std::sync::Mutex<r2r::Node>>,
    /// Holds the cancellation token of the currently running execution, if any.
    /// Uses `std::sync::Mutex` — quick lock/unlock, never crosses `.await`.
    pub active_cancel: std::sync::Mutex<Option<CancellationToken>>,
    /// `web_video_server` child process, if running.
    /// Uses `tokio::sync::Mutex` — kill functions need async sleep.
    pub video_server_process: AsyncMutex<Option<Child>>,
    /// Port the video server listens on.
    pub video_server_port: u16,
    /// `rosbridge_server` child process, if running.
    /// Uses `tokio::sync::Mutex` — kill functions need async sleep.
    pub rosbridge_process: AsyncMutex<Option<Child>>,
    /// Port the rosbridge WebSocket listens on.
    pub rosbridge_port: u16,
}

// ─── Router ─────────────────────────────────────────────────────────────────

/// Builds the complete API router with all routes registered.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        // Execution
        .route("/api/execute", post(execution::execute_handler))
        .route("/api/abort", post(execution::abort_handler))
        // Introspection
        .route("/api/introspect", get(introspection::introspect_handler))
        .route(
            "/api/introspect/topic/{*name}",
            get(introspection::topic_detail_handler),
        )
        .route(
            "/api/introspect/service/{*name}",
            get(introspection::service_detail_handler),
        )
        .route(
            "/api/introspect/action/{*name}",
            get(introspection::action_detail_handler),
        )
        // Robot Profiles
        .route("/api/profiles", get(profiles::list_handler))
        .route("/api/profiles/active", get(profiles::active_handler))
        .route("/api/profiles/select", post(profiles::select_handler))
        // Camera
        .route("/api/camera/status", get(camera::status_handler))
        .route("/api/camera/start", post(camera::start_handler))
        .route("/api/camera/stop", post(camera::stop_handler))
        // LiDAR
        .route("/api/lidar/status", get(lidar::status_handler))
        .route("/api/lidar/start", post(lidar::start_handler))
        .route("/api/lidar/stop", post(lidar::stop_handler))
        // WebSocket
        .route("/ws", get(ws::websocket_handler))
        .with_state(state)
}
