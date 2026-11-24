use axum::Router;
use contracts::{
    ApiError, Json,
    server::{Mount, Req},
};

use super::internal;
use crate::local_api::endpoints::desktop::CheckNetworkMetered;

pub fn routes() -> Router {
    Router::new().endpoint::<CheckNetworkMetered, _, _>(is_metered)
}

async fn is_metered(
    _: Req<CheckNetworkMetered>,
) -> Result<Json<bool>, ApiError> {
    engine::prelude::is_network_metered()
        .await
        .map(Json)
        .map_err(internal)
}
