//! The share of a server-first server, a server that follows no instance, which this Core writes
//! with its own key (`docs/internals/plan.md`, "Shares, members, and players"): `POST|DELETE
//! /v1/servers/{id}/worlds/{wid}/content/share`, `GET .../content/share/diff`, and
//! `GET .../content/share/files`. Mirrors `Archon.Content.v1` (`archon/content/v1.ts`).
//!
//! A server that follows an instance gets 409 on all of them: it reads its instance's share,
//! and each share has one writer. What a version carries is `content::server_share`, plus the
//! config files the push picks.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path as FsPath,
    sync::Arc,
};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sharing_client::{push, store::StoreClient};

use super::{
    AppState,
    content::{server_first, world},
    error::{ApiError, ErrorCode},
};
use crate::{
    access::{Caller, Level},
    content::{
        addons,
        folder::{self, Folder},
        records::{self, ContentFile, SideToggle},
        server_share::{Carried, carried},
    },
    installs::ModLoader,
    servers::{ServerError, records as servers_records},
};

/// Config files `GET .../content/share/files` lists at most.
const MAX_LISTED: usize = 2000;
/// Extensions a shared config file may have, as `SHAREABLE_CONFIG_EXTENSIONS` in
/// `packages/ui/src/composables/server-sharing/config-files.ts`.
const CONFIG_EXTENSIONS: &[&str] = &[
    "json",
    "json5",
    "jsonc",
    "yml",
    "yaml",
    "css",
    "toml",
    "txt",
    "ini",
    "cfg",
    "conf",
    "properties",
    "xml",
    "nbt",
];

/// Body of POST `.../content/share`. Mirrors the body of `ArchonContentV1Module.share`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ShareRequest {
    /// Files under `config/` to send with this version. Configs sent earlier stay shared.
    #[serde(default)]
    pub config_paths: Vec<String>,
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
    pub blocked: bool,
}

/// Mirrors Archon.Content.v1.ShareableFilesResponse.
#[derive(Debug, Clone, Serialize)]
pub struct ShareableFilesResponse {
    pub files: Vec<ShareableFile>,
    pub truncated: bool,
}

/// Mirrors Archon.Content.v1.SharedContentChange.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change<T> {
    Added { after: T },
    Removed { before: T },
    Updated { before: T, after: T },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChange {
    Added,
    Removed,
    Updated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoaderName {
    pub name: String,
    pub version: Option<String>,
}

/// Mirrors Archon.Content.v1.SharedContentDiffEntry. Core never shares a modpack, so it has no
/// `modpack` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DiffEntry {
    Project {
        project_id: String,
        change: Change<String>,
    },
    ExternalFile {
        file_type: String,
        file_name: String,
        kind: FileChange,
    },
    GameVersion {
        change: Change<String>,
    },
    Loader {
        change: Change<LoaderName>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectName {
    pub name: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionName {
    pub project_id: String,
    pub name: String,
    pub version_number: Option<String>,
}

/// Response of GET `.../content/share/diff`: what a push would change from the latest version.
/// Mirrors Archon.Content.v1.SharedInstancePublishDiff.
#[derive(Debug, Clone, Serialize)]
pub struct SharedInstancePublishDiff {
    pub shared_instance_id: String,
    pub latest_version: i32,
    pub local_updated_at: DateTime<Utc>,
    pub has_changes: bool,
    /// A push changes nothing the server runs.
    pub requires_restart: bool,
    pub diffs: Vec<DiffEntry>,
    pub projects: HashMap<String, ProjectName>,
    pub versions: HashMap<String, VersionName>,
}

/// A version's upstream content fields. Core shares no modpack: every addon is listed itself.
#[derive(Serialize)]
struct VersionContent {
    modrinth_ids: Vec<String>,
    modpack_id: Option<String>,
    game_version: String,
    loader: &'static str,
    loader_version: String,
}

/// What a push of the server would send now.
struct Snapshot {
    /// The server's name, which the share takes.
    name: String,
    folder: Arc<Folder>,
    content: VersionContent,
    carried: Vec<Carried>,
}

/// Whether `path` is a file under `config/` a share may carry, as `toShareableConfigPath` in
/// `config-files.ts` decides.
fn shareable_config(path: &str) -> bool {
    let mut parts = path.split('/');
    if parts.next() != Some("config") {
        return false;
    }
    let names = parts.collect::<Vec<_>>();
    let Some(last) = names.last() else {
        return false;
    };
    names.iter().all(|name| {
        !name.is_empty()
            && !name.starts_with('.')
            && !name.contains(['\\', ':'])
            && !name.ends_with(['.', ' '])
    }) && last.rsplit_once('.').is_some_and(|(_, extension)| {
        CONFIG_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    })
}

async fn snapshot(state: &AppState, id: &str) -> Result<Snapshot, ApiError> {
    let server = servers_records::get(&state.db, id)
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
    let listed = addons::scan(&folder, Some(loader));
    let rows: HashMap<String, ContentFile> = records::files(&state.db, id)
        .await?
        .into_iter()
        .map(|row| (row.path.clone(), row))
        .collect();
    let toggles: HashMap<String, SideToggle> = records::toggles(&state.db, id)
        .await?
        .into_iter()
        .map(|toggle| (toggle.path.clone(), toggle))
        .collect();
    let carried = carried(&listed, &rows, &toggles);
    let modrinth_ids = carried
        .iter()
        .filter_map(|item| match item {
            Carried::Modrinth { version_id, .. } => Some(version_id.clone()),
            Carried::File { .. } => None,
        })
        .collect();
    Ok(Snapshot {
        name: server.name,
        folder,
        content: VersionContent {
            modrinth_ids,
            modpack_id: None,
            game_version,
            loader: shared_loader,
            loader_version: loader_version.unwrap_or_default(),
        },
        carried,
    })
}

/// The backend's share routes, reached with this Core's key.
async fn backend(state: &AppState) -> Result<push::Backend, ApiError> {
    let key: Option<String> =
        sqlx::query_scalar("SELECT backend_key FROM core WHERE id = 1")
            .fetch_optional(&state.db)
            .await?
            .flatten();
    match (state.storage.backend_url(), key) {
        (Some(url), Some(key)) => {
            Ok(push::Backend::new(state.content.http.clone(), url, key))
        }
        _ => Err(ApiError::new(
            ErrorCode::Conflict,
            "Pair this Core with an account to share its servers.",
        )),
    }
}

impl From<push::PushError> for ApiError {
    fn from(error: push::PushError) -> Self {
        use push::PushError;
        let code = match &error {
            PushError::Executable(_)
            | PushError::TooLarge
            | PushError::TooManyFiles(_)
            | PushError::Refused(_) => ErrorCode::InvalidRequest,
            PushError::NoStorage
            | PushError::PushInFlight
            | PushError::FileChanged(_)
            | PushError::Unavailable(_) => ErrorCode::Conflict,
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
            .map_err(crate::content::Error::from)?;
        if !shareable_config(&path) {
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
    let existing = match records::share_id(&state.db, &id).await? {
        Some(share_id) => {
            match backend.rename_share(&share_id, &snapshot.name).await {
                Err(push::PushError::Backend {
                    status: 403 | 404, ..
                }) => None,
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
            records::set_share_id(&state.db, &id, Some(&share_id)).await?;
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
        .filter_map(|item| match item {
            Carried::File {
                file_name,
                file_type,
                on_disk,
            } => Some((
                file_name.clone(),
                file_type.to_string(),
                on_disk.clone(),
            )),
            Carried::Modrinth { .. } => None,
        })
        .chain(configs.into_iter().map(|path| {
            (path.clone(), push::CONFIG_FILE_TYPE.to_owned(), path)
        }))
        .collect::<Vec<_>>();
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
        let store =
            StoreClient::new(concat!("copal/", env!("CARGO_PKG_VERSION")));
        push::push(
            &backend,
            &store,
            &share_id,
            &snapshot.content,
            None,
            &files,
            &staging.join("pool"),
            &mut |_| {},
        )
        .await
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

/// `DELETE .../content/share`: delete the server's share. Everyone who had it stops getting
/// updates.
pub async fn unshare(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Owner).await?;
    world(&id, &wid)?;
    server_first(&state, &id).await?;
    if records::share_id(&state.db, &id).await?.is_none() {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            "This server is not shared.",
        ));
    }
    drop_share(&state, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Delete the server's own share, if it has one, so its players stop getting it. Called when the
/// share is deleted, when the server is, and before the server follows an instance, since a
/// server never has two shares.
pub async fn drop_share(state: &AppState, id: &str) -> Result<(), ApiError> {
    let Some(share_id) = records::share_id(&state.db, id).await? else {
        return Ok(());
    };
    // An unpaired Core, or a 403 to a Core paired again, means the Core was removed from the
    // account that had the share, and the backend deleted its shares then.
    if let Ok(backend) = backend(state).await {
        match backend.delete_share(&share_id).await {
            Ok(()) | Err(push::PushError::Backend { status: 403, .. }) => {}
            Err(error) => return Err(error.into()),
        }
    }
    records::set_share_id(&state.db, id, None).await?;
    Ok(())
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
    let response = tokio::task::spawn_blocking(move || list_configs(&folder))
        .await
        .map_err(std::io::Error::other)??;
    Ok(Json(response))
}

/// Every file under `config/`, without following links. Folders that cannot be opened, such as
/// a link, are skipped.
fn list_configs(folder: &Folder) -> std::io::Result<ShareableFilesResponse> {
    let mut files = Vec::new();
    let mut folders = vec!["config".to_owned()];
    while let Some(path) = folders.pop() {
        let Ok(dir) = folder.open_dir(Some(&path)) else {
            continue;
        };
        for entry in dir.entries()?.filter_map(Result::ok) {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let child = format!("{path}/{name}");
            if kind.is_dir() {
                folders.push(child);
            } else if kind.is_file() {
                if files.len() == MAX_LISTED {
                    return Ok(ShareableFilesResponse {
                        files,
                        truncated: true,
                    });
                }
                files.push(ShareableFile {
                    blocked: !shareable_config(&child),
                    size: entry.metadata().map_or(0, |meta| meta.len()),
                    path: child,
                });
            }
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(ShareableFilesResponse {
        files,
        truncated: false,
    })
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
    let share_id = records::share_id(&state.db, &id)
        .await?
        .ok_or_else(not_shared)?;
    let latest = backend(&state)
        .await?
        .latest_ready(&share_id)
        .await?
        .ok_or_else(not_shared)?;
    let snapshot = snapshot(&state, &id).await?;

    // Projects, by the version each side has.
    let after = snapshot
        .carried
        .iter()
        .filter_map(|item| match item {
            Carried::Modrinth {
                version_id,
                project_id,
            } => Some((project_id.clone(), version_id.clone())),
            Carried::File { .. } => None,
        })
        .collect::<BTreeMap<_, _>>();
    let version_ids = latest
        .modrinth_ids
        .iter()
        .chain(after.values())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let modrinth = &state.content.modrinth;
    let versions = modrinth
        .versions_by_id(&version_ids)
        .await
        .unwrap_or_default();
    let before = latest
        .modrinth_ids
        .iter()
        .map(|version_id| {
            let project = versions
                .iter()
                .find(|version| &version.id == version_id)
                .map_or(version_id, |version| &version.project_id);
            (project.clone(), version_id.clone())
        })
        .collect::<BTreeMap<_, _>>();
    let mut diffs = project_diffs(&before, &after);

    // Files: content files by SHA-1, and configs only where the server's copy changed, since a
    // config not sent again stays shared.
    let shared = latest
        .external_files
        .iter()
        .map(|file| ((file.file_type.clone(), file.file_name.clone()), file))
        .collect::<BTreeMap<_, _>>();
    let folder = Arc::clone(&snapshot.folder);
    let local = snapshot
        .carried
        .iter()
        .filter_map(|item| match item {
            Carried::File {
                file_name,
                file_type,
                on_disk,
            } => Some((
                file_type.to_string(),
                file_name.clone(),
                on_disk.clone(),
            )),
            Carried::Modrinth { .. } => None,
        })
        .chain(
            shared
                .keys()
                .filter(|(file_type, _)| file_type == push::CONFIG_FILE_TYPE)
                .map(|(file_type, file_name)| {
                    (file_type.clone(), file_name.clone(), file_name.clone())
                }),
        )
        .collect::<Vec<_>>();
    let local = tokio::task::spawn_blocking(move || {
        local
            .into_iter()
            .filter_map(|(file_type, file_name, on_disk)| {
                let sha1 = folder.sha1(&on_disk).ok()??;
                Some(((file_type, file_name), sha1))
            })
            .collect::<BTreeMap<_, _>>()
    })
    .await
    .map_err(std::io::Error::other)?;
    for ((file_type, file_name), sha1) in &local {
        let kind = match shared.get(&(file_type.clone(), file_name.clone())) {
            None => FileChange::Added,
            Some(file)
                if !file.sha1.as_deref().is_some_and(|shared| {
                    shared.eq_ignore_ascii_case(sha1)
                }) =>
            {
                FileChange::Updated
            }
            Some(_) => continue,
        };
        diffs.push(DiffEntry::ExternalFile {
            file_type: file_type.clone(),
            file_name: file_name.clone(),
            kind,
        });
    }
    for (file_type, file_name) in shared.keys() {
        if ![push::CONFIG_FILE_TYPE, push::CONFIG_BUNDLE_FILE_TYPE]
            .contains(&file_type.as_str())
            && !local.contains_key(&(file_type.clone(), file_name.clone()))
        {
            diffs.push(DiffEntry::ExternalFile {
                file_type: file_type.clone(),
                file_name: file_name.clone(),
                kind: FileChange::Removed,
            });
        }
    }

    let content = &snapshot.content;
    if latest.game_version != content.game_version {
        diffs.push(DiffEntry::GameVersion {
            change: Change::Updated {
                before: latest.game_version.clone(),
                after: content.game_version.clone(),
            },
        });
    }
    let loader = |name: &str, version: &str| LoaderName {
        name: name.to_owned(),
        version: Some(version.to_owned()).filter(|version| !version.is_empty()),
    };
    let (shared_loader, local_loader) = (
        loader(&latest.loader, &latest.loader_version),
        loader(content.loader, &content.loader_version),
    );
    if shared_loader != local_loader {
        diffs.push(DiffEntry::Loader {
            change: Change::Updated {
                before: shared_loader,
                after: local_loader,
            },
        });
    }

    let project_ids = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let projects = modrinth
        .projects(&project_ids)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|project| {
            (
                project.id,
                ProjectName {
                    name: project.title,
                    icon_url: project.icon_url,
                },
            )
        })
        .collect();
    let versions = versions
        .into_iter()
        .map(|version| {
            (
                version.id,
                VersionName {
                    project_id: version.project_id,
                    name: version.version_number.clone(),
                    version_number: Some(version.version_number),
                },
            )
        })
        .collect();
    Ok(Json(SharedInstancePublishDiff {
        shared_instance_id: share_id,
        latest_version: latest.version,
        local_updated_at: Utc::now(),
        has_changes: !diffs.is_empty(),
        requires_restart: false,
        diffs,
        projects,
        versions,
    }))
}

/// Each project added, removed, or on another version, from the version each side has of it.
fn project_diffs(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<DiffEntry> {
    let projects = before.keys().chain(after.keys()).collect::<BTreeSet<_>>();
    projects
        .into_iter()
        .filter_map(|project| {
            let change = match (before.get(project), after.get(project)) {
                (None, Some(after)) => Change::Added {
                    after: after.clone(),
                },
                (Some(before), None) => Change::Removed {
                    before: before.clone(),
                },
                (Some(before), Some(after)) if before != after => {
                    Change::Updated {
                        before: before.clone(),
                        after: after.clone(),
                    }
                }
                _ => return None,
            };
            Some(DiffEntry::Project {
                project_id: project.clone(),
                change,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_paths_are_shareable_as_the_app_decides() {
        assert!(shareable_config("config/a.toml"));
        assert!(shareable_config("config/mod/b.JSON"));
        assert!(!shareable_config("config"));
        assert!(!shareable_config("mods/a.toml"));
        assert!(!shareable_config("config/a.jar"));
        assert!(!shareable_config("config/.hidden/a.toml"));
        assert!(!shareable_config("config/a.toml."));
    }
}
