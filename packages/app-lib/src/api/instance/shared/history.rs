//! The owner's local history of a share: each version this App pushed, kept with its files in the
//! App's content store (`sharing_client::history`). The History tab reads it, and restoring a
//! version reads only it, never the pool.

use super::client::*;
use super::diff::*;
use super::install::shared_instance_install_data;
use super::publish::*;
use super::types::*;
use super::*;
use sharing_client::history::{self, HistoryEntry, HistoryError, HistoryStore};
use sharing_client::install::{ConfigUpdate, config_update};
use sharing_client::push::Pushed;
use sharing_client::versions::VersionContent;
use std::path::{Path, PathBuf};

/// How many entries each share keeps: a number, or `all`. Unset keeps
/// [`history::DEFAULT_KEPT`].
const KEPT_SETTING: &str = "share_history_kept";

/// The App's history: entries in the App's database, files retained in the content store.
struct AppHistory<'a>(&'a State);

impl HistoryStore for AppHistory<'_> {
    type Error = crate::Error;

    fn db(&self) -> &sqlx::SqlitePool {
        &self.0.pool
    }

    async fn kept(&self) -> crate::Result<Option<usize>> {
        let setting =
            crate::state::content_store::setting(&self.0.pool, KEPT_SETTING)
                .await?;
        Ok(match setting.as_deref() {
            None => Some(history::DEFAULT_KEPT),
            Some("all") => None,
            Some(kept) => Some(kept.parse().map_err(|_| {
                crate::ErrorKind::InputError(format!(
                    "the kept versions setting {kept} is invalid"
                ))
            })?),
        })
    }

    async fn store_file(&self, path: &Path) -> crate::Result<String> {
        Ok(self.0.content_store.store_file(path).await?.metadata.sha512)
    }

    async fn blob_path(&self, blob: &str) -> crate::Result<Option<PathBuf>> {
        Ok(self
            .0
            .content_store
            .lookup(Some(blob), None)
            .await?
            .map(|stored| stored.path))
    }

    async fn retain(&self, owner: &str, blobs: &[String]) -> crate::Result<()> {
        self.0
            .content_store
            .retain(history::OWNER_KIND, owner, blobs)
            .await
    }

    async fn release(&self, owner: &str) -> crate::Result<()> {
        self.0
            .content_store
            .release(history::OWNER_KIND, owner)
            .await
    }
}

fn history_error(error: HistoryError<crate::Error>) -> crate::Error {
    match error {
        HistoryError::Store(error) => error,
        error => crate::ErrorKind::InputError(error.to_string()).into(),
    }
}

/// Records a push of `share_id`, naming what it changed over the version before it
/// (`sharing_client::history::record_push`).
pub(super) async fn record_push(
    share_id: &str,
    content: &VersionContent,
    pushed: &Pushed,
    state: &State,
) {
    let after = ShareVersion {
        version: pushed.version,
        ready: true,
        content: content.clone(),
        root: [0; 32],
        external_files: pushed.external_files.clone(),
        pool: None,
        store: None,
        backtrack_version: None,
    };
    let diffs = async {
        shared_version_diffs(pushed.previous.as_ref(), &after, state)
            .await
            .map(|(diffs, _)| diffs)
    };
    history::record_push(&AppHistory(state), share_id, content, pushed, diffs)
        .await;
}

/// The last push of `share_id` from this App: the newest entry of its history.
pub(crate) async fn last_shared_push(
    share_id: &str,
    state: &State,
) -> crate::Result<Option<HistoryEntry>> {
    history::latest(&AppHistory(state), share_id).await
}

/// Deletes the whole history of `share_id`, once the share is gone.
pub(super) async fn forget_share_history(
    share_id: &str,
    state: &State,
) -> crate::Result<()> {
    history::prune(&AppHistory(state), share_id, Some(0)).await
}

/// The instance's share history, newest first. Only its owner has one.
#[tracing::instrument]
pub async fn get_shared_instance_history(
    instance_id: &str,
) -> crate::Result<Vec<SharedInstanceHistoryEntry>> {
    let state = State::get().await?;
    let attachment = attachment(instance_id, &state).await?;
    if attachment.role != SharedInstanceRole::Owner {
        return Ok(Vec::new());
    }
    Ok(history::list(&AppHistory(&state), &attachment.id)
        .await?
        .into_iter()
        .map(history_entry)
        .collect())
}

fn history_entry(entry: HistoryEntry) -> SharedInstanceHistoryEntry {
    let version = entry_version(&entry);
    let (loader, loader_version) = instance_loader(&version);
    let config_files = entry
        .diffs
        .iter()
        .find(|diff| {
            matches!(
                diff.type_,
                SharedInstanceUpdateDiffType::ConfigFilesUpdated
            )
        })
        .map(|diff| diff.config_file_names.clone())
        .unwrap_or_default();
    SharedInstanceHistoryEntry {
        version: entry.version,
        created: DateTime::from_timestamp_millis(entry.created)
            .unwrap_or_default(),
        details: SharedInstanceVersionDetails {
            content_count: entry.content.modrinth_ids.len()
                + version
                    .external_files
                    .iter()
                    .filter(|file| file.file_type != CONFIG_FILE_TYPE)
                    .count(),
            game_version: entry.content.game_version,
            loader,
            loader_version,
            diffs: entry.diffs,
            config_files,
        },
    }
}

/// An entry as the share version it recorded, for the code that reads versions. Its files are
/// read from the history, so it names no pool.
fn entry_version(entry: &HistoryEntry) -> ShareVersion {
    ShareVersion {
        version: entry.version,
        ready: true,
        content: entry.content.clone(),
        root: [0; 32],
        external_files: entry.manifest().cloned().collect(),
        pool: None,
        store: None,
        backtrack_version: None,
    }
}

/// Puts the owner's instance back to `version` from its history. What the share carried is
/// replaced; what the owner added since their last push stays. Nothing reaches members until the
/// owner pushes, so the instance is left with changes to push unless the version is the latest.
#[tracing::instrument]
pub async fn restore_shared_instance_version(
    instance_id: &str,
    version: i32,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    let attachment = owner_attachment(instance_id, &state).await?;
    let store = AppHistory(&state);
    let plan = history::restore_plan(&store, &attachment.id, version)
        .await
        .map_err(history_error)?;
    let last_push = history::latest(&store, &attachment.id).await?;
    let restored = entry_version(&plan.entry);
    // Every config of the version is written back. A config the last push shared and the
    // version does not is removed while it is still as pushed.
    let last_files =
        last_push.map(|entry| entry.manifest().cloned().collect::<Vec<_>>());
    let configs = ConfigUpdate {
        write: restored.configs().cloned().collect(),
        left: config_update(&restored.external_files, last_files.as_deref())
            .left,
    };
    let data = shared_instance_install_data(
        &attachment.id,
        attachment.manager_id.clone(),
        attachment.server_manager_name.clone(),
        attachment.server_manager_icon_url.clone(),
        None,
        metadata.instance.name,
        restored,
        configs,
        Some(&plan.files),
        &state,
    )
    .await?;
    crate::install::update_shared_instance(instance_id.to_string(), Some(data))
        .await
}

/// Deletes `version` from the instance's share history. The newest entry is the last push,
/// which restores compare with, so it stays.
#[tracing::instrument]
pub async fn delete_shared_instance_version(
    instance_id: &str,
    version: i32,
) -> crate::Result<()> {
    let state = State::get().await?;
    let attachment = owner_attachment(instance_id, &state).await?;
    let store = AppHistory(&state);
    if history::latest(&store, &attachment.id)
        .await?
        .is_some_and(|latest| latest.version == version)
    {
        bail!(crate::ErrorKind::InputError(
            "the latest version cannot be deleted".to_string(),
        ));
    }
    history::delete(&store, &attachment.id, version).await
}

/// How many versions each share's history keeps, and how many each has now.
#[tracing::instrument]
pub async fn get_shared_history_settings()
-> crate::Result<SharedHistorySettings> {
    let state = State::get().await?;
    Ok(SharedHistorySettings {
        kept: AppHistory(&state).kept().await?,
        entries: history::counts(&state.pool).await?,
    })
}

/// Keeps the newest `kept` versions of every share's history, all of them when `None`, and
/// deletes the rest now.
#[tracing::instrument]
pub async fn set_shared_history_kept(
    kept: Option<usize>,
) -> crate::Result<SharedHistorySettings> {
    let state = State::get().await?;
    let value = kept.map_or_else(|| "all".to_string(), |kept| kept.to_string());
    crate::state::content_store::set_setting(&state.pool, KEPT_SETTING, &value)
        .await?;
    let store = AppHistory(&state);
    for share_id in history::counts(&state.pool).await?.into_keys() {
        history::prune(&store, &share_id, kept).await?;
    }
    get_shared_history_settings().await
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

async fn owner_attachment(
    instance_id: &str,
    state: &State,
) -> crate::Result<SharedInstanceAttachment> {
    let attachment = attachment(instance_id, state).await?;
    if attachment.role != SharedInstanceRole::Owner {
        bail!(crate::ErrorKind::InputError(
            "only the owner has a shared instance's history".to_string(),
        ));
    }
    Ok(attachment)
}
