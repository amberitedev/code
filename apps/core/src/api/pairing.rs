//! Pairing as the owner sees it on this machine, and removing this Core from its account. The App
//! unpairs before it deletes the Core on the backend; `copal unpair` has the Core leave the account
//! itself first. A Core that was off when its owner removed it learns from the backend instead, at
//! its next start (see `access::sync`).

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::extract::State;
use contracts::{
    ApiError, Empty, ErrorCode, Json,
    backend::cores::LeaveAccount,
    client::{CallError, Remote, call},
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
/// How long `DELETE /pairing` from the CLI waits for the backend.
const LEAVE_TIMEOUT: Duration = Duration::from_secs(10);

/// The owner's name, while the Core has one.
pub async fn owner(db: &SqlitePool) -> Result<Option<String>, ApiError> {
    sqlx::query_scalar(
        "SELECT u.name FROM core c JOIN users u ON u.id = c.owner_user_id WHERE c.id = 1",
    )
    .fetch_optional(db)
    .await
    .map_err(internal)
}

async fn status(db: &SqlitePool) -> Result<PairingStatus, ApiError> {
    let paired = pairing::paired(db).await.map_err(internal)?;
    let shown = if paired {
        None
    } else {
        pairing::shown(db).await.map_err(internal)?
    };
    let owner = if paired { owner(db).await? } else { None };
    Ok(PairingStatus {
        paired,
        owner,
        expires_at: shown
            .as_ref()
            .map(|shown| crate::access::at(shown.expires_at)),
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
                "Core could not get a new code from Amberite yet. It keeps trying: run copal pair in a minute. copal logs says why.",
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
    // The App removes the Core on the backend itself; the CLI has no one else to.
    if matches!(caller, Caller::Local) {
        leave_account(&state).await?;
    }
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

/// Take this Core off its account on the backend, with its own key. A backend that no longer knows
/// it is as good as done.
async fn leave_account(state: &CoreState) -> Result<(), ApiError> {
    match leave(&state.db, state.storage.backend_url())
        .await
        .map_err(internal)?
    {
        Some(false) => Err(ApiError::new(
            ErrorCode::InternalError,
            "Core could not reach Amberite to leave the account. Check the internet connection and try again, or remove this Core in Amberite under Settings, Cores.",
        )),
        Some(true) | None => Ok(()),
    }
}

/// Whether the backend took the Core in `db` off its account. `None` when there is nothing to
/// leave: no backend, or not paired. `copal uninstall` calls this too, before it deletes the
/// key with the data folder.
pub async fn leave(
    db: &SqlitePool,
    backend_url: Option<&str>,
) -> sqlx::Result<Option<bool>> {
    let Some(base) = backend_url else {
        return Ok(None);
    };
    let key: Option<String> = sqlx::query_scalar(
        "SELECT backend_key FROM core WHERE id = 1 AND owner_user_id IS NOT NULL",
    )
    .fetch_optional(db)
    .await?
    .flatten();
    let Some(key) = key else {
        return Ok(None);
    };
    Ok(Some(told(base, &key).await))
}

/// Whether the backend took the Core with `key` off its account.
async fn told(base: &str, key: &str) -> bool {
    let Ok(http) = crate::network::http().build() else {
        return false;
    };
    let remote = Remote {
        http: &http,
        base,
        bearer: Some(key),
        ticket: None,
        timeout: Some(LEAVE_TIMEOUT),
    };
    match call::<LeaveAccount>(&remote, &(), &(), Empty).await {
        Ok(_) => true,
        Err(CallError::Api { error, .. })
            if error.error == ErrorCode::CoreRemoved =>
        {
            true
        }
        Err(error) => {
            tracing::warn!(%error, "could not leave the account");
            false
        }
    }
}
