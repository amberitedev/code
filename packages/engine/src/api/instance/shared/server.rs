//! Servers follow an instance through its share, as one more member. The App only makes sure
//! the instance has a share and tells a server's Core its id; Core reads the latest version from
//! the backend itself, fetches its files, and installs them (`content::shared` in Core). No file
//! goes from the App to the server.

use super::publish::{
    attach_new_shared_instance, owned_shared_instance, shared_attachment,
};
use super::*;
use crate::state::VersionEnvironment;

/// The instance's share, created with its first version when it has none, so the instance can
/// run servers. A share nobody else is in is still a share; it just has no members.
#[tracing::instrument(skip(state))]
pub async fn ensure_shared_instance(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<SharedInstanceAttachment> {
    if let Some(attachment) = shared_attachment(instance_id, state).await? {
        return Ok(attachment);
    }
    let (_, attachment) = owned_shared_instance(instance_id, state).await?;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;
    Ok(attachment)
}

/// The instance's share, created with no version when it has none, for its owner to publish the
/// first once there is something to share: a Core shares a server that is still installing.
#[tracing::instrument(skip(state))]
pub async fn ensure_unpublished_shared_instance(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<SharedInstanceAttachment> {
    if let Some(attachment) = shared_attachment(instance_id, state).await? {
        return Ok(attachment);
    }
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    attach_new_shared_instance(&metadata, state).await?;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;
    shared_attachment(instance_id, state).await?.ok_or_else(|| {
        crate::ErrorKind::InputError(
            "shared instance attachment was not persisted".to_string(),
        )
        .into()
    })
}

/// What applying the latest version of share `shared_instance_id` to server `instance_id` takes,
/// read for the user who sent `read_ticket`. It goes to [`crate::install::update_shared_instance`],
/// which fetches the version's pool files for as long as the version's fetch ticket lasts.
#[tracing::instrument(skip(state, read_ticket))]
pub async fn followed_shared_instance_version(
    state: &Engine,
    instance_id: &str,
    shared_instance_id: &str,
    read_ticket: &str,
) -> crate::Result<SharedInstanceInstallData> {
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    let version = super::client::get_latest_remote_version_with_ticket(
        shared_instance_id,
        read_ticket,
        state,
    )
    .await?;
    let applied = metadata
        .shared_instance
        .filter(|attachment| attachment.id == shared_instance_id)
        .and_then(|attachment| attachment.applied_version);
    let configs = super::content::remote_config_update(
        Some(instance_id),
        applied,
        &version,
        state,
    )
    .await?;
    super::install::shared_instance_install_data(
        shared_instance_id,
        None,
        None,
        None,
        None,
        None,
        metadata.instance.name,
        version,
        configs,
        None,
        state,
    )
    .await
}

/// The Modrinth versions among `ids` that players never run, so a change to one is for servers
/// only.
pub(super) async fn server_only_versions(
    ids: impl IntoIterator<Item = &String>,
    state: &Engine,
) -> crate::Result<HashSet<String>> {
    let mut ids = ids.into_iter().map(String::as_str).collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return Ok(HashSet::new());
    }
    Ok(CachedEntry::get_version_v3_many(
        &ids,
        None,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?
    .into_iter()
    .filter(|version| {
        matches!(
            version.environment,
            Some(VersionEnvironment::DedicatedServerOnly)
        )
    })
    .map(|version| version.id)
    .collect())
}
