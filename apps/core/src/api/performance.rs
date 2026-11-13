//! A server's memory, JVM flags, and restarts, which the App shows under performance.

use std::sync::Arc;

use axum::extract::State;
use contracts::{
    ApiError, ErrorCode, Json,
    core::{
        Action, GetPerformance, Performance, SetFlags, SetMemory, SetRestarts,
    },
    server::Req,
};

use super::{CoreState, internal};
use crate::{
    access::{Caller, Permission, activity},
    servers::{ServerError, memory, restarts, server_rows},
};

/// The least memory a server may have.
pub const MIN_MEMORY_MB: u32 = 512;

/// The most flags a server may have.
const MAX_FLAGS: usize = 64;

async fn performance(
    state: &CoreState,
    id: &str,
) -> Result<Performance, ApiError> {
    let server = server_rows::get(&state.db, id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.into()))?;
    let own = memory::flags(&state.db, id).await.map_err(internal)?;
    Ok(Performance {
        memory_mb: u32::try_from(server.memory_mb).unwrap_or(u32::MAX),
        custom_flags: own.is_some(),
        flags: own
            .unwrap_or_else(|| memory::TUNED_FLAGS.map(String::from).to_vec()),
        restarts: restarts::get(&state.db, id).await.map_err(internal)?,
    })
}

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    request: Req<GetPerformance>,
) -> Result<Json<Performance>, ApiError> {
    let id = request.params.id;
    state.access.require(&caller, &id, Permission::View).await?;
    Ok(Json(performance(&state, &id).await?))
}

pub async fn set_memory(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req {
        params,
        body: request,
        ..
    }: Req<SetMemory>,
) -> Result<Json<Performance>, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    // The owner passes `require` for any id, so a missing server is found here, before a write.
    performance(&state, &id).await?;
    if request.memory_mb < MIN_MEMORY_MB {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            format!("A server needs at least {MIN_MEMORY_MB} MB."),
        ));
    }
    sqlx::query("UPDATE servers SET memory_mb = ? WHERE id = ?")
        .bind(request.memory_mb)
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(internal)?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Json(performance(&state, &id).await?))
}

pub async fn set_flags(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req {
        params,
        body: request,
        ..
    }: Req<SetFlags>,
) -> Result<Json<Performance>, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    if let Some(flags) = &request.flags
        && (flags.len() > MAX_FLAGS
            || flags
                .iter()
                .any(|flag| !flag.starts_with('-') || flag.trim() != flag))
    {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            format!(
                "Give at most {MAX_FLAGS} flags, each starting with - and without spaces around it."
            ),
        ));
    }
    performance(&state, &id).await?;
    memory::set_flags(&state.db, &id, request.flags.as_deref())
        .await
        .map_err(internal)?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Json(performance(&state, &id).await?))
}

pub async fn set_restarts(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req {
        params,
        body: request,
        ..
    }: Req<SetRestarts>,
) -> Result<Json<Performance>, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    performance(&state, &id).await?;
    if request.smart_after_hours == Some(0) {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "Smart restarts need at least an hour of uptime.",
        ));
    }
    restarts::set(&state.db, &id, &request)
        .await
        .map_err(internal)?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Json(performance(&state, &id).await?))
}
