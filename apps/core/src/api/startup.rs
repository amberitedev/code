//! The Java a server runs: a managed major other than the one the Minecraft version needs, or, as
//! an advanced override, a custom Java path. Both run as `copal-servers` like every server.
//! Editors and the owner only, since choosing what runs is running code.

use std::sync::Arc;

use axum::extract::State;
use contracts::{
    ApiError, Empty, ErrorCode,
    core::{Action, SetJava, SetJavaVersion},
    server::Req,
};

use super::{CoreState, internal};
use crate::{
    access::{Caller, Permission, activity},
    servers::{ServerError, platform, server_rows},
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
    let server = server_rows::get(&state.db, id)
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
    server_rows::set_java(
        &state.db,
        id,
        server.java_path.as_deref(),
        chosen.map(i64::from),
    )
    .await
    .map_err(internal)?;
    activity::log(&state.db, caller, Some(id), Action::SettingsChanged).await;
    Ok(())
}

pub async fn set_java_version(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<SetJavaVersion>,
) -> Result<Empty, ApiError> {
    state
        .access
        .require(&caller, &params.id, Permission::Edit)
        .await?;
    set_major(&state, &caller, &params.id, body.major).await?;
    Ok(Empty)
}

/// A custom Java path, or null for Core's. It must be an absolute path to a file.
pub async fn set_java_path(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<SetJava>,
) -> Result<Empty, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let server = server_rows::get(&state.db, &id)
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
    server_rows::set_java(
        &state.db,
        &id,
        body.path.as_deref(),
        server.java_version,
    )
    .await
    .map_err(internal)?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Empty)
}
