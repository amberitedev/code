//! Diagnostics API handlers - System stats and health.

use axum::extract::State;
use axum::Json;
use serde::Serialize;
use std::sync::Arc;
use crate::application::registry::ServiceRegistry;

/// System stats response.
#[derive(Serialize)]
pub struct SystemStats {
    pub cpu_usage: f64,
    pub memory_usage: u64,
    pub total_memory: u64,
    pub running_instances: usize,
    pub version: String,
}

/// Handle system stats.
pub async fn get_system_stats(
    State(registry): State<Arc<ServiceRegistry>>,
) -> Json<SystemStats> {
    let running = registry.instance_service.running_count();

    Json(SystemStats {
        cpu_usage: 0.0,
        memory_usage: 0,
        total_memory: 0,
        running_instances: running,
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

/// Health response.
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
}

/// Handle health check.
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse { status: "healthy".into() })
}
