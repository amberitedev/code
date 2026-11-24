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

use engine::{
    routes::api_error,
    state::{EditInstance, InstanceLaunchOverridesPatch, MemorySettings},
};

use super::{CoreState, internal};
use crate::{
    access::{Caller, activity},
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
    let server = server_rows::get(&state.db, &state.engine, id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.into()))?;
    Ok(Performance {
        memory_mb: u32::try_from(server.memory_mb).unwrap_or(u32::MAX),
        custom_flags: server.jvm_flags.is_some(),
        flags: server
            .jvm_flags
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
    state.access.require(&caller, &id).await?;
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
    state.access.require(&caller, &id).await?;
    // The owner passes `require` for any id, so a missing server is found here, before a write.
    performance(&state, &id).await?;
    if request.memory_mb < MIN_MEMORY_MB {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            format!("A server needs at least {MIN_MEMORY_MB} MB."),
        ));
    }
    let memory = MemorySettings {
        maximum: request.memory_mb,
    };
    edit_launch(
        &state,
        &id,
        InstanceLaunchOverridesPatch {
            memory: Some(Some(memory)),
            ..Default::default()
        },
    )
    .await?;
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
    state.access.require(&caller, &id).await?;
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
    edit_launch(
        &state,
        &id,
        InstanceLaunchOverridesPatch {
            extra_launch_args: Some(request.flags),
            ..Default::default()
        },
    )
    .await?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Json(performance(&state, &id).await?))
}

/// Change the launch settings the server keeps on its engine instance. Its edit-instance route is
/// never mounted on a Core: it would set host paths and launch hooks.
pub(super) async fn edit_launch(
    state: &CoreState,
    id: &str,
    launch: InstanceLaunchOverridesPatch,
) -> Result<(), ApiError> {
    engine::instance::edit(
        &state.engine,
        id,
        EditInstance {
            launch_overrides: Some(launch),
            ..Default::default()
        },
    )
    .await
    .map(drop)
    .map_err(api_error)
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
    state.access.require(&caller, &id).await?;
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
