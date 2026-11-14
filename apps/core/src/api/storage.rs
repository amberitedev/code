//! Storage for the shared file pool (see `crate::storage`). Its settings are the owner's;
//! `/store/...`, every method, is forwarded to the store unchanged. Pool tickets, not sessions,
//! decide who may use it, and the store checks them itself.

use std::{net::IpAddr, sync::Arc};

use axum::{
    Extension,
    extract::{Request, State},
    response::Response,
};
use contracts::{
    ApiError, Json,
    core::{ChangeStorage, GetStorage, StorageStatus},
    server::Req,
};

use super::CoreState;
use crate::access::{Caller, Peer, require_owner};

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<GetStorage>,
) -> Result<Json<StorageStatus>, ApiError> {
    require_owner(&caller)?;
    Ok(Json(state.storage.status().await?))
}

pub async fn settings(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    request: Req<ChangeStorage>,
) -> Result<Json<StorageStatus>, ApiError> {
    require_owner(&caller)?;
    Ok(Json(state.storage.change(request.body).await?))
}

/// `/store/...`. The local socket has no peer and counts as this machine.
pub async fn forward(
    State(state): State<Arc<CoreState>>,
    peer: Option<Extension<Peer>>,
    request: Request,
) -> Response {
    let caller =
        peer.map_or(IpAddr::from([127, 0, 0, 1]), |Extension(Peer(ip))| ip);
    state.storage.forward(request, caller).await
}
