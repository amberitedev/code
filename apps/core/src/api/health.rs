//! `GET /health`: whether Core is up, and which Core it is. The App probes it to pick between a
//! Core's local and internet address, and Core asks it of itself on start: the first answer
//! writes the health marker that ends an update's trial (see `update::launcher`).

use std::sync::Arc;

use axum::{Json, extract::State};
use serde::Serialize;

use super::AppState;
use crate::db;

#[derive(Serialize)]
pub struct Health {
    pub status: &'static str,
    pub version: &'static str,
    pub core_id: String,
}

pub async fn get(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Health>, axum::http::StatusCode> {
    let core_id = db::core_id(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?;
    state.update.answered();
    Ok(Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        core_id,
    }))
}
