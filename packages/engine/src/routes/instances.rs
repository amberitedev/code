use std::sync::Arc;

use axum::extract::State;
use contracts::{ApiError, ErrorCode, Json, server::Req};

use super::api_error;
use crate::{
    Engine,
    access::{Access, Caller, InstanceAccess, Permission, View},
    endpoints::instances::{
        GetInstance, GetInstances, Instance, ListInstances,
    },
    state::InstanceMetadata,
};

pub async fn list_instances(
    State(engine): State<Arc<Engine>>,
    State(access): State<Arc<dyn Access>>,
    caller: Caller,
    _: Req<ListInstances>,
) -> Result<Json<Vec<Instance>>, ApiError> {
    let instances = crate::instance::list(&engine).await.map_err(api_error)?;
    Ok(Json(visible(&*access, &caller, instances).await))
}

pub async fn get_instances(
    State(engine): State<Arc<Engine>>,
    State(access): State<Arc<dyn Access>>,
    caller: Caller,
    request: Req<GetInstances>,
) -> Result<Json<Vec<Instance>>, ApiError> {
    let ids: Vec<String> =
        serde_json::from_str(&request.query.ids).map_err(|error| {
            ApiError::new(ErrorCode::InvalidInput, error.to_string())
        })?;
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    let instances = crate::instance::get_many(&engine, &ids)
        .await
        .map_err(api_error)?;
    Ok(Json(visible(&*access, &caller, instances).await))
}

pub async fn get_instance(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<View>,
    _: Req<GetInstance>,
) -> Result<Json<Option<Instance>>, ApiError> {
    let found = crate::instance::get(&engine, &instance.id)
        .await
        .map_err(api_error)?;
    Ok(Json(found.map(Instance::from)))
}

async fn visible(
    access: &dyn Access,
    caller: &Caller,
    instances: Vec<InstanceMetadata>,
) -> Vec<Instance> {
    let mut visible = Vec::with_capacity(instances.len());
    for instance in instances {
        if access
            .require(caller, Some(&instance.instance.id), Permission::View)
            .await
            .is_ok()
        {
            visible.push(instance.into());
        }
    }
    visible
}
