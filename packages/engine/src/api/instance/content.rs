use crate::state::{
    CacheBehaviour, ContentFile, ContentItem, ContentSet, Dependency, Engine,
    InstanceInstallCandidate, InstanceInstallTarget, LinkedModpackInfo,
    ProjectType,
};
use dashmap::DashMap;

#[tracing::instrument(skip(state))]
pub async fn sync_content_files(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<Vec<crate::state::instances::InstanceFile>> {
    crate::state::sync_content_files(instance_id, state).await
}

#[tracing::instrument(skip(state))]
pub async fn list_content_sets(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<Vec<ContentSet>> {
    crate::state::list_content_sets(instance_id, &state.pool).await
}

#[tracing::instrument(skip(state))]
pub async fn get_projects(
    state: &Engine,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<DashMap<String, ContentFile>> {
    crate::state::get_content_projects(
        instance_id,
        None,
        cache_behaviour,
        state,
    )
    .await
}

#[tracing::instrument(skip(state))]
pub async fn get_installed_project_ids(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<Vec<String>> {
    crate::state::get_installed_project_ids_for_instance(
        instance_id,
        None,
        state,
    )
    .await
}

#[tracing::instrument(skip(state))]
pub async fn get_install_candidates(
    state: &Engine,
    project_id: &str,
    project_type: ProjectType,
    targets: Vec<InstanceInstallTarget>,
) -> crate::Result<Vec<InstanceInstallCandidate>> {
    crate::state::get_instance_install_candidates(
        project_id,
        project_type,
        &targets,
        &state.pool,
    )
    .await
}

#[tracing::instrument(skip(state))]
pub async fn get_content_items(
    state: &Engine,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<ContentItem>> {
    let mut items =
        crate::state::list_indexed_content(instance_id, cache_behaviour, state)
            .await?;
    super::synced_packs::decorate_content(instance_id, &mut items, state)
        .await?;
    Ok(items)
}

#[tracing::instrument(skip(state))]
pub async fn refresh_content_updates(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<()> {
    crate::state::refresh_content_updates(instance_id, state).await
}

#[tracing::instrument(skip(state))]
pub async fn get_linked_modpack_content(
    state: &Engine,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<ContentItem>> {
    crate::state::list_linked_modpack_content(
        instance_id,
        None,
        cache_behaviour,
        state,
    )
    .await
}

#[tracing::instrument(skip(state))]
pub async fn get_dependencies_as_content_items(
    state: &Engine,
    dependencies: Vec<Dependency>,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<ContentItem>> {
    crate::state::dependencies_to_content_items(
        &dependencies,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await
}

#[tracing::instrument(skip(state))]
pub async fn get_linked_modpack_info(
    state: &Engine,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Option<LinkedModpackInfo>> {
    crate::state::get_linked_modpack_info(
        instance_id,
        None,
        cache_behaviour,
        state,
    )
    .await
}
