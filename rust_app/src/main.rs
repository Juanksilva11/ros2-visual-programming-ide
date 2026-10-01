use rust_app::api::{self, AppState, EngineCommand};
use rust_app::engine::ExecutionEngine;
use rust_app::events::SystemEvent;
use rust_app::hal;
use rust_app::robot::RobotContext;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex as AsyncMutex, broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Initialize Tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("RUST APP: Real-Time Fleet Management System Initializing...");

    let cancel_token = CancellationToken::new();

    // 1. ROS Setup
    let ctx = r2r::Context::create()?;
    let node = r2r::Node::create(ctx, "rust_app_node", "")?;
    let shared_node = Arc::new(std::sync::Mutex::new(node));

    // 2. Robot Context — initialized with the default profile (Turtlesim)
    let default_profile = hal::get_all_profiles()
        .into_iter()
        .next()
        .expect("At least one robot profile must be registered");
    tracing::info!(
        "Default robot profile: {} ({})",
        default_profile.name,
        default_profile.id
    );
    let robot_context = RobotContext::new(shared_node.clone(), default_profile);

    // Spin Thread
    let spin_node = shared_node.clone();
    let spin_token = cancel_token.clone();
    std::thread::spawn(move || {
        tracing::debug!("ROS 2 Spin Thread started.");
        loop {
            if spin_token.is_cancelled() {
                tracing::info!("ROS 2 Spin Thread shutting down.");
                break;
            }
            if let Ok(mut node_guard) = spin_node.lock() {
                node_guard.spin_once(Duration::from_millis(0));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    // 3. Channels Setup
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<EngineCommand>(32);
    let (event_tx, _event_rx) = broadcast::channel::<SystemEvent>(100);

    // 3b. Experiment markers — bridge every system event onto a ROS topic
    // so rosbag2 records platform events in the same clock domain as
    // /cmd_vel and odometry (measurement campaign, thesis Ch. 6).
    let markers = rust_app::metrics::ExperimentMarkers::new(&shared_node);
    if let Some(m) = markers.clone() {
        let mut marker_rx = event_tx.subscribe();
        let marker_token = cancel_token.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = marker_token.cancelled() => break,
                    ev = marker_rx.recv() => match ev {
                        Ok(event) => m.emit_system_event(&event),
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            }
        });
    }

    // 4. Engine Actor
    let event_tx_clone = event_tx.clone();
    let engine_token = cancel_token.clone();
    tokio::spawn(async move {
        tracing::debug!("Engine Actor started.");
        loop {
            tokio::select! {
                _ = engine_token.cancelled() => {
                    tracing::info!("Engine Actor shutting down.");
                    break;
                }
                cmd_opt = cmd_rx.recv() => {
                    if let Some(cmd) = cmd_opt {
                        match cmd {
                            EngineCommand::Run { program, ctx, responder, cancel } => {
                                let result =
                                    ExecutionEngine::run_program(program, ctx, event_tx_clone.clone(), cancel)
                                        .await;
                                let _ = responder.send(result);
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    });

    // 5. Web Server State
    let state = Arc::new(AppState {
        engine_tx: cmd_tx,
        event_bus: event_tx,
        markers,
        robot_ctx: AsyncMutex::new(robot_context),
        shared_node: shared_node.clone(),
        active_cancel: std::sync::Mutex::new(None),
        video_server_process: AsyncMutex::new(None),
        video_server_port: 8080,
        rosbridge_process: AsyncMutex::new(None),
        rosbridge_port: 9090,
    });

    // 6. Router (routes are registered inside api::router)
    let app = api::router(state).layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on http://0.0.0.0:3000 (WebSocket enabled)");
    let listener = tokio::net::TcpListener::bind(addr).await?;

    let server_token = cancel_token.clone();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        server_token.cancelled().await;
        tracing::info!("Axum Server shutting down.");
    });

    tokio::spawn(async move {
        if let Err(e) = server.await {
            tracing::error!("Server error: {}", e);
        }
    });

    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for event");

    tracing::info!("Shutdown signal received. Cancelling tasks...");
    cancel_token.cancel();
    tokio::time::sleep(Duration::from_millis(500)).await;
    tracing::info!("System gracefully shut down.");

    Ok(())
}
