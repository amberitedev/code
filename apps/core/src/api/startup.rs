//! How a server starts: the command it runs, and the Java: a managed major other than the one the
//! Minecraft version needs, or, as an advanced override, a custom Java path. Both run as
//! `copal-servers` like every server.

use std::sync::Arc;

use axum::extract::State;
use contracts::{
    ApiError, Empty, ErrorCode, Json,
    core::{Action, ChangeStartup, GetStartup, RuntimeOptions, SetJava},
    server::Req,
};

use engine::state::InstanceLaunchOverridesPatch;

use super::{CoreState, internal};
use crate::{
    access::{Caller, activity},
    servers::{ServerError, launch, platform, server_rows},
};

/// Java majors Core installs.
const MAJORS: [u32; 5] = [8, 11, 17, 21, 25];

/// Choose the managed Java major, or `None` for the one the version needs.
async fn set_major(
    state: &CoreState,
    caller: &Caller,
    id: &str,
    major: Option<u32>,
) -> Result<(), ApiError> {
    if let Some(major) = major
        && !MAJORS.contains(&major)
    {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            format!("Core installs Java {MAJORS:?}, not {major}."),
        ));
    }
    server_rows::get(&state.db, &state.engine, id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.into()))?;
    // The major the version needs is the default, not a choice: stored, it would pin the server
    // to it after a game version change.
    let platform = platform::platform(&state.engine, id)
        .await
        .map_err(engine::routes::api_error)?;
    let needed = match platform {
        Some(platform) => {
            engine::server::java_major(&state.engine, &platform.game_version)
                .await
                .ok()
        }
        None => None,
    };
    let chosen = major.filter(|major| Some(*major) != needed);
    server_rows::set_java_version(&state.db, id, chosen.map(i64::from))
        .await
        .map_err(internal)?;
    activity::log(&state.db, caller, Some(id), Action::SettingsChanged).await;
    Ok(())
}

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<GetStartup>,
) -> Result<Json<RuntimeOptions>, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id).await?;
    let row = server_rows::get(&state.db, &state.engine, &id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let Some(platform) = platform::platform(&state.engine, &id)
        .await
        .map_err(engine::routes::api_error)?
    else {
        return Ok(Json(RuntimeOptions {
            java_version: None,
            original_invocation: None,
        }));
    };
    let java_version =
        launch::java_major(&state.engine, &row, &platform.game_version)
            .await
            .ok();
    let original_invocation =
        engine::server::server_command(&state.engine, &id)
            .await
            .ok()
            .map(|command| {
                // Shown with the tuned flags, whatever the server's own.
                let tuned = server_rows::ServerRow {
                    jvm_flags: None,
                    ..row.clone()
                };
                launch::invocation(&launch::args(
                    state.servers.paths(),
                    &tuned,
                    command.args,
                ))
            });
    Ok(Json(RuntimeOptions {
        java_version,
        original_invocation,
    }))
}

pub async fn change(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<ChangeStartup>,
) -> Result<Empty, ApiError> {
    state.access.require(&caller, &params.id).await?;
    set_major(&state, &caller, &params.id, body.java_version).await?;
    Ok(Empty)
}

/// A custom Java path, or null for Core's. It must be an absolute path to a file.
pub async fn set_java_path(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<SetJava>,
) -> Result<Empty, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id).await?;
    server_rows::get(&state.db, &state.engine, &id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    if let Some(path) = &body.path {
        let path = std::path::Path::new(path);
        if !path.is_absolute() || !path.is_file() {
            return Err(ApiError::new(
                ErrorCode::InvalidInput,
                "A custom Java is an absolute path to its java binary on the Core's machine.",
            ));
        }
    }
    super::performance::edit_launch(
        &state,
        &id,
        InstanceLaunchOverridesPatch {
            java_path: Some(body.path),
            ..Default::default()
        },
    )
    .await?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Empty)
}
