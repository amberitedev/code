//! The share of a server-first server, a server that follows no instance, which this Core writes
//! with its own key (`docs/internals/plan.md`, "Shares, members, and players"): `POST|DELETE
//! /v1/servers/{id}/worlds/{wid}/content/share`, `GET .../content/share/diff`, and
//! `GET .../content/share/files`. Mirrors `Archon.Content.v1` (`archon/content/v1.ts`).
//!
//! Also the share's history on this Core, for the owner alone (plan.md, "Links and history"):
//! `GET .../content/share/history`, `POST .../content/share/history/{version}/restore` and
//! `DELETE .../content/share/history/{version}`.
//!
//! A server that follows an instance gets 409 on all of them: it reads its instance's share,
//! and each share has one writer. What a version carries is `commands::share_snapshot`, plus the
//! config files the push picks.

use std::{collections::BTreeSet, path::Path as FsPath, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sharing_client::{
    backend::{Backend, BackendError},
    diff::{DiffError, SharedInstanceUpdateDiff, share_diff},
    history::SharedInstanceHistoryEntry,
    push,
    rules::{
        CONFIG_FILE_TYPE, FolderEntry, SCAN_HEAD_SIZE, ShareFolder,
        list_shareable,
    },
};
use ts_rs::TS;

use super::{
    AppState,
    activity::Action,
    content::{server_first, world},
    error::{ApiError, ErrorCode},
};
use crate::{
    access::{Caller, Level, activity},
    state::servers::{
        adapters::{
            filesystem::folder::{self, Folder},
            sqlite::server_rows,
        },
        commands::{
            delete_share_version::delete_share_version,
            drop_own_share::drop_own_share,
            get_share_history::get_share_history,
            record_push::record_push,
            restore_share_version::{Restored, restore_share_version},
            share_snapshot::{
                Names, SCOPE, push_options, share_contents, snapshot,
            },
        },
    },
};

/// Config files `GET .../content/share/files` lists at most.
const MAX_LISTED: usize = 2000;

/// Body of POST `.../content/share`. Mirrors the body of `ArchonContentV1Module.share`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ShareRequest {
    /// Files under `config/` to send with this version. Configs sent earlier stay shared while
    /// the server has them, unless they are in `removed_config_paths`.
    #[serde(default)]
    pub config_paths: Vec<String>,
    /// Configs sent earlier that this version stops sharing.
    #[serde(default)]
    pub removed_config_paths: Vec<String>,
    /// Restart a running server once the version is out.
    #[serde(default)]
    pub restart: Option<bool>,
}

/// Mirrors Archon.Content.v1.ShareWorldContentResponse.
#[derive(Debug, Clone, Serialize)]
pub struct ShareWorldContentResponse {
    pub shared_instance_id: String,
    pub version: Option<i32>,
    pub restarted: bool,
}

/// Mirrors Archon.Content.v1.ShareableFile.
#[derive(Debug, Clone, Serialize)]
pub struct ShareableFile {
    pub path: String,
    pub size: u64,
    /// Under `config/`, but a share cannot carry it (see `sharing_client::rules`).
    pub blocked: bool,
    /// The share's latest version carries it.
    pub shared: bool,
}

/// Mirrors Archon.Content.v1.ShareableFilesResponse.
#[derive(Debug, Clone, Serialize)]
pub struct ShareableFilesResponse {
    pub files: Vec<ShareableFile>,
    pub truncated: bool,
}

/// Response of GET `.../content/share/diff`: what a push would change from the latest version,
/// in the App's diff shape (`sharing_client::diff`). Mirrors
/// Archon.Content.v1.SharedInstancePublishDiff.
#[derive(Debug, Clone, Serialize, TS)]
pub struct SharedInstancePublishDiff {
    pub shared_instance_id: String,
    pub latest_version: i32,
    pub has_changes: bool,
    /// The server is running, so it runs these changes only once it restarts.
    pub requires_restart: bool,
    pub diffs: Vec<SharedInstanceUpdateDiff>,
}

/// The backend's share routes, reached with this Core's key.
pub(super) async fn backend(state: &AppState) -> Result<Backend, ApiError> {
    let key: Option<String> =
        sqlx::query_scalar("SELECT backend_key FROM core WHERE id = 1")
            .fetch_optional(&state.db)
            .await?
            .flatten();
    match (state.storage.backend_url(), key) {
        (Some(url), Some(key)) => {
            Ok(Backend::new(state.content.http.clone(), url, key))
        }
        _ => Err(ApiError::new(
            ErrorCode::Conflict,
            "Pair this Core with an account to share its servers or follow an instance.",
        )),
    }
}

impl From<push::PushError> for ApiError {
    fn from(error: push::PushError) -> Self {
        use push::PushError;
        let code = match &error {
            PushError::Executable(_)
            | PushError::TooLarge
            | PushError::Refused(_) => ErrorCode::InvalidRequest,
            PushError::NoStorage
            | PushError::PushInFlight
            | PushError::FileChanged(_)
            | PushError::Unavailable(_) => ErrorCode::Conflict,
            PushError::Backend(error) => return Self::from_backend(error),
            _ => ErrorCode::Internal,
        };
        Self::new(code, error.to_string())
    }
}

impl From<BackendError> for ApiError {
    fn from(error: BackendError) -> Self {
        Self::from_backend(&error)
    }
}

impl ApiError {
    /// A share that is gone or no longer reachable with this Core's key (404, 401) is a
    /// conflict the caller can resolve; anything else is the backend's fault.
    fn from_backend(error: &BackendError) -> Self {
        let code = match error {
            BackendError::Unavailable { .. } => ErrorCode::Conflict,
            _ => ErrorCode::Internal,
        };
        Self::new(code, error.to_string())
    }
}

/// Copy `path` out of the server folder to `to`, so the push reads bytes no link or later write
/// can change.
fn stage(folder: &Folder, path: &str, to: &FsPath) -> std::io::Result<()> {
    let mut file = folder.open_file(path)?.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{path} is not there"),
        )
    })?;
    std::io::copy(&mut file, &mut std::fs::File::create(to)?)?;
    Ok(())
}

/// `POST .../content/share`: push a version of the server's share, creating the share on the
/// first push.
pub async fn share(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<ShareRequest>,
) -> Result<Json<ShareWorldContentResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    server_first(&state, &id).await?;
    let mut configs = Vec::with_capacity(request.config_paths.len());
    for path in &request.config_paths {
        let path = folder::clean(path.trim_start_matches('/'))
            .map_err(crate::state::servers::Error::from)?;
        if !SCOPE.carries(&path) {
            return Err(ApiError::new(
                ErrorCode::InvalidRequest,
                format!("{path} cannot be shared."),
            ));
        }
        configs.push(path);
    }
    let backend = backend(&state).await?;
    let snapshot = snapshot(&state.content, &id).await?;
    // The share takes the server's name on every push. One deleted elsewhere, or made by this
    // Core under an earlier pairing, is replaced by a new one.
    let existing = match server_rows::share_id(&state.db, &id).await? {
        Some(share_id) => {
            match backend.rename_share(&share_id, &snapshot.name).await {
                Err(error)
                    if matches!(
                        error.status(),
                        Some(
                            reqwest::StatusCode::FORBIDDEN
                                | reqwest::StatusCode::NOT_FOUND
                        )
                    ) =>
                {
                    None
                }
                result => result.map(|()| Some(share_id))?,
            }
        }
        None => None,
    };
    let share_id = match existing {
        Some(share_id) => share_id,
        None => {
            let share_id =
                backend.create_server_share(&snapshot.name, &id).await?;
            server_rows::set_share_id(&state.db, &id, Some(&share_id)).await?;
            share_id
        }
    };

    let staging = state
        .content
        .paths
        .staging(&id)
        .join(uuid::Uuid::new_v4().to_string());
    let sources = snapshot
        .carried
        .iter()
        .map(|row| {
            (
                row.file_name.clone(),
                row.file_type.clone(),
                row.path.clone(),
            )
        })
        .chain(
            configs
                .into_iter()
                .map(|path| (path.clone(), CONFIG_FILE_TYPE.to_owned(), path)),
        )
        .collect::<Vec<_>>();
    let locate =
        locate_config(Arc::clone(&snapshot.folder), staging.join("kept"));
    let result = async {
        let files = {
            let folder = Arc::clone(&snapshot.folder);
            let staging = staging.clone();
            tokio::task::spawn_blocking(move || {
                std::fs::create_dir_all(&staging)?;
                sources
                    .into_iter()
                    .enumerate()
                    .map(|(n, (file_name, file_type, on_disk))| {
                        let path = staging.join(n.to_string());
                        stage(&folder, &on_disk, &path)?;
                        Ok(push::PushFile {
                            file_name,
                            file_type,
                            path,
                        })
                    })
                    .collect::<std::io::Result<Vec<_>>>()
            })
            .await
            .map_err(std::io::Error::other)??
        };
        let pushed = push::push(
            &backend,
            &state.content.store,
            &share_id,
            &snapshot.content,
            None,
            files,
            push::KeptConfigs {
                scope: SCOPE,
                removed: &request.removed_config_paths,
                locate: &locate,
            },
            &staging.join("pool"),
            &mut |event| {
                if let push::PushEvent::Dropped(names) = event {
                    tracing::info!(server = %id, ?names, "stopped sharing config files");
                }
            },
        )
        .await?;
        // Recorded while the staged copies are still there.
        record_push(&state.content, &snapshot, &share_id, &pushed).await;
        Ok::<_, ApiError>(pushed.version)
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&staging).await;
    let version = result?;

    let restarted =
        request.restart == Some(true) && state.servers.phase(&id).is_some();
    if restarted {
        let (state, id) = (Arc::clone(&state), id.clone());
        tokio::spawn(async move {
            if let Err(error) = state.servers.restart(&id).await {
                tracing::warn!(server = %id, %error, "restart failed");
            }
        });
    }
    Ok(Json(ShareWorldContentResponse {
        shared_instance_id: share_id,
        version: Some(version),
        restarted,
    }))
}

/// Where the push finds a config the latest version shares: a copy under `staging` of the
/// server's file, while the server has it.
fn locate_config(
    folder: Arc<Folder>,
    staging: std::path::PathBuf,
) -> impl Fn(
    String,
) -> futures::future::BoxFuture<
    'static,
    std::io::Result<Option<std::path::PathBuf>>,
> + Send
+ Sync {
    move |file_name: String| {
        let (folder, staging) = (Arc::clone(&folder), staging.clone());
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                std::fs::create_dir_all(&staging)?;
                let to = staging.join(uuid::Uuid::new_v4().to_string());
                match stage(&folder, &file_name, &to) {
                    Ok(()) => Ok(Some(to)),
                    Err(error)
                        if error.kind() == std::io::ErrorKind::NotFound =>
                    {
                        Ok(None)
                    }
                    Err(error) => Err(error),
                }
            })
            .await
            .map_err(std::io::Error::other)?
        })
    }
}

/// `DELETE .../content/share`: delete the server's share. Everyone who had it stops getting
/// updates. The backend is owed the deletion (`commands::drop_own_share`), which reaches it once
/// it can, so an unpaired Core deletes it when it pairs again or never needs to.
pub async fn unshare(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Owner).await?;
    world(&id, &wid)?;
    server_first(&state, &id).await?;
    if !drop_own_share(&state.content, &state.access.owed, &id).await? {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            "This server is not shared.",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `GET .../content/share/files`: the config files a push can carry.
pub async fn files(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Json<ShareableFilesResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    server_first(&state, &id).await?;
    let folder = Folder::open(&state.servers.paths().server(&id))?;
    let shared = shared_configs(&state, &id).await;
    let response =
        tokio::task::spawn_blocking(move || list_configs(&folder, &shared))
            .await
            .map_err(std::io::Error::other)??;
    Ok(Json(response))
}

/// The configs the latest version of the server's share carries. None when it has no share or
/// the backend cannot be read now, which only leaves the files unmarked.
async fn shared_configs(state: &AppState, id: &str) -> BTreeSet<String> {
    let latest = async {
        let Some(share_id) = server_rows::share_id(&state.db, id).await? else {
            return Ok(None);
        };
        Ok::<_, ApiError>(backend(state).await?.latest_ready(&share_id).await?)
    };
    match latest.await {
        Ok(latest) => latest
            .iter()
            .flat_map(|version| version.configs())
            .map(|file| file.file_name.clone())
            .collect(),
        Err(error) => {
            tracing::warn!(server = %id, error = %error.description, "could not read the share's configs");
            BTreeSet::new()
        }
    }
}

/// Every file under `config/` the share reaches (`rules::list_shareable`), without following
/// links.
fn list_configs(
    folder: &Folder,
    shared: &BTreeSet<String>,
) -> std::io::Result<ShareableFilesResponse> {
    let listed =
        list_shareable(SCOPE, &ServerFolder(folder), shared, MAX_LISTED)?;
    Ok(ShareableFilesResponse {
        files: listed
            .files
            .into_iter()
            .map(|file| ShareableFile {
                path: file.path,
                size: file.size,
                blocked: file.blocked,
                shared: file.shared,
            })
            .collect(),
        truncated: listed.truncated,
    })
}

/// The server folder as the share's file listing reads it, through `Folder`, which never
/// follows a link.
struct ServerFolder<'a>(&'a Folder);

impl ShareFolder for ServerFolder<'_> {
    fn entries(&self, path: &str) -> std::io::Result<Vec<FolderEntry>> {
        let dir = self.0.open_dir((!path.is_empty()).then_some(path))?;
        let mut entries = Vec::new();
        for entry in dir.entries()?.filter_map(Result::ok) {
            let (Ok(name), Ok(kind)) =
                (entry.file_name().into_string(), entry.file_type())
            else {
                continue;
            };
            if kind.is_dir() {
                entries.push(FolderEntry::Dir(name));
            } else if kind.is_file() {
                let size = entry.metadata().map_or(0, |meta| meta.len());
                entries.push(FolderEntry::File { name, size });
            }
        }
        Ok(entries)
    }

    fn head(&self, path: &str) -> Vec<u8> {
        use std::io::Read;

        let mut head = Vec::with_capacity(SCAN_HEAD_SIZE);
        if let Ok(Some(file)) = self.0.open_file(path) {
            let _ = file.take(SCAN_HEAD_SIZE as u64).read_to_end(&mut head);
        }
        head
    }
}

/// `GET .../content/share/diff`: what a push would change. 404 while the share has no version
/// players can install, so the App pushes straight away.
pub async fn diff(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Json<SharedInstancePublishDiff>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    server_first(&state, &id).await?;
    let not_shared = || {
        ApiError::new(ErrorCode::NotFound, "This server has no shared version.")
    };
    let share_id = server_rows::share_id(&state.db, &id)
        .await?
        .ok_or_else(not_shared)?;
    let latest = backend(&state)
        .await?
        .latest_ready(&share_id)
        .await?
        .ok_or_else(not_shared)?;
    let snapshot = snapshot(&state.content, &id).await?;
    let (before, after) =
        share_contents(&snapshot, &latest.content, &latest.external_files)
            .await?;
    let diffs =
        share_diff(&before, &after, &push_options(), &Names(&state.content))
            .await
            .map_err(|error| match error {
                DiffError::MissingVersion(_) => {
                    ApiError::new(ErrorCode::Conflict, error.to_string())
                }
                DiffError::Names(error) => {
                    crate::state::servers::Error::from(error).into()
                }
            })?;
    let has_changes = !diffs.is_empty();
    Ok(Json(SharedInstancePublishDiff {
        shared_instance_id: share_id,
        latest_version: latest.version,
        has_changes,
        requires_restart: has_changes && state.servers.phase(&id).is_some(),
        diffs,
    }))
}

/// `GET .../content/share/history`: the versions of the server's share this Core pushed, newest
/// first, as the App lists an instance's.
pub async fn history(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Json<Vec<SharedInstanceHistoryEntry>>, ApiError> {
    caller.require(&state.db, &id, Level::Owner).await?;
    world(&id, &wid)?;
    server_first(&state, &id).await?;
    Ok(Json(get_share_history(&state.content, &id).await?))
}

/// `POST .../content/share/history/{version}/restore`: put the server back to a version, to
/// push again. 204 once it is back; 202 while the version's loader or pack installs first.
pub async fn restore(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid, version)): Path<(String, String, i32)>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Owner).await?;
    world(&id, &wid)?;
    server_first(&state, &id).await?;
    let busy = state.content.busy(&id)?;
    let restored =
        restore_share_version(&state.content, &id, busy, version).await?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(match restored {
        Restored::Now => StatusCode::NO_CONTENT,
        Restored::Background => StatusCode::ACCEPTED,
    })
}

/// `DELETE .../content/share/history/{version}`: delete a version other than the last push.
/// Refused while the server's content is busy, such as with a restore of that version.
pub async fn delete_version(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid, version)): Path<(String, String, i32)>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Owner).await?;
    world(&id, &wid)?;
    server_first(&state, &id).await?;
    let _busy = state.content.busy(&id)?;
    delete_share_version(&state.content, &id, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configs_are_listed_and_blocked_by_the_share_rules() {
        let directory = tempfile::tempdir().unwrap();
        let config = directory.path().join("config");
        std::fs::create_dir_all(config.join(".hidden")).unwrap();
        std::fs::write(config.join("a.toml"), "x = 1").unwrap();
        std::fs::write(config.join("b.jar"), "jar").unwrap();
        std::fs::write(config.join("c.json"), b"MZ\x90\x00\x03\x00\x00\x00")
            .unwrap();
        std::fs::write(config.join(".hidden/d.toml"), "x = 1").unwrap();
        let folder = Folder::open(directory.path()).unwrap();
        let shared = BTreeSet::from(["config/a.toml".to_owned()]);

        let listed = list_configs(&folder, &shared).unwrap();

        let files = listed
            .files
            .iter()
            .map(|file| (file.path.as_str(), file.blocked, file.shared))
            .collect::<Vec<_>>();
        assert_eq!(
            files,
            [
                ("config/a.toml", false, true),
                ("config/b.jar", true, false),
                ("config/c.json", true, false),
            ]
        );
    }
}
