//! The Minecraft EULA, accepted once by the owner for every server on this Core. No server
//! starts before that. Owner only.

use std::sync::Arc;

use axum::extract::State;
use contracts::{
    ApiError, Json,
    core::{AcceptEula, Action, Eula, GetEula},
    server::Req,
};
use sqlx::SqlitePool;

use super::{CoreState, internal};
use crate::access::{Caller, activity, now, require_owner};

/// Where the EULA is, to show before accepting it.
pub const URL: &str = "https://aka.ms/MinecraftEULA";

/// When the owner accepted the EULA, if they did.
pub async fn accepted_at(db: &SqlitePool) -> sqlx::Result<Option<i64>> {
    Ok(
        sqlx::query_scalar("SELECT eula_accepted_at FROM core WHERE id = 1")
            .fetch_optional(db)
            .await?
            .flatten(),
    )
}

/// Accept the EULA, keeping the first acceptance's time.
pub async fn accept(db: &SqlitePool) -> sqlx::Result<()> {
    crate::db::core_id(db).await?;
    sqlx::query(
        "UPDATE core SET eula_accepted_at = coalesce(eula_accepted_at, ?) WHERE id = 1",
    )
    .bind(now())
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<GetEula>,
) -> Result<Json<Eula>, ApiError> {
    require_owner(&caller)?;
    Ok(Json(Eula {
        url: URL.into(),
        accepted_at: accepted_at(&state.db).await.map_err(internal)?,
    }))
}

pub async fn post(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<AcceptEula>,
) -> Result<Json<Eula>, ApiError> {
    require_owner(&caller)?;
    let before = accepted_at(&state.db).await.map_err(internal)?;
    accept(&state.db).await.map_err(internal)?;
    if before.is_none() {
        activity::log(&state.db, &caller, None, Action::EulaAccepted).await;
    }
    Ok(Json(Eula {
        url: URL.into(),
        accepted_at: accepted_at(&state.db).await.map_err(internal)?,
    }))
}
