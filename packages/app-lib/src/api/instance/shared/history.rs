//! A share's kept versions: what each one changed, and restoring the owner's instance to one.

use super::client::*;
use super::diff::*;
use super::install::shared_instance_install_data;
use super::publish::*;
use super::types::*;
use super::*;

/// `version` of the instance's share, with what it changed over `base`, the kept version before
/// it. Without a base, everything the version holds is added.
#[tracing::instrument]
pub async fn get_shared_instance_version(
    instance_id: &str,
    version: i32,
    base: Option<i32>,
) -> crate::Result<SharedInstanceVersionDetails> {
    let state = State::get().await?;
    let attachment = attachment(instance_id, &state).await?;
    let (after, before) = tokio::try_join!(
        kept_version(&attachment.id, version, &state),
        async {
            match base {
                Some(base) => {
                    kept_version(&attachment.id, base, &state).await.map(Some)
                }
                None => Ok(None),
            }
        },
    )?;
    let diffs = shared_version_diffs(before.as_ref(), &after, &state).await?;
    let config_files = super::content::changed_config_files(
        &after.external_files,
        before
            .as_ref()
            .map(|version| version.external_files.as_slice()),
    )?
    .into_iter()
    .filter(|file| file.file_type == CONFIG_FILE_TYPE)
    .map(|file| file.file_name)
    .collect();
    let (version_ids, external_files) = remote_shared_content(&after)?;
    Ok(SharedInstanceVersionDetails {
        content_count: version_ids.len() + external_files.len(),
        game_version: after.game_version,
        loader: after.loader,
        loader_version: after.loader_version,
        diffs,
        config_files,
    })
}

/// Puts the owner's instance back to `version` of its share, overwriting changes not yet
/// pushed. Nothing reaches members until the owner pushes, so the instance is left with
/// changes to push unless the version is the latest.
#[tracing::instrument]
pub async fn restore_shared_instance_version(
    instance_id: &str,
    version: i32,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    let attachment = attachment(instance_id, &state).await?;
    if attachment.role != SharedInstanceRole::Owner {
        bail!(crate::ErrorKind::InputError(
            "only the owner can restore a shared instance version".to_string(),
        ));
    }
    let version = kept_version(&attachment.id, version, &state).await?;
    // No applied version, so every config file of the restored version is written.
    let data = shared_instance_install_data(
        &attachment.id,
        attachment.manager_id.clone(),
        attachment.server_manager_name.clone(),
        attachment.server_manager_icon_url.clone(),
        None,
        metadata.instance.name,
        version,
        None,
        &state,
    )
    .await?;
    crate::install::update_shared_instance(instance_id.to_string(), data).await
}

async fn attachment(
    instance_id: &str,
    state: &State,
) -> crate::Result<SharedInstanceAttachment> {
    shared_attachment(instance_id, state).await?.ok_or_else(|| {
        crate::ErrorKind::InputError(
            "instance is not attached to a shared instance".to_string(),
        )
        .into()
    })
}

async fn kept_version(
    shared_instance_id: &str,
    version: i32,
    state: &State,
) -> crate::Result<InstanceVersionResponse> {
    get_remote_version(shared_instance_id, version, state)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "version {version} is no longer kept"
            ))
            .into()
        })
}
