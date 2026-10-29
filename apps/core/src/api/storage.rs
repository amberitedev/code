//! Storage for the shared file pool (see `crate::storage`). Owner only.
//!
//! - `GET /storage`: [`StorageStatus`].
//! - `PATCH /storage`: [`StorageSettings`] in, [`StorageStatus`] out.
//! - `/store/...`, every method: forwarded to the store unchanged. Pool tickets, not sessions,
//!   decide who may use it, and the store checks them itself.

use std::{net::IpAddr, sync::Arc};

use axum::{
    Extension, Json,
    extract::{Request, State},
    response::Response,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{AppState, error::ApiError};
use crate::access::{Caller, Peer};

/// The settings to change. Absent ones stay. Bytes throughout.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct StorageSettings {
    /// Offer storage to other people's shares. Off, the Core runs privately.
    #[ts(optional)]
    pub enabled: Option<bool>,
    /// The most the store may hold.
    #[ts(optional)]
    pub cap: Option<i64>,
    /// The most it may serve a day.
    #[ts(optional)]
    pub egress_cap: Option<i64>,
}

/// Bytes throughout. What the store holds is shown whether or not storage is offered: a Core
/// that does not offer it still holds its owner's shares.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StorageStatus {
    /// Offers storage to other people's shares. Off, the Core runs privately.
    pub enabled: bool,
    pub cap: u64,
    pub egress_cap: u64,
    /// What the stored versions take, counting each object once.
    pub used: u64,
    /// Free on the store's disk.
    pub free: u64,
    /// Served today, UTC.
    pub egress_today: u64,
    /// Core is installed as a service. The pool places shares only on Cores that are.
    pub service: bool,
    /// Versions the store holds.
    pub versions: u32,
}

/// `GET /storage`.
pub async fn get(
    caller: Caller,
    State(state): State<Arc<AppState>>,
) -> Result<Json<StorageStatus>, ApiError> {
    caller.require_owner()?;
    Ok(Json(state.storage.status().await?))
}

/// `PATCH /storage`.
pub async fn settings(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Json(change): Json<StorageSettings>,
) -> Result<Json<StorageStatus>, ApiError> {
    caller.require_owner()?;
    Ok(Json(state.storage.change(change).await?))
}

/// `/store/...`. The local socket has no peer and counts as this machine.
pub async fn forward(
    State(state): State<Arc<AppState>>,
    peer: Option<Extension<Peer>>,
    request: Request,
) -> Response {
    let caller =
        peer.map_or(IpAddr::from([127, 0, 0, 1]), |Extension(Peer(ip))| ip);
    state.storage.forward(request, caller).await
}
