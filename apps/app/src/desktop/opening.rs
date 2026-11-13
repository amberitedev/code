//! Serves `crate::server::endpoints::desktop::GetOpeningCommand`.

use axum::{Router, extract::State};
use contracts::{
    ApiError, Json,
    server::{Req, route},
};

use super::AppSlot;
use crate::server::{endpoints::desktop::GetOpeningCommand, events::Command};

pub fn routes(app: AppSlot) -> Router {
    let (path, method) =
        route::<GetOpeningCommand, _, _, _>(get_opening_command);
    Router::new().route(path, method).with_state(app)
}

async fn get_opening_command(
    State(app): State<AppSlot>,
    _: Req<GetOpeningCommand>,
) -> Result<Json<Option<Command>>, ApiError> {
    crate::api::utils::get_opening_command(app.wait().await)
        .await
        .map(Json)
        .map_err(engine::routes::api_error)
}
