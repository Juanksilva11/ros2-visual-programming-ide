use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Tipos de eventos que el sistema puede emitir
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // Estilo industrial (ej. STEP_STARTED)
pub enum EventType {
    Info,
    Warning,
    Error,
    StepStart,
    StepFinish,
    Progress,
    ProgramFinish,
    ProgramAbort,
}

// El DTO (Data Transfer Object) que viajará por el WebSocket
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SystemEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,
    pub message: String,
    pub metadata: Option<serde_json::Value>, // Datos extra flexibles (JSON)
}

impl SystemEvent {
    // Constructor rápido
    pub fn new(event_type: EventType, message: String) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            message,
            metadata: None,
        }
    }

    pub fn with_progress(message: String, current: f64, total: f64) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type: EventType::Progress,
            message,
            metadata: Some(serde_json::json!({
                "current": current,
                "total": total,
                "percent": (current / total) * 100.0
            })),
        }
    }
}
