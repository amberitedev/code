use std::collections::HashMap;

use axum::{
    Router,
    extract::{DefaultBodyLimit, State},
};
use contracts::{
    ApiError, Empty, ErrorCode, Json,
    server::{Mount, Req},
};
use engine::routes::api_error;
use engine::worlds::{self, DisplayStatus, World, WorldWithInstance};
use enumset::EnumSet;

use super::{groups, join_log};
use crate::local_api::{
    AppState,
    endpoints::{
        groups::{
            CreateInstanceGroup, DeleteInstanceGroup,
            ListInstanceGroupMemberships, ListInstanceGroups,
            RenameInstanceGroup, SetInstanceGroupMemberships,
            SetInstanceGroupOrder,
        },
        library::{
            CacheGeneratedIcon, CacheIcon, ListHiddenInstances,
            ListInstanceWorlds, ListRecentIconConfigs, ListRecentWorlds,
            RecentIconConfigs, SetGeneratedInstanceIcon, SetInstanceHidden,
        },
    },
};
use crate::state::Desktop;

pub fn routes() -> Router<AppState> {
    Router::new()
        .endpoint::<ListRecentIconConfigs, _, _>(list_recent_icon_configs)
        .endpoint::<ListHiddenInstances, _, _>(list_hidden_instances)
        .endpoint::<SetInstanceHidden, _, _>(set_instance_hidden)
        .endpoint::<CacheIcon, _, _>(cache_icon)
        .endpoint::<CacheGeneratedIcon, _, _>(cache_generated_icon)
        .endpoint::<SetGeneratedInstanceIcon, _, _>(set_generated_instance_icon)
        .endpoint::<ListRecentWorlds, _, _>(list_recent_worlds)
        .endpoint::<ListInstanceWorlds, _, _>(list_instance_worlds)
        .endpoint::<ListInstanceGroups, _, _>(list_instance_groups)
        .endpoint::<ListInstanceGroupMemberships, _, _>(
            list_instance_group_memberships,
        )
        .endpoint::<CreateInstanceGroup, _, _>(create_instance_group)
        .endpoint::<RenameInstanceGroup, _, _>(rename_instance_group)
        .endpoint::<DeleteInstanceGroup, _, _>(delete_instance_group)
        .endpoint::<SetInstanceGroupOrder, _, _>(set_instance_group_order)
        .endpoint::<SetInstanceGroupMemberships, _, _>(
            set_instance_group_memberships,
        )
        // The engine caps icons and symbols at 4 MiB, past axum's default body limit.
        .layer(DefaultBodyLimit::disable())
}

async fn list_recent_icon_configs(
    State(engine): State<Desktop>,
    _: Req<ListRecentIconConfigs>,
) -> Result<Json<RecentIconConfigs>, ApiError> {
    super::recent_icons::list(engine.db())
        .await
        .map(|configs| Json(RecentIconConfigs(configs)))
        .map_err(engine::routes::api_error)
}

async fn list_hidden_instances(
    State(engine): State<Desktop>,
    _: Req<ListHiddenInstances>,
) -> Result<Json<Vec<String>>, ApiError> {
    crate::library::get_hidden_instance_ids(&engine)
        .await
        .map(Json)
        .map_err(engine::routes::api_error)
}

async fn set_instance_hidden(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<SetInstanceHidden>,
) -> Result<Empty, ApiError> {
    crate::library::set_instance_hidden(&engine, &body.instance_id, body.hidden)
        .await
        .map(|()| Empty)
        .map_err(engine::routes::api_error)
}

async fn cache_icon(
    State(engine): State<Desktop>,
    request: Req<CacheIcon>,
) -> Result<Json<String>, ApiError> {
    engine::instance::cache_icon_bytes(&engine, request.body.bytes.to_vec())
        .await
        .map(Json)
        .map_err(engine::routes::api_error)
}

async fn cache_generated_icon(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<CacheGeneratedIcon>,
) -> Result<Json<String>, ApiError> {
    let icon = engine::instance::cache_generated_icon(
        &engine,
        body.config.clone(),
        body.symbol_bytes,
    )
    .await
    .map_err(engine::routes::api_error)?;
    if body.add_to_recents {
        super::recent_icons::record(&body.config, engine.db())
            .await
            .map_err(engine::routes::api_error)?;
    }
    Ok(Json(icon))
}

async fn set_generated_instance_icon(
    State(engine): State<Desktop>,
    Req { params, body, .. }: Req<SetGeneratedInstanceIcon>,
) -> Result<Json<Option<String>>, ApiError> {
    let id = &params.id;
    let icon = if body.only_if_empty {
        engine::instance::edit_generated_icon_if_empty(
            &engine,
            id,
            body.config.clone(),
            body.symbol_bytes,
        )
        .await
    } else {
        engine::instance::edit_generated_icon(
            &engine,
            id,
            body.config.clone(),
            body.symbol_bytes,
        )
        .await
        .map(Some)
    }
    .map_err(engine::routes::api_error)?;
    if icon.is_some() {
        super::recent_icons::record(&body.config, engine.db())
            .await
            .map_err(engine::routes::api_error)?;
    }
    Ok(Json(icon))
}

async fn list_recent_worlds(
    State(desktop): State<Desktop>,
    request: Req<ListRecentWorlds>,
) -> Result<Json<Vec<WorldWithInstance>>, ApiError> {
    let display_statuses = match request.query.display_statuses {
        Some(statuses) => serde_json::from_str::<Vec<DisplayStatus>>(&statuses)
            .map_err(|error| {
                ApiError::new(ErrorCode::InvalidInput, error.to_string())
            })?
            .into_iter()
            .collect(),
        None => EnumSet::all(),
    };
    let joins = join_log::all_joins(desktop.db()).await.map_err(api_error)?;
    worlds::get_recent_worlds(
        &desktop,
        request.query.limit,
        display_statuses,
        &joins,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn list_instance_worlds(
    State(desktop): State<Desktop>,
    request: Req<ListInstanceWorlds>,
) -> Result<Json<Vec<World>>, ApiError> {
    let joins = join_log::joins(desktop.db(), &request.params.id)
        .await
        .map_err(api_error)?;
    worlds::get_instance_worlds(&desktop, &request.params.id, &joins)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn list_instance_groups(
    State(desktop): State<Desktop>,
    _: Req<ListInstanceGroups>,
) -> Result<Json<Vec<groups::InstanceGroup>>, ApiError> {
    groups::list_groups(desktop.db())
        .await
        .map(Json)
        .map_err(api_error)
}

async fn list_instance_group_memberships(
    State(desktop): State<Desktop>,
    _: Req<ListInstanceGroupMemberships>,
) -> Result<Json<HashMap<String, Vec<String>>>, ApiError> {
    groups::list_memberships(desktop.db())
        .await
        .map(Json)
        .map_err(api_error)
}

async fn create_instance_group(
    State(desktop): State<Desktop>,
    request: Req<CreateInstanceGroup>,
) -> Result<Json<groups::InstanceGroup>, ApiError> {
    groups::create_group(&desktop, request.body.name)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn rename_instance_group(
    State(desktop): State<Desktop>,
    request: Req<RenameInstanceGroup>,
) -> Result<Json<groups::InstanceGroup>, ApiError> {
    groups::rename_group(&desktop, request.params.id, request.body.name)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn delete_instance_group(
    State(desktop): State<Desktop>,
    request: Req<DeleteInstanceGroup>,
) -> Result<Empty, ApiError> {
    groups::delete_group(&desktop, request.params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn set_instance_group_order(
    State(desktop): State<Desktop>,
    request: Req<SetInstanceGroupOrder>,
) -> Result<Empty, ApiError> {
    groups::set_group_order(&desktop, request.body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn set_instance_group_memberships(
    State(desktop): State<Desktop>,
    request: Req<SetInstanceGroupMemberships>,
) -> Result<Empty, ApiError> {
    groups::set_group_memberships(&desktop, request.body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
