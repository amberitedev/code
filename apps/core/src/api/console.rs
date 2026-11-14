//! `POST /servers/{id}/console`: send one command to a server's console and learn whether Core
//! passed it on, or why it was refused. The `/events` socket only reads the console.

use std::sync::Arc;

use axum::extract::State;
use contracts::{
    ApiError, Empty, ErrorCode,
    core::{Action, SendCommand},
    server::Req,
};

use super::CoreState;
use crate::access::{Caller, Permission, activity};

/// The longest command Core passes on.
const MAX_COMMAND: usize = 4096;

pub async fn send(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<SendCommand>,
) -> Result<Empty, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let command = body.command;
    // A line break would run a second command the activity log never shows.
    if command.trim().is_empty()
        || command.len() > MAX_COMMAND
        || command.contains(['\n', '\r'])
    {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
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
    Ok(Empty)
}
