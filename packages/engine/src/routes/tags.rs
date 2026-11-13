use std::sync::Arc;

use axum::extract::State;
use contracts::{ApiError, Json, server::Req};

use super::api_error;
use crate::{
    Engine,
    access::{EngineAccess, Owner},
    endpoints::tags::{GetCategoryTags, GetGameVersionTags, GetLoaderTags},
    tags::{self, Category, GameVersion, Loader},
};

pub async fn get_category_tags(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    _: Req<GetCategoryTags>,
) -> Result<Json<Vec<Category>>, ApiError> {
    tags::get_category_tags(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn get_loader_tags(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    _: Req<GetLoaderTags>,
) -> Result<Json<Vec<Loader>>, ApiError> {
    tags::get_loader_tags(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn get_game_version_tags(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    _: Req<GetGameVersionTags>,
) -> Result<Json<Vec<GameVersion>>, ApiError> {
    tags::get_game_version_tags(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}
