use std::sync::Arc;

use axum::extract::State;
use contracts::{ApiError, Empty, Json, server::Req};

use super::api_error;
use crate::{
    Engine,
    access::{Edit, InstanceAccess, Owner, View},
    endpoints::instances::{
        EditInstance, GetOptimalJava, KillInstance, RemoveInstance,
    },
    instance,
    state::JavaVersion,
};

pub async fn remove_instance(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    _: Req<RemoveInstance>,
) -> Result<Empty, ApiError> {
    instance::remove(&engine, &instance.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn edit_instance(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<EditInstance>,
) -> Result<Empty, ApiError> {
    let patch = request.body.into_core().map_err(api_error)?;
    instance::edit(&engine, &instance.id, patch)
        .await
        .map(|_| Empty)
        .map_err(api_error)
}

pub async fn kill_instance(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    _: Req<KillInstance>,
) -> Result<Empty, ApiError> {
    instance::kill(&engine, &instance.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn get_optimal_java(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<View>,
    _: Req<GetOptimalJava>,
) -> Result<Json<Option<JavaVersion>>, ApiError> {
    instance::get_optimal_jre_key(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}
