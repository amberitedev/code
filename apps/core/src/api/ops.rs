//! Long ops, as the App runs them: a route registers the caller's op id, does the work in the
//! background and answers 202. The op ends with `CoreEvent::OpEnded` on every socket. No request
//! stays open for an op's length, so a caller's timeout never cuts one off.

use std::{collections::HashSet, future::Future, sync::Mutex};

use axum::{extract::State, http::StatusCode};
use contracts::{
    ApiError, ErrorCode, Json,
    core::{CoreEvent, ListOps},
    server::Req,
};
use std::sync::Arc;

use super::{CoreState, internal};
use crate::access::{Caller, require_owner};

#[derive(Default)]
pub struct Ops(Mutex<HashSet<String>>);

/// Run `work` as `op` and answer 202 once it is registered; `conflict` when an op with that id is
/// running. The op ends with what `work` returns.
pub fn start<F>(
    state: &Arc<CoreState>,
    op: String,
    work: F,
) -> Result<StatusCode, ApiError>
where
    F: Future<Output = Result<(), ApiError>> + Send + 'static,
{
    if !state.ops.0.lock().expect("not poisoned").insert(op.clone()) {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "An operation with this id is already running",
        ));
    }
    let state = Arc::clone(state);
    // Its own task, so a panic in the work still ends the op.
    let work = tokio::spawn(work);
    tokio::spawn(async move {
        let error = match work.await {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error),
            Err(error) => Some(internal(error)),
        };
        state.ops.0.lock().expect("not poisoned").remove(&op);
        let _ = state.backups.events.send(CoreEvent::OpEnded { op, error });
    });
    Ok(StatusCode::ACCEPTED)
}

pub async fn list(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<ListOps>,
) -> Result<Json<Vec<String>>, ApiError> {
    require_owner(&caller)?;
    Ok(Json(
        state
            .ops
            .0
            .lock()
            .expect("not poisoned")
            .iter()
            .cloned()
            .collect(),
    ))
}
