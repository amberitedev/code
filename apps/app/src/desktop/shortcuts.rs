//! Serves `crate::server::endpoints::desktop::CreateShortcut`.

use std::path::PathBuf;

use axum::Router;
use contracts::{
    ApiError, Json,
    server::{Req, route},
};

use super::internal;
use crate::{
    api::shortcuts::create_instance_shortcut,
    server::endpoints::desktop::CreateShortcut,
};

pub fn routes() -> Router {
    let (path, handler) = route::<CreateShortcut, _, _, _>(create_shortcut);
    Router::new().route(path, handler)
}

async fn create_shortcut(
    Req { body, .. }: Req<CreateShortcut>,
) -> Result<Json<PathBuf>, ApiError> {
    create_instance_shortcut(
        body.instance_name,
        body.instance_id,
        body.output_path,
        body.server,
        body.singleplayer_world,
    )
    .await
    .map(Json)
    .map_err(|error| internal(format!("{error:#}")))
}
