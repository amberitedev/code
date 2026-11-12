//! The activity log: access changes, sessions, sensitive reads, and server actions, with who did
//! them. Owner only.

use std::sync::Arc;

use axum::extract::State;
use contracts::{
    ApiError, Json,
    core::{ActivityPage, ListActivity},
    server::Req,
};

use super::{CoreState, internal};
use crate::access::{Caller, activity};

pub async fn list(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    request: Req<ListActivity>,
) -> Result<Json<ActivityPage>, ApiError> {
    caller.require_owner()?;
    Ok(Json(
        activity::page(&state.db, &request.query)
            .await
            .map_err(internal)?,
    ))
}
