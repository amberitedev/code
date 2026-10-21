//! The Minecraft EULA, accepted once by the owner for every server on this Core. No server
//! starts before that. Owner only.
//!
//! - `GET /eula`: [`Eula`].
//! - `POST /eula`: accept it. [`Eula`].

use std::sync::Arc;

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ts_rs::TS;

use super::{AppState, activity::Action, error::ApiError};
use crate::access::{Caller, activity, now};

/// Where the EULA is, to show before accepting it.
pub const URL: &str = "https://aka.ms/MinecraftEULA";

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Eula {
    /// The EULA's address.
    pub url: String,
    /// Unix seconds. Null until the owner accepts it.
    pub accepted_at: Option<i64>,
}

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

/// `GET /eula`.
pub async fn get(
    caller: Caller,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Eula>, ApiError> {
    caller.require_owner()?;
    Ok(Json(Eula {
        url: URL.into(),
        accepted_at: accepted_at(&state.db).await?,
    }))
}

/// `POST /eula`.
pub async fn post(
    caller: Caller,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Eula>, ApiError> {
    caller.require_owner()?;
    let before = accepted_at(&state.db).await?;
    accept(&state.db).await?;
    if before.is_none() {
        activity::log(&state.db, &caller, None, Action::EulaAccepted).await;
    }
    Ok(Json(Eula {
        url: URL.into(),
        accepted_at: accepted_at(&state.db).await?,
    }))
}
