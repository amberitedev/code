//! Pairing as the owner sees it on this machine, and removing this Core from its account, which
//! the App does before it deletes the Core on the backend. A Core that was off when its owner
//! removed it learns from the backend instead, at its next start (see `access::sync`).

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::extract::State;
use contracts::{
    ApiError, Empty, ErrorCode, Json,
    core::{Action, GetPairing, PairingStatus, RequestPairingCode, Unpair},
    server::Req,
};
use sqlx::SqlitePool;

use super::{CoreState, internal};
use crate::access::{
    Caller, Changed, Dangerous, activity, pairing, require_owner, sync,
};

/// How long `POST /pairing/code` waits for the new code.
const NEW_CODE_WAIT: Duration = Duration::from_secs(20);

async fn status(db: &SqlitePool) -> Result<PairingStatus, ApiError> {
    let paired = pairing::paired(db).await.map_err(internal)?;
    let shown = if paired {
        None
    } else {
        pairing::shown(db).await.map_err(internal)?
    };
    Ok(PairingStatus {
        paired,
        expires_at: shown.as_ref().map(|shown| shown.expires_at),
        code: shown.map(|shown| shown.code),
    })
}

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<GetPairing>,
) -> Result<Json<PairingStatus>, ApiError> {
    require_owner(&caller)?;
    Ok(Json(status(&state.db).await?))
}

pub async fn new_code(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<RequestPairingCode>,
) -> Result<Json<PairingStatus>, ApiError> {
    require_owner(&caller)?;
    if pairing::paired(&state.db).await.map_err(internal)? {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "This Core is already paired. To pair it with another account, remove it in Amberite under Settings, Cores, first.",
        ));
    }
    let old = pairing::shown(&state.db)
        .await
        .map_err(internal)?
        .map(|shown| shown.code);
    pairing::drop_code(&state.db).await.map_err(internal)?;
    state.access.new_code.notify_one();
    let deadline = Instant::now() + NEW_CODE_WAIT;
    loop {
        let status = status(&state.db).await?;
        if status.paired || (status.code.is_some() && status.code != old) {
            return Ok(Json(status));
        }
        if Instant::now() > deadline {
            return Err(ApiError::new(
                ErrorCode::InternalError,
                "Core could not get a new code from Amberite yet. It keeps trying: run copal status in a minute. copal logs says why.",
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

pub async fn remove(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<CoreState>>,
    _: Req<Unpair>,
) -> Result<Empty, ApiError> {
    let mut tx = state.db.begin().await.map_err(internal)?;
    activity::record(&mut tx, caller.user_id(), None, &Action::CoreRemoved)
        .await
        .map_err(internal)?;
    sync::unpair(&mut tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    let _ = state.access.changed.send(Changed::Everyone);
    // Sync notices and pairs again.
    state.access.owed.notify_one();
    Ok(Empty)
}
