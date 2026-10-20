//! A server's memory and restarts, which the App shows under performance.
//!
//! - `GET /servers/{id}/performance`: [`Performance`]. Viewers and up.
//! - `PUT /servers/{id}/memory`: [`Memory`] in, [`Performance`] out. Editors and up. A running
//!   server uses the new memory from its next start.
//! - `PUT /servers/{id}/restarts`: `Restarts` in, [`Performance`] out. Editors and up.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    AppState,
    activity::Action,
    error::{ApiError, ErrorCode},
};
use crate::{
    access::{Caller, Level, activity},
    servers::{
        ServerError, records,
        restarts::{self, Restarts},
    },
};

/// The least memory a server may have.
pub const MIN_MEMORY_MB: u32 = 512;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Performance {
    /// The most memory Java may use.
    pub memory_mb: u32,
    pub restarts: Restarts,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Memory {
    pub memory_mb: u32,
}

async fn performance(
    state: &AppState,
    id: &str,
) -> Result<Performance, ApiError> {
    let server = records::get(&state.db, id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.into()))?;
    Ok(Performance {
        memory_mb: u32::try_from(server.memory_mb).unwrap_or(u32::MAX),
        restarts: restarts::get(&state.db, id).await?,
    })
}

/// `GET /servers/{id}/performance`.
pub async fn get(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Performance>, ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
    Ok(Json(performance(&state, &id).await?))
}

/// `PUT /servers/{id}/memory`.
pub async fn set_memory(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<Memory>,
) -> Result<Json<Performance>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    if request.memory_mb < MIN_MEMORY_MB {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            format!("A server needs at least {MIN_MEMORY_MB} MB."),
        ));
    }
    sqlx::query("UPDATE servers SET memory_mb = ? WHERE id = ?")
        .bind(request.memory_mb)
        .bind(&id)
        .execute(&state.db)
        .await?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Json(performance(&state, &id).await?))
}

/// `PUT /servers/{id}/restarts`.
pub async fn set_restarts(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<Restarts>,
) -> Result<Json<Performance>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    if request.smart_after_hours == Some(0) {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "Smart restarts need at least an hour of uptime.",
        ));
    }
    restarts::set(&state.db, &id, &request).await?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Json(performance(&state, &id).await?))
}
