use std::sync::Arc;

use axum::extract::State;
use contracts::{ApiError, Empty, Json, server::Req};

use super::api_error;
use crate::{
    Engine,
    access::{EngineAccess, Owner},
    endpoints::groups::{
        CreateInstanceGroup, DeleteInstanceGroup, ListInstanceGroups,
        RenameInstanceGroup, SetInstanceGroupMemberships,
        SetInstanceGroupOrder,
    },
    instance::{self, InstanceGroup},
};

pub async fn list_instance_groups(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    _: Req<ListInstanceGroups>,
) -> Result<Json<Vec<InstanceGroup>>, ApiError> {
    instance::list_groups(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn create_instance_group(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<CreateInstanceGroup>,
) -> Result<Json<InstanceGroup>, ApiError> {
    instance::create_group(&engine, request.body.name)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn rename_instance_group(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<RenameInstanceGroup>,
) -> Result<Json<InstanceGroup>, ApiError> {
    instance::rename_group(&engine, request.params.id, request.body.name)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn delete_instance_group(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<DeleteInstanceGroup>,
) -> Result<Empty, ApiError> {
    instance::delete_group(&engine, request.params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn set_instance_group_order(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<SetInstanceGroupOrder>,
) -> Result<Empty, ApiError> {
    instance::set_group_order(&engine, request.body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn set_instance_group_memberships(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<SetInstanceGroupMemberships>,
) -> Result<Empty, ApiError> {
    instance::set_group_memberships(&engine, request.body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
