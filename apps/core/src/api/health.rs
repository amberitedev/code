//! `GET /health`: whether Core is up, which Core it is, and the protocol it speaks. The App probes
//! it to pick between a Core's local and internet address and to check the protocol, and Core
//! asks it of itself on start: the first answer writes the health marker that ends an update's
//! trial (see `update::launcher`).

use std::sync::Arc;

use axum::extract::State;
use contracts::{
    ApiError, Json, PROTOCOL_VERSION,
    core::{CheckHealth, Health},
    server::Req,
};

use super::{CoreState, internal};
use crate::db;

pub async fn get(
    State(state): State<Arc<CoreState>>,
    _: Req<CheckHealth>,
) -> Result<Json<Health>, ApiError> {
    let core_id = db::core_id(&state.db).await.map_err(internal)?;
    state.update.answered();
    Ok(Json(Health {
        protocol: PROTOCOL_VERSION,
        version: env!("CARGO_PKG_VERSION").into(),
        core_id,
    }))
}
