use crate::ErrorKind;
use crate::event::InstancePayloadType;
use crate::event::emit::emit_instance;
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::{
    CacheBehaviour, CachedEntry, ContentSourceKind, Engine, ProjectType,
};
use crate::util::fetch;
use anyhow::bail;
use modrinth_content_management::{
    ContentType, ResolutionPreferences, ResolveContentPlan,
};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstallProjectWithDependenciesRequest {
    pub project_id: String,
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub version_id: Option<String>,
    pub content_type: ContentType,
    #[serde(default)]
    #[cfg_attr(
        feature = "export-ts",
        ts(as = "Option<ResolutionPreferences>", optional)
    )]
    pub selected: ResolutionPreferences,
}

#[tracing::instrument(skip(state))]
pub async fn update_project(
    state: &Engine,
    instance_id: &str,
    project_path: &str,
    skip_send_event: Option<bool>,
) -> crate::Result<String> {
    ensure_shared_instance_can_modify_project(instance_id, project_path, state)
        .await?;
    ensure_project_not_frozen(instance_id, project_path, state).await?;
    let path = crate::state::instances::commands::update_project(
        instance_id,
        project_path,
        state,
    )
    .await?;
    state
        .hooks()
        .content_changed(state, instance_id, &[project_path, &path])
        .await;
    if !skip_send_event.unwrap_or(false) {
        emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
            .await?;
    }

    Ok(path)
}

#[tracing::instrument(skip(state))]
pub async fn add_project_from_version(
    state: &Engine,
    instance_id: &str,
    version_id: &str,
    reason: fetch::DownloadReason,
    dependent_on_version_id: Option<String>,
) -> crate::Result<String> {
    ensure_instance_content_unlocked(instance_id, state).await?;
    let project_path =
        crate::state::instances::commands::add_project_from_version(
            instance_id,
            version_id,
            reason,
            dependent_on_version_id,
            crate::state::ContentSourceKind::Local,
            state,
        )
        .await?;
    state
        .hooks()
        .content_installed(state, instance_id, &project_path)
        .await;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;

    Ok(project_path)
}

#[tracing::instrument(skip(state))]
pub async fn install_project_with_dependencies(
    state: &Arc<Engine>,
    instance_id: &str,
    request: InstallProjectWithDependenciesRequest,
) -> crate::Result<ResolveContentPlan> {
    let metadata = super::get::get(state, instance_id)
        .await?
        .ok_or(ErrorKind::InputError("unknown instance".to_string()))?;
    ensure_metadata_content_unlocked(&metadata)?;
    let plan = crate::state::instances::commands::resolve_install_plan(
        instance_id,
        crate::state::instances::commands::InstanceInstallProjectRequest {
            project_id: request.project_id,
            version_id: request.version_id,
            content_type: request.content_type,
            selected: request.selected,
        },
        state,
    )
    .await?;

    let instance_id = metadata.instance.id;
    let project_ids = plan_project_ids(&plan);
    let install_plan = plan.clone();
    let state = Arc::clone(state);
    tokio::spawn(async move {
        let result = async {
			let paths = crate::state::instances::commands::install_resolved_content_plan(
				&instance_id,
				&install_plan,
				&state,
			).await?;
			for path in paths {
				state.hooks().content_installed(&state, &instance_id, &path).await;
			}
			Ok::<(), crate::Error>(())
		}.await;
        match result {
            Ok(()) => {
                if let Err(error) = emit_instance(
                    &state.events,
                    &instance_id,
                    InstancePayloadType::ContentInstallFinished {
                        project_ids: project_ids.clone(),
                    },
                )
                .await
                {
                    tracing::error!(
                        "Failed to emit content install finished event: {error}"
                    );
                }
                if let Err(error) = emit_instance(
                    &state.events,
                    &instance_id,
                    InstancePayloadType::Edited,
                )
                .await
                {
                    tracing::error!(
                        "Failed to emit instance edited event after content install: {error}"
                    );
                }
            }
            Err(error) => {
                if let Err(emit_error) = emit_instance(
                    &state.events,
                    &instance_id,
                    InstancePayloadType::ContentInstallFailed {
                        project_ids,
                        message: error.to_string(),
                    },
                )
                .await
                {
                    tracing::error!(
                        "Failed to emit content install failed event: {emit_error}"
                    );
                }
            }
        }
    });

    Ok(plan)
}

fn plan_project_ids(plan: &ResolveContentPlan) -> Vec<String> {
    let mut project_ids = Vec::with_capacity(plan.dependencies.len() + 1);
    project_ids.push(plan.primary.project_id.clone());
    project_ids.extend(
        plan.dependencies
            .iter()
            .map(|dependency| dependency.project_id.clone()),
    );
    project_ids
}

#[tracing::instrument(skip(state))]
pub async fn switch_project_version_with_dependencies(
    state: &Engine,
    instance_id: &str,
    project_path: &str,
    version_id: &str,
) -> crate::Result<String> {
    ensure_shared_instance_can_modify_project(instance_id, project_path, state)
        .await?;
    ensure_project_not_frozen(instance_id, project_path, state).await?;
    let metadata = super::get::get(state, instance_id)
        .await?
        .ok_or(ErrorKind::InputError("unknown instance".to_string()))?;
    let path =
        crate::state::instances::commands::switch_project_version_with_dependencies(
            instance_id,
            project_path,
            version_id,
            state,
        )
        .await?;
    state
        .hooks()
        .content_changed(state, instance_id, &[project_path, &path])
        .await;
    emit_instance(
        &state.events,
        &metadata.instance.id,
        InstancePayloadType::Edited,
    )
    .await?;

    Ok(path)
}

#[tracing::instrument(skip(state))]
pub async fn add_project_from_path(
    state: &Engine,
    instance_id: &str,
    path: &Path,
    project_type: Option<ProjectType>,
) -> crate::Result<String> {
    ensure_instance_content_unlocked(instance_id, state).await?;
    let project_path =
        crate::state::instances::commands::add_project_from_path(
            instance_id,
            path,
            project_type,
            state,
        )
        .await?;
    state
        .hooks()
        .content_installed(state, instance_id, &project_path)
        .await;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;

    Ok(project_path)
}

#[tracing::instrument(skip(state))]
pub async fn is_file_on_modrinth(
    state: &Engine,
    path: &Path,
) -> crate::Result<bool> {
    let (_, hash) = fetch::sha1_file_async(path).await?;
    let files = CachedEntry::get_file_many(
        &[&hash],
        Some(CacheBehaviour::Bypass),
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?;

    Ok(!files.is_empty())
}

#[tracing::instrument(skip(state))]
pub async fn toggle_disable_project(
    state: &Engine,
    instance_id: &str,
    project: &str,
    desired_enabled: Option<bool>,
) -> crate::Result<String> {
    ensure_shared_instance_can_modify_project(instance_id, project, state)
        .await?;
    let res = crate::state::instances::commands::toggle_disable_project(
        instance_id,
        project,
        desired_enabled,
        state,
    )
    .await?;
    state
        .hooks()
        .content_changed(state, instance_id, &[project])
        .await;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;

    Ok(res)
}

#[tracing::instrument(skip(state))]
pub async fn remove_project(
    state: &Engine,
    instance_id: &str,
    project: &str,
) -> crate::Result<()> {
    ensure_shared_instance_can_modify_project(instance_id, project, state)
        .await?;
    crate::state::instances::commands::remove_project(
        instance_id,
        project,
        state,
    )
    .await?;
    state
        .hooks()
        .content_changed(state, instance_id, &[project])
        .await;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;

    Ok(())
}

#[tracing::instrument(skip(state))]
pub async fn set_project_locked(
    state: &Engine,
    instance_id: &str,
    project: &str,
    locked: bool,
) -> crate::Result<()> {
    ensure_shared_instance_can_modify_project(instance_id, project, state)
        .await?;
    crate::state::instances::commands::set_project_locked(
        instance_id,
        project,
        locked,
        state,
    )
    .await?;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;

    Ok(())
}

async fn ensure_shared_instance_can_modify_project(
    instance_id: &str,
    project_path: &str,
    state: &Engine,
) -> crate::Result<()> {
    let metadata = crate::state::instances::commands::get_instance_metadata(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or(ErrorKind::InputError("unknown instance".to_string()))?;
    ensure_metadata_content_unlocked(&metadata)?;
    if !metadata
        .shared_instance
        .is_some_and(|attachment| attachment.role.is_member())
    {
        return Ok(());
    }

    let source_kind =
        crate::state::instances::commands::content_source_kind_for_project_path(
            instance_id,
            project_path,
            state,
        )
        .await?;
    if source_kind.is_some_and(ContentSourceKind::is_shared_instance_managed) {
        bail!(ErrorKind::InputError(
            "shared instance managed content cannot be changed directly"
                .to_string(),
        ));
    }

    Ok(())
}

async fn ensure_project_not_frozen(
    instance_id: &str,
    project_path: &str,
    state: &Engine,
) -> crate::Result<()> {
    if crate::state::instances::commands::is_project_locked(
        instance_id,
        project_path,
        state,
    )
    .await?
    {
        bail!(ErrorKind::InputError(
            "frozen content cannot change versions; unfreeze it first"
                .to_string(),
        ));
    }

    Ok(())
}

/// Installs a Modrinth modpack version over the instance, which then follows that modpack.
#[tracing::instrument(skip(state))]
pub async fn install_modrinth_modpack(
    state: &Arc<Engine>,
    instance_id: &str,
    project_id: &str,
    version_id: &str,
) -> crate::Result<crate::install::InstallJobSnapshot> {
    let metadata = crate::state::instances::commands::get_instance_metadata(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or(ErrorKind::InputError("unknown instance".to_string()))?;
    ensure_metadata_content_unlocked(&metadata)?;
    crate::install::install_pack_to_existing_instance(
        metadata.instance.id,
        crate::api::pack::install_from::CreatePackLocation::FromVersionId {
            project_id: project_id.to_string(),
            version_id: version_id.to_string(),
            title: metadata.instance.name.clone(),
            icon_url: None,
        },
        None,
        state,
    )
    .await
}

#[tracing::instrument(skip(state))]
pub async fn update_managed_modrinth_version(
    state: &Arc<Engine>,
    instance_id: &str,
    version_id: &str,
) -> crate::Result<crate::install::InstallJobSnapshot> {
    let metadata = crate::state::instances::commands::get_instance_metadata(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or(ErrorKind::InputError("unknown instance".to_string()))?;
    ensure_metadata_content_unlocked(&metadata)?;

    let post_install_edit = match &metadata.link {
        crate::state::InstanceLink::ServerProjectModpack {
            server_project_id,
            content_project_id,
            ..
        } => Some(crate::install::InstallPostInstallEdit {
            name: Some(metadata.instance.name.clone()),
            icon_path: Some(metadata.instance.icon_path.clone()),
            link: Some(crate::state::InstanceLink::ServerProjectModpack {
                server_project_id: server_project_id.clone(),
                content_project_id: content_project_id.clone(),
                content_version_id: version_id.to_string(),
            }),
        }),
        _ => None,
    };

    let project_id = match &metadata.link {
        crate::state::InstanceLink::ModrinthModpack { project_id, .. } => {
            project_id.clone()
        }
        crate::state::InstanceLink::ServerProjectModpack {
            content_project_id,
            ..
        } => content_project_id.clone(),
        _ => bail!(unmanaged_pack_error(&metadata.instance.id)),
    };

    crate::install::install_pack_to_existing_instance(
        metadata.instance.id,
        crate::api::pack::install_from::CreatePackLocation::FromVersionId {
            project_id,
            version_id: version_id.to_string(),
            title: metadata.instance.name.clone(),
            icon_url: None,
        },
        post_install_edit,
        state,
    )
    .await
}

#[tracing::instrument(skip(state))]
pub async fn repair_managed_modrinth(
    state: &Arc<Engine>,
    instance_id: &str,
) -> crate::Result<crate::install::InstallJobSnapshot> {
    let metadata = crate::state::instances::commands::get_instance_metadata(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or(ErrorKind::InputError("unknown instance".to_string()))?;
    ensure_metadata_content_unlocked(&metadata)?;

    let post_install_edit = match &metadata.link {
        crate::state::InstanceLink::ServerProjectModpack { .. } => {
            Some(crate::install::InstallPostInstallEdit {
                name: Some(metadata.instance.name.clone()),
                icon_path: Some(metadata.instance.icon_path.clone()),
                link: Some(metadata.link.clone()),
            })
        }
        _ => None,
    };

    let (project_id, version_id) = match &metadata.link {
        crate::state::InstanceLink::ModrinthModpack {
            project_id,
            version_id,
        } => (project_id.clone(), version_id.clone()),
        crate::state::InstanceLink::ServerProjectModpack {
            content_project_id,
            content_version_id,
            ..
        } => (content_project_id.clone(), content_version_id.clone()),
        _ => bail!(unmanaged_pack_error(&metadata.instance.id)),
    };

    crate::install::install_pack_to_existing_instance(
        metadata.instance.id,
        crate::api::pack::install_from::CreatePackLocation::FromVersionId {
            project_id,
            version_id,
            title: metadata.instance.name.clone(),
            icon_url: None,
        },
        post_install_edit,
        state,
    )
    .await
}

fn unmanaged_pack_error(instance_id: &str) -> ErrorKind {
    ErrorKind::InputError(format!(
        "instance {instance_id} is not a managed Modrinth pack, or has been disconnected"
    ))
}

async fn ensure_instance_content_unlocked(
    instance_id: &str,
    state: &Engine,
) -> crate::Result<()> {
    if instance_rows::is_instance_quarantined(instance_id, &state.pool).await? {
        bail!(quarantined_content_error());
    }

    let instance = instance_rows::get_instance_by_id(instance_id, &state.pool)
        .await?
        .ok_or(ErrorKind::InputError("unknown instance".to_string()))?;
    ensure_installation_content_unlocked(instance.install_stage)
}

pub fn ensure_metadata_content_unlocked(
    metadata: &crate::state::InstanceMetadata,
) -> crate::Result<()> {
    if metadata.quarantined {
        bail!(quarantined_content_error());
    }

    ensure_installation_content_unlocked(metadata.instance.install_stage)
}

pub(super) fn ensure_installation_content_unlocked(
    stage: crate::state::InstanceInstallStage,
) -> crate::Result<()> {
    if matches!(
        stage,
        crate::state::InstanceInstallStage::MinecraftInstalling
            | crate::state::InstanceInstallStage::PackInstalling
    ) && crate::install::control::CURRENT_INSTALL
        .try_with(|_| ())
        .is_err()
    {
        bail!(ErrorKind::InputError(
            "content cannot be changed while this instance is installing or updating"
                .to_string(),
        ));
    }
    Ok(())
}

fn quarantined_content_error() -> ErrorKind {
    ErrorKind::InputError(
        "content in quarantined instances cannot be changed".to_string(),
    )
}
