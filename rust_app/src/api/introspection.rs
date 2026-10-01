//! ROS 2 graph introspection — discovers live topics, services, and actions.

use axum::{Json, extract::Path, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

// ─── Response Types ─────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct IntrospectResponse {
    pub topics: Vec<String>,
    pub services: Vec<String>,
    pub actions: Vec<String>,
}

#[derive(Serialize)]
pub struct DetailResponse {
    pub info: String,
    pub interface: String,
}

// ─── CLI Helpers ────────────────────────────────────────────────────────────

/// Runs `ros2 <subcommand> list` and returns each non-empty line.
async fn run_ros2_command(subcommand: &str) -> Vec<String> {
    let output = tokio::process::Command::new("ros2")
        .arg(subcommand)
        .arg("list")
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect(),
        Ok(out) => {
            tracing::warn!(
                "ros2 {} list failed: {}",
                subcommand,
                String::from_utf8_lossy(&out.stderr)
            );
            vec![format!("Error running ros2 {} list", subcommand)]
        }
        Err(e) => {
            tracing::error!("Failed to execute ros2 CLI: {}", e);
            vec![format!("ros2 CLI not found: {}", e)]
        }
    }
}

/// Runs an arbitrary `ros2` command and returns stdout as a string.
async fn run_ros2_raw(args: &[&str]) -> String {
    let output = tokio::process::Command::new("ros2")
        .args(args)
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            tracing::warn!("ros2 {:?} failed: {}", args, stderr);
            format!("Error: {}", stderr.trim())
        }
        Err(e) => format!("ros2 CLI not found: {}", e),
    }
}

/// Set to true to use verbose (`-v`) output for topic/service info commands.
const VERBOSE_INFO: bool = false;

/// Extracts the message type from various `ros2` info outputs.
fn extract_type_from_info(info: &str) -> Option<String> {
    for line in info.lines() {
        let trimmed = line.trim();
        if let Some(t) = trimmed.strip_prefix("Type: ") {
            return Some(t.trim().to_string());
        }
        if let (Some(start), Some(end)) = (trimmed.find('['), trimmed.find(']'))
            && start < end
        {
            let inside = &trimmed[start + 1..end];
            if inside.contains('/') {
                return Some(inside.to_string());
            }
        }
        if trimmed.contains('/') && !trimmed.contains(' ') {
            return Some(trimmed.to_string());
        }
    }
    None
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// Lists all ROS 2 topics, services, and actions concurrently.
pub async fn introspect_handler() -> Result<impl IntoResponse, AppError> {
    tracing::info!("ROS 2 Graph Introspection requested.");

    let (topics, services, actions) = tokio::join!(
        run_ros2_command("topic"),
        run_ros2_command("service"),
        run_ros2_command("action"),
    );

    Ok(Json(IntrospectResponse {
        topics,
        services,
        actions,
    }))
}

/// Returns detailed info and interface definition for a specific topic.
pub async fn topic_detail_handler(Path(name): Path<String>) -> Result<impl IntoResponse, AppError> {
    let topic = format!("/{}", name);
    tracing::info!("Topic detail requested: {}", topic);

    let mut args = vec!["topic", "info"];
    if VERBOSE_INFO {
        args.push("-v");
    }
    args.push(&topic);

    let info = run_ros2_raw(&args).await;
    let iface = if let Some(msg_type) = extract_type_from_info(&info) {
        run_ros2_raw(&["interface", "show", &msg_type]).await
    } else {
        "Could not determine message type.".to_string()
    };

    Ok(Json(DetailResponse {
        info,
        interface: iface,
    }))
}

/// Returns detailed info and interface definition for a specific service.
pub async fn service_detail_handler(
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let svc = format!("/{}", name);
    tracing::info!("Service detail requested: {}", svc);

    let info = run_ros2_raw(&["service", "info", &svc]).await;
    let svc_type = run_ros2_raw(&["service", "type", &svc]).await;
    let iface = if !svc_type.starts_with("Error") && svc_type.contains('/') {
        run_ros2_raw(&["interface", "show", svc_type.trim()]).await
    } else {
        "Could not determine service type.".to_string()
    };

    Ok(Json(DetailResponse {
        info,
        interface: iface,
    }))
}

/// Returns detailed info and interface definition for a specific action.
pub async fn action_detail_handler(
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let action = format!("/{}", name);
    tracing::info!("Action detail requested: {}", action);

    let info = run_ros2_raw(&["action", "info", "-t", &action]).await;
    let iface = if let Some(action_type) = extract_type_from_info(&info) {
        run_ros2_raw(&["interface", "show", &action_type]).await
    } else {
        "Could not determine action type.".to_string()
    };

    Ok(Json(DetailResponse {
        info,
        interface: iface,
    }))
}
