//! Servers follow an instance through its share, as one more member. The App only makes sure
//! the instance has a share and tells a server's Core its id; Core reads the latest version from
//! the backend itself, fetches its files, and installs them (`content::shared` in Core). No file
//! goes from the App to the server.

use super::publish::{owned_shared_instance, shared_attachment};
use super::*;
use crate::state::VersionEnvironment;

/// The instance's share, created with its first version when it has none, so the instance can
/// run servers. A share nobody else is in is still a share; it just has no members.
#[tracing::instrument]
pub async fn ensure_shared_instance(
    instance_id: &str,
) -> crate::Result<SharedInstanceAttachment> {
    let state = State::get().await?;
    if let Some(attachment) = shared_attachment(instance_id, &state).await? {
        return Ok(attachment);
    }
    let (_, attachment) = owned_shared_instance(instance_id, &state).await?;
    emit_instance(instance_id, InstancePayloadType::Edited).await?;
    Ok(attachment)
}

/// The Modrinth versions among `ids` that players never run, so a change to one is for servers
/// only.
pub(super) async fn server_only_versions(
    ids: impl IntoIterator<Item = &String>,
    state: &State,
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
