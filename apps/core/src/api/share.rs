//! The share of a server-first server, a server that follows no instance, which this Core writes
//! with its own key (`docs/internals/plan.md`, "Shares, members, and players"): `POST|DELETE
//! /v1/servers/{id}/worlds/{wid}/content/share`, `GET .../content/share/diff`, and
//! `GET .../content/share/files`. Mirrors `Archon.Content.v1` (`archon/content/v1.ts`).
//!
//! A server that follows an instance gets 409 on all of them: it reads its instance's share,
//! and each share has one writer. What a version carries is `domain::server_share`, plus the
//! config files the push picks.

use std::{
    collections::{BTreeSet, HashMap},
    path::Path as FsPath,
    sync::Arc,
};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use modrinth_content_management::{
    CommonExternalFilePolicy, ContentSetConfiguration, ContentType,
    LoaderReference,
};
use serde::{Deserialize, Serialize};
use sharing_client::{
    backend::{Backend, BackendError},
    diff::{
        CarriedFile, DiffError, DiffOptions, Disabled, ShareContent,
        SharedInstanceUpdateDiff, VersionName, has_changes, share_diff,
    },
    history,
    install::ModpackFile,
    push,
    rules::{
        CONFIG_FILE_TYPE, FolderEntry, SCAN_HEAD_SIZE, ShareFolder, ShareScope,
        list_shareable,
    },
    versions::{ManifestFile, VersionContent},
};
use ts_rs::TS;

use super::{
    AppState,
    content::{server_first, world},
    error::{ApiError, ErrorCode},
};
use crate::{
    access::{Caller, Level},
    installs::ModLoader,
    servers::ServerError,
    state::servers::{
        adapters::filesystem::content_files,
        adapters::network::modrinth::ModrinthError,
        adapters::{
            filesystem::folder::{self, Folder},
            history::History,
            sqlite::{content_rows, server_rows},
        },
        commands::drop_own_share::drop_own_share,
        domain::server_share::{content_rows, pack_file, removed},
        modrinth_pack,
    },
};

/// Config files `GET .../content/share/files` lists at most.
const MAX_LISTED: usize = 2000;
/// Where a server's shared files come from.
const SCOPE: ShareScope = ShareScope::Server;

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

/// What a push of the server would send now.
struct Snapshot {
    /// The server's name, which the share takes.
    name: String,
    folder: Arc<Folder>,
    content: VersionContent,
    /// The files the push carries, as the server has them.
    carried: Vec<push::ContentRow>,
    /// The files its modpack placed, by which the share diff splits a version on that modpack.
    pack_files: Vec<ModpackFile>,
}

async fn snapshot(state: &AppState, id: &str) -> Result<Snapshot, ApiError> {
    let server = server_rows::get(&state.db, id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.to_owned()))?;
    let loader = server.loader.as_deref().and_then(ModLoader::parse);
    let (Some(loader), Some(game_version)) = (loader, server.game_version)
    else {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "Install a loader on this server before sharing it.",
        ));
    };
    // A server without a client loader is shared as vanilla, which players install as Fabric so
    // they can add their own mods.
    let (shared_loader, loader_version) = match loader {
        ModLoader::Fabric => ("fabric", server.loader_version),
        ModLoader::Quilt => ("quilt", server.loader_version),
        ModLoader::Forge => ("forge", server.loader_version),
        ModLoader::NeoForge => ("neoforge", server.loader_version),
        ModLoader::Vanilla | ModLoader::Paper => ("vanilla", None),
    };
    let folder = Arc::new(Folder::open(&state.servers.paths().server(id))?);
    let listed = content_files::scan(&folder, Some(loader));
    let mut rows = HashMap::new();
    let mut entry_sides = HashMap::new();
    for (row, sides) in
        content_rows::get_tracked_files_with_sides(&state.db, id).await?
    {
        entry_sides.insert(row.relative_path.clone(), sides);
        rows.insert(row.relative_path.clone(), row);
    }
    // A pack uploaded as a file is not on Modrinth, so its files are shared one by one.
    let modpack_id =
        content_rows::get_pack(&state.db, id)
            .await?
            .and_then(|pack| {
                modrinth_pack(&pack.link)
                    .map(|(_, version_id)| version_id.to_owned())
            });
    let pack_files = rows
        .values()
        .filter(|row| row.by_pack())
        .map(pack_file)
        .collect::<Vec<_>>();
    let read = push::read_content(
        content_rows(&listed, &rows, &entry_sides, modpack_id.is_some()),
        if modpack_id.is_some() {
            &pack_files
        } else {
            &[]
        },
    );
    let removed_files = if modpack_id.is_some() {
        let level = folder.level_name();
        let dirs = [ContentType::Mod, ContentType::DataPack]
            .map(|kind| content_files::kind_folder(kind, &level))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        removed(&listed, &rows, &entry_sides, &dirs)
    } else {
        Vec::new()
    };
    // The files are staged when a push sends them; the builder decides the content.
    let (content, _) = push::build_version(
        SCOPE,
        push::VersionInput {
            modrinth_ids: read.modrinth_ids,
            modpack_id,
            removed_files,
            game_version,
            loader: shared_loader.to_owned(),
            loader_version,
            files: Vec::new(),
        },
    )
    .map_err(|error| ApiError::new(ErrorCode::Internal, error.to_string()))?;
    Ok(Snapshot {
        name: server.name,
        folder,
        content,
        carried: read.carried,
        pack_files,
    })
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
    let snapshot = snapshot(&state, &id).await?;
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
        record_push(&state, &snapshot, &share_id, &pushed).await;
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
    let snapshot = snapshot(&state, &id).await?;
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

/// A push compares every file it sends by SHA-1, which both sides know.
fn push_options() -> DiffOptions {
    DiffOptions {
        common_external_files: CommonExternalFilePolicy::AssumeUnchanged,
        disabled: Disabled::default(),
    }
}

/// Whether the server has changes its last push does not have, so a push would change something
/// (upstream's `shared_instance_needs_update`). Compared with the newest entry of the share's
/// history on this Core, so it asks no backend. False for a server that is not shared, and when
/// this Core has no push of it.
pub async fn needs_update(state: &AppState, id: &str) -> bool {
    let check = async {
        let Some(share_id) = server_rows::share_id(&state.db, id).await? else {
            return Ok(false);
        };
        let Some(last) = history::latest(&history_store(state), &share_id)
            .await
            .map_err(crate::state::servers::Error::from)?
        else {
            return Ok(false);
        };
        let snapshot = snapshot(state, id).await?;
        let files = last.manifest().cloned().collect::<Vec<_>>();
        let (before, after) =
            share_contents(&snapshot, &last.content, &files).await?;
        Ok::<_, ApiError>(has_changes(
            &before,
            &after,
            push_options().common_external_files,
        ))
    };
    check.await.unwrap_or_else(|error| {
        tracing::debug!(server = %id, error = %error.description, "could not compare the server to its last push");
        false
    })
}

/// The share history this Core keeps.
fn history_store(state: &AppState) -> History<'_> {
    History {
        store: &state.content.content_store,
        db: &state.db,
    }
}

/// Records `pushed` in the share's history, naming what it changed over the version before it.
/// The push already succeeded, so a failure is logged and the version is only missing from the
/// history.
async fn record_push(
    state: &AppState,
    snapshot: &Snapshot,
    share_id: &str,
    pushed: &push::Pushed,
) {
    let diffs = async {
        let before = match &pushed.previous {
            Some(previous) => version_content(
                snapshot,
                &previous.content,
                &previous.external_files,
            )?,
            None => ShareContent::empty(configuration(&snapshot.content)),
        };
        let after = version_content(
            snapshot,
            &snapshot.content,
            &pushed.external_files,
        )?;
        share_diff(&before, &after, &push_options(), &Names(&state.content))
            .await
            .map_err(|error| {
                ApiError::new(ErrorCode::Internal, error.to_string())
            })
    };
    let diffs = diffs.await.unwrap_or_else(|error| {
        tracing::warn!(share = %share_id, error = %error.description, "could not name what a push changed");
        Vec::new()
    });
    if let Err(error) = history::record(
        &history_store(state),
        share_id,
        &snapshot.content,
        pushed,
        diffs,
        Some(history::DEFAULT_KEPT),
    )
    .await
    {
        tracing::warn!(share = %share_id, %error, "could not record a push in the share's history");
    }
}

fn configuration(content: &VersionContent) -> ContentSetConfiguration {
    ContentSetConfiguration {
        modpack_version_id: content.modpack_id.clone(),
        game_version: content.game_version.clone(),
        loader: LoaderReference {
            name: content.loader.clone(),
            version: Some(content.loader_version.clone()),
        },
    }
}

/// A version, `content` with `files`, as the share diff reads it. A version on the server's own
/// modpack is split by the files that modpack placed.
fn version_content(
    snapshot: &Snapshot,
    content: &VersionContent,
    files: &[ManifestFile],
) -> Result<ShareContent, ApiError> {
    let pack = if content.modpack_id.is_some()
        && content.modpack_id == snapshot.content.modpack_id
    {
        snapshot.pack_files.clone()
    } else {
        Vec::new()
    };
    ShareContent::of(
        content,
        files.iter().map(CarriedFile::from),
        pack,
        configuration(content),
    )
    .map_err(|error| ApiError::new(ErrorCode::Internal, error.to_string()))
}

/// A version (`content` with `files`), and what a push of the server sends now, as the share
/// diff reads them.
async fn share_contents(
    snapshot: &Snapshot,
    content: &VersionContent,
    files: &[ManifestFile],
) -> Result<(ShareContent, ShareContent), ApiError> {
    let before = version_content(snapshot, content, files)?;
    // A push sends the server's addons as they are on disk, and every config still shared as
    // the server has it, so one it no longer has leaves the share.
    let local = snapshot
        .carried
        .iter()
        .map(|row| {
            (
                row.file_type.clone(),
                row.file_name.clone(),
                row.path.clone(),
            )
        })
        .chain(
            files
                .iter()
                .filter(|file| file.file_type == CONFIG_FILE_TYPE)
                .map(|file| {
                    (
                        CONFIG_FILE_TYPE.to_owned(),
                        file.file_name.clone(),
                        file.file_name.clone(),
                    )
                }),
        )
        .collect::<Vec<_>>();
    let folder = Arc::clone(&snapshot.folder);
    let local = tokio::task::spawn_blocking(move || {
        local
            .into_iter()
            .map(|(file_type, file_name, on_disk)| {
                let sha1 = folder.sha1(&on_disk).ok().flatten();
                (file_type, file_name, sha1)
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(std::io::Error::other)?;
    let after = ShareContent::of(
        &snapshot.content,
        local
            .iter()
            .map(|(file_type, file_name, sha1)| CarriedFile {
                file_type,
                file_name,
                sha1: sha1.as_deref(),
            }),
        snapshot.pack_files.clone(),
        configuration(&snapshot.content),
    )
    .map_err(|error| ApiError::new(ErrorCode::Internal, error.to_string()))?;
    Ok((before, after))
}

/// Modrinth's names through Core's own client.
struct Names<'a>(&'a crate::state::servers::Content);

impl sharing_client::diff::Names for Names<'_> {
    type Error = ModrinthError;

    async fn versions(
        &self,
        ids: &[String],
    ) -> Result<Vec<VersionName>, ModrinthError> {
        Ok(self
            .0
            .modrinth
            .versions_by_id(ids)
            .await?
            .into_iter()
            .map(|version| VersionName {
                id: version.id,
                project_id: version.project_id,
                name: version.version_number.clone(),
                version_number: version.version_number,
            })
            .collect())
    }

    async fn projects(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, String>, ModrinthError> {
        Ok(self
            .0
            .modrinth
            .projects(ids)
            .await?
            .into_iter()
            .map(|project| (project.id, project.title))
            .collect())
    }
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
