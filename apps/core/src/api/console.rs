//! `POST /servers/{id}/console`: send one command to a server's console and learn whether Core
//! passed it on. [`SendCommand`] in, no body out. Editors and up. The console socket takes
//! commands too, but answers none of them; this route reports why one was refused.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    AppState,
    activity::Action,
    error::{ApiError, ErrorCode},
};
use crate::access::{Caller, Level, activity};

/// The longest command Core passes on.
const MAX_COMMAND: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SendCommand {
    /// One line, as typed into the server console, such as `say hello`.
    pub command: String,
}

/// `POST /servers/{id}/console`.
pub async fn send(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<SendCommand>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let command = request.command;
    // A line break would run a second command the activity log never shows.
    if command.trim().is_empty()
        || command.len() > MAX_COMMAND
        || command.contains(['\n', '\r'])
    {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            format!("Send one line of at most {MAX_COMMAND} characters."),
        ));
    }
    state.servers.command(&id, &command).await?;
    activity::log(
        &state.db,
        &caller,
        Some(&id),
        Action::ConsoleCommand { command },
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}
