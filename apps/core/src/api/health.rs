//! `GET /health`: whether Core is up, and which Core it is. The App probes it to pick between a
//! Core's local and internet address, and Core asks it of itself on start: the first answer
//! writes the health marker that ends an update's trial (see `update::launcher`).

use std::sync::Arc;

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use super::CoreState;
use crate::db;

/// Response of `GET /health`. The CLI reads it too.
#[derive(Debug, Serialize, Deserialize)]
pub struct Health {
    pub status: String,
    pub version: String,
    pub core_id: String,
}

pub async fn get(
    State(state): State<Arc<CoreState>>,
) -> Result<Json<Health>, axum::http::StatusCode> {
    let core_id = db::core_id(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?;
    state.update.answered();
    Ok(Json(Health {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        core_id,
    }))
}
