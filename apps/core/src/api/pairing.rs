//! Pairing as the owner sees it on this machine, and removing this Core from its account, which
//! the App does before it deletes the Core on the backend.
//!
//! - `GET /pairing`: whether this Core is paired, and while it is not, the code it shows. The
//!   owner only. [`PairingStatus`].
//! - `POST /pairing/code`: drop the code shown and get a fresh one, while unpaired. The owner only.
//!   [`PairingStatus`] once Core has the new code, within 20 seconds.
//! - `DELETE /pairing`: dangerous, the owner only. The pairing is cleared, every session ends, and
//!   Core shows a new pairing code. 204. A Core that was off when its owner removed it learns from
//!   the backend instead, at its next start (see `access::sync`).

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ts_rs::TS;

use super::{
    AppState,
    activity::Action,
    error::{ApiError, ErrorCode},
};
use crate::access::{Caller, Changed, Dangerous, activity, pairing, sync};

/// How long `POST /pairing/code` waits for the new code.
const NEW_CODE_WAIT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PairingStatus {
    pub paired: bool,
    /// The code to enter in the App, like `MZUX-D576`. Null once paired, and while Core is still
    /// getting one from Amberite.
    pub code: Option<String>,
    /// Unix seconds.
    pub expires_at: Option<i64>,
}

async fn status(db: &SqlitePool) -> Result<PairingStatus, ApiError> {
    let paired = pairing::paired(db).await?;
    let shown = if paired {
        None
    } else {
        pairing::shown(db).await?
    };
    Ok(PairingStatus {
        paired,
        expires_at: shown.as_ref().map(|shown| shown.expires_at),
        code: shown.map(|shown| shown.code),
    })
}

/// `GET /pairing`.
pub async fn get(
    caller: Caller,
    State(state): State<Arc<AppState>>,
) -> Result<Json<PairingStatus>, ApiError> {
    caller.require_owner()?;
    Ok(Json(status(&state.db).await?))
}

/// `POST /pairing/code`.
pub async fn new_code(
    caller: Caller,
    State(state): State<Arc<AppState>>,
) -> Result<Json<PairingStatus>, ApiError> {
    caller.require_owner()?;
    if pairing::paired(&state.db).await? {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "This Core is already paired. To pair it with another account, remove it in Amberite under Settings, Cores, first.",
        ));
    }
    let old = pairing::shown(&state.db).await?.map(|shown| shown.code);
    pairing::drop_code(&state.db).await?;
    state.access.new_code.notify_one();
    let deadline = Instant::now() + NEW_CODE_WAIT;
    loop {
        let status = status(&state.db).await?;
        if status.paired || (status.code.is_some() && status.code != old) {
            return Ok(Json(status));
        }
        if Instant::now() > deadline {
            return Err(ApiError::new(
                ErrorCode::Internal,
                "Core could not get a new code from Amberite yet. It keeps trying: run copal status in a minute. copal logs says why.",
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

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
