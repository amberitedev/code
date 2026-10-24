//! A server's memory, JVM flags, and restarts, which the App shows under performance.
//!
//! - `GET /servers/{id}/performance`: [`Performance`]. Viewers and up.
//! - `PUT /servers/{id}/memory`: [`Memory`] in, [`Performance`] out. Editors and up.
//! - `PUT /servers/{id}/flags`: [`JvmFlags`] in, [`Performance`] out. Editors and up.
//! - `PUT /servers/{id}/restarts`: `Restarts` in, [`Performance`] out. Editors and up.
//!
//! A running server uses new memory and flags from its next start.

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
        ServerError, memory, records,
        restarts::{self, Restarts},
    },
};

/// The least memory a server may have.
pub const MIN_MEMORY_MB: u32 = 512;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Performance {
    /// The most memory Java may use. Java starts with less and grows to it.
    pub memory_mb: u32,
    /// The JVM flags the server starts with.
    pub flags: Vec<String>,
    /// `flags` are the server's own rather than Core's tuned ones.
    pub custom_flags: bool,
    pub restarts: Restarts,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct JvmFlags {
    /// The server's own flags, each starting with `-`. Null goes back to the tuned ones.
    pub flags: Option<Vec<String>>,
}

/// The most flags a server may have.
const MAX_FLAGS: usize = 64;

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
    let own = memory::flags(&state.db, id).await?;
    Ok(Performance {
        memory_mb: u32::try_from(server.memory_mb).unwrap_or(u32::MAX),
        custom_flags: own.is_some(),
        flags: own
            .unwrap_or_else(|| memory::TUNED_FLAGS.map(String::from).to_vec()),
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
    // The owner passes `require` for any id, so a missing server is found here, before a write.
    performance(&state, &id).await?;
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

/// `PUT /servers/{id}/flags`.
pub async fn set_flags(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<JvmFlags>,
) -> Result<Json<Performance>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    if let Some(flags) = &request.flags
        && (flags.len() > MAX_FLAGS
            || flags
                .iter()
                .any(|flag| !flag.starts_with('-') || flag.trim() != flag))
    {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            format!(
                "Give at most {MAX_FLAGS} flags, each starting with - and without spaces around it."
            ),
        ));
    }
    performance(&state, &id).await?;
    memory::set_flags(&state.db, &id, request.flags.as_deref()).await?;
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
    performance(&state, &id).await?;
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
