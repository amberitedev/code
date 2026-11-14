use axum::Router;
use contracts::{
    ApiError, Json,
    server::{Req, route},
};

use super::internal;
use crate::server::endpoints::desktop::CheckNetworkMetered;

pub fn routes() -> Router {
    let (path, handler) = route::<CheckNetworkMetered, _, _, _>(is_metered);
    Router::new().route(path, handler)
}

async fn is_metered(
    _: Req<CheckNetworkMetered>,
) -> Result<Json<bool>, ApiError> {
    engine::prelude::is_network_metered()
        .await
        .map(Json)
        .map_err(internal)
}
