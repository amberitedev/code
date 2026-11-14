use std::{collections::HashMap, path::PathBuf, sync::Arc};

use axum::extract::State;
use contracts::{ApiError, Empty, Json, server::Req};

use super::api_error;
use crate::{
    Engine,
    access::{EngineAccess, Owner},
    endpoints::jre::{
        FindJavaInstalls, GetJavaVersions, GetSystemMemory, InstallJava,
        SetJavaVersion,
    },
    jre,
    state::JavaVersion,
};

pub async fn get_java_versions(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    _: Req<GetJavaVersions>,
) -> Result<Json<HashMap<u32, JavaVersion>>, ApiError> {
    jre::get_java_versions(&engine)
        .await
        .map(|versions| Json(versions.into_iter().collect()))
        .map_err(api_error)
}

pub async fn set_java_version(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<SetJavaVersion>,
) -> Result<Empty, ApiError> {
    jre::set_java_version(&engine, request.body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn find_java_installs(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<FindJavaInstalls>,
) -> Result<Json<Vec<JavaVersion>>, ApiError> {
    jre::find_filtered_jres(&engine, request.query.version)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn install_java(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<InstallJava>,
) -> Result<Json<PathBuf>, ApiError> {
    jre::auto_install_java(&engine, request.body)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn get_system_memory(
    _: EngineAccess<Owner>,
    _: Req<GetSystemMemory>,
) -> Result<Json<u64>, ApiError> {
    jre::get_max_memory().await.map(Json).map_err(api_error)
}
