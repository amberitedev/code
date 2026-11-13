use std::{collections::HashMap, sync::Arc};

use axum::extract::State;
use contracts::{ApiError, Empty, Json, server::Req};
use modrinth_content_management::ResolveContentPlan;

use super::{api_error, files::inside_instance};
use crate::{
    Engine,
    access::{Edit, EngineAccess, InstanceAccess, Owner, View},
    endpoints::content::{
        AddProjectFromVersion, CheckProjectInstalled, GetLinkedModpack,
        InstallProjectWithDependencies, ListContentItems,
        ListDependencyContentItems, ListInstallCandidates,
        ListInstalledProjectIds, ListInstanceProjects,
        ListLinkedModpackContent, ListPackExportCandidates,
        RefreshContentUpdates, RemoveProject, RepairModpack, SetProjectLocked,
        SwitchProjectVersion, SyncContentFiles, ToggleProject,
        UpdateModpackVersion, UpdateProject,
    },
    install::InstallJobSnapshot,
    instance::{self, PackExportCandidate},
    state::{
        ContentFile, ContentItem, InstanceInstallCandidate, LinkedModpackInfo,
    },
};

type Answer<T> = Result<Json<T>, ApiError>;

pub async fn list_instance_projects(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<View>,
    request: Req<ListInstanceProjects>,
) -> Answer<HashMap<String, ContentFile>> {
    instance::get_projects(&engine, &instance.id, request.query.cache_behaviour)
        .await
        .map(|projects| Json(projects.into_iter().collect()))
        .map_err(api_error)
}

pub async fn list_installed_project_ids(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<View>,
    _: Req<ListInstalledProjectIds>,
) -> Answer<Vec<String>> {
    instance::get_installed_project_ids(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

/// `false` when the instance's content cannot be read.
pub async fn check_project_installed(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<View>,
    request: Req<CheckProjectInstalled>,
) -> Answer<bool> {
    let project_id = &request.query.project_id;
    let installed = instance::get_projects(&engine, &instance.id, None)
        .await
        .is_ok_and(|projects| {
            projects.iter().any(|project| {
                project
                    .metadata
                    .as_ref()
                    .is_some_and(|metadata| &metadata.project_id == project_id)
            })
        });
    Ok(Json(installed))
}

pub async fn list_install_candidates(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<ListInstallCandidates>,
) -> Answer<Vec<InstanceInstallCandidate>> {
    let query = request.body;
    instance::get_install_candidates(
        &engine,
        &query.project_id,
        query.project_type,
        query.targets,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn list_content_items(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<View>,
    request: Req<ListContentItems>,
) -> Answer<Vec<ContentItem>> {
    instance::get_content_items(
        &engine,
        &instance.id,
        request.query.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn refresh_content_updates(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    _: Req<RefreshContentUpdates>,
) -> Result<Empty, ApiError> {
    instance::refresh_content_updates(&engine, &instance.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn sync_content_files(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    _: Req<SyncContentFiles>,
) -> Result<Empty, ApiError> {
    instance::sync_content_files(&engine, &instance.id)
        .await
        .map(|_| Empty)
        .map_err(api_error)
}

pub async fn list_dependency_content_items(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<ListDependencyContentItems>,
) -> Answer<Vec<ContentItem>> {
    let query = request.body;
    instance::get_dependencies_as_content_items(
        &engine,
        query.dependencies,
        query.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn get_linked_modpack(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<View>,
    request: Req<GetLinkedModpack>,
) -> Answer<Option<LinkedModpackInfo>> {
    instance::get_linked_modpack_info(
        &engine,
        &instance.id,
        request.query.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn list_linked_modpack_content(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<View>,
    request: Req<ListLinkedModpackContent>,
) -> Answer<Vec<ContentItem>> {
    instance::get_linked_modpack_content(
        &engine,
        &instance.id,
        request.query.cache_behaviour,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn add_project_from_version(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<AddProjectFromVersion>,
) -> Answer<String> {
    let project = request.body;
    instance::add_project_from_version(
        &engine,
        &instance.id,
        &project.version_id,
        project.reason,
        project.dependent_on_version_id,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn install_project_with_dependencies(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<InstallProjectWithDependencies>,
) -> Answer<ResolveContentPlan> {
    instance::install_project_with_dependencies(
        &engine,
        &instance.id,
        request.body,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn update_project(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<UpdateProject>,
) -> Answer<String> {
    let path = inside_instance(&request.body.project_path)?;
    instance::update_project(&engine, &instance.id, path, None)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn switch_project_version(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<SwitchProjectVersion>,
) -> Answer<String> {
    let switch = request.body;
    instance::switch_project_version_with_dependencies(
        &engine,
        &instance.id,
        inside_instance(&switch.project_path)?,
        &switch.version_id,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn toggle_project(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<ToggleProject>,
) -> Answer<String> {
    let toggle = request.body;
    instance::toggle_disable_project(
        &engine,
        &instance.id,
        inside_instance(&toggle.project_path)?,
        toggle.desired_enabled,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn set_project_locked(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<SetProjectLocked>,
) -> Result<Empty, ApiError> {
    let lock = request.body;
    instance::set_project_locked(
        &engine,
        &instance.id,
        inside_instance(&lock.project_path)?,
        lock.locked,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

pub async fn remove_project(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<RemoveProject>,
) -> Result<Empty, ApiError> {
    let path = inside_instance(&request.body.project_path)?;
    instance::remove_project(&engine, &instance.id, path)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn update_modpack_version(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<UpdateModpackVersion>,
) -> Answer<InstallJobSnapshot> {
    instance::update_managed_modrinth_version(
        &engine,
        &instance.id,
        &request.body.version_id,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn repair_modpack(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    _: Req<RepairModpack>,
) -> Answer<InstallJobSnapshot> {
    instance::repair_managed_modrinth(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn list_pack_export_candidates(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<ListPackExportCandidates>,
) -> Answer<Vec<PackExportCandidate>> {
    instance::get_pack_export_candidates_for_parent(
        &engine,
        &instance.id,
        request.query.parent,
    )
    .await
    .map(Json)
    .map_err(api_error)
}
