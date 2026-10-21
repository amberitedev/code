//! Removing this Core from its account, which the App does before it deletes the Core on the
//! backend.
//!
//! - `DELETE /pairing`: dangerous, the owner only. The pairing is cleared, every session ends, and
//!   Core shows a new pairing code. 204. A Core that was off when its owner removed it learns from
//!   the backend instead, at its next start (see `access::sync`).

use std::sync::Arc;

use axum::{extract::State, http::StatusCode};

use super::{AppState, activity::Action, error::ApiError};
use crate::access::{Changed, Dangerous, activity, sync};

/// `DELETE /pairing`.
pub async fn remove(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<AppState>>,
) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await?;
    activity::record(&mut tx, caller.user_id(), None, &Action::CoreRemoved)
        .await?;
    sync::unpair(&mut tx).await?;
    tx.commit().await?;
    let _ = state.access.changed.send(Changed::Everyone);
    // Sync notices and pairs again.
    state.access.owed.notify_one();
    Ok(StatusCode::NO_CONTENT)
}
