//! The activity log: sessions, sensitive reads, and server actions, with who did them.

use std::sync::Arc;

use axum::extract::State;
use contracts::{
    ApiError, Json,
    core::{ActionLogResponse, ListActivity},
    server::Req,
};

use super::CoreState;
use crate::access::{Caller, activity, require_owner};

pub async fn list(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    request: Req<ListActivity>,
) -> Result<Json<ActionLogResponse>, ApiError> {
    require_owner(&caller)?;
    Ok(Json(activity::page(&state.db, &request.query).await?))
}
