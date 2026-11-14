use std::sync::Arc;

use axum::extract::State;
use contracts::{ApiError, Empty, Json, server::Req};

use super::api_error;
use crate::{
    Engine,
    access::{EngineAccess, Owner},
    cache,
    endpoints::cache::{
        GetCachedOrganization, GetCachedProject, GetCachedProjectV3,
        GetCachedProjectVersions, GetCachedProjects, GetCachedProjectsV3,
        GetCachedSearchResults, GetCachedSearchResultsV3, GetCachedTeam,
        GetCachedUser, GetCachedUsers, GetCachedVersion, GetCachedVersions,
        PurgeCache,
    },
    state::{
        Organization, Project, ProjectV3, SearchResults, SearchResultsV3,
        TeamMember, User, Version,
    },
};

fn as_strs(ids: &[String]) -> Vec<&str> {
    ids.iter().map(String::as_str).collect()
}

pub async fn get_cached_project(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedProject>,
) -> Result<Json<Option<Project>>, ApiError> {
    cache::get_project(
        &engine,
        &request.params.id,
        request.query.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn get_cached_projects(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedProjects>,
) -> Result<Json<Vec<Project>>, ApiError> {
    cache::get_project_many(
        &engine,
        &as_strs(&request.body.ids),
        request.body.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn get_cached_project_versions(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedProjectVersions>,
) -> Result<Json<Option<Vec<Version>>>, ApiError> {
    cache::get_project_versions(
        &engine,
        &request.params.id,
        request.query.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn get_cached_project_v3(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedProjectV3>,
) -> Result<Json<Option<ProjectV3>>, ApiError> {
    cache::get_project_v3(
        &engine,
        &request.params.id,
        request.query.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn get_cached_projects_v3(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedProjectsV3>,
) -> Result<Json<Vec<ProjectV3>>, ApiError> {
    cache::get_project_v3_many(
        &engine,
        &as_strs(&request.body.ids),
        request.body.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn get_cached_version(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedVersion>,
) -> Result<Json<Option<Version>>, ApiError> {
    cache::get_version(
        &engine,
        &request.params.id,
        request.query.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn get_cached_versions(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedVersions>,
) -> Result<Json<Vec<Version>>, ApiError> {
    cache::get_version_many(
        &engine,
        &as_strs(&request.body.ids),
        request.body.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn get_cached_user(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedUser>,
) -> Result<Json<Option<User>>, ApiError> {
    cache::get_user(&engine, &request.params.id, request.query.cache_behaviour)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn get_cached_users(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedUsers>,
) -> Result<Json<Vec<User>>, ApiError> {
    cache::get_user_many(
        &engine,
        &as_strs(&request.body.ids),
        request.body.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn get_cached_team(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedTeam>,
) -> Result<Json<Option<Vec<TeamMember>>>, ApiError> {
    cache::get_team(&engine, &request.params.id, request.query.cache_behaviour)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn get_cached_organization(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedOrganization>,
) -> Result<Json<Option<Organization>>, ApiError> {
    cache::get_organization(
        &engine,
        &request.params.id,
        request.query.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn get_cached_search_results(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedSearchResults>,
) -> Result<Json<Option<SearchResults>>, ApiError> {
    cache::get_search_results(
        &engine,
        &request.query.id,
        request.query.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn get_cached_search_results_v3(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetCachedSearchResultsV3>,
) -> Result<Json<Option<SearchResultsV3>>, ApiError> {
    cache::get_search_results_v3(
        &engine,
        &request.query.id,
        request.query.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn purge_cache(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<PurgeCache>,
) -> Result<Empty, ApiError> {
    cache::purge_cache_types(&engine, &request.body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
