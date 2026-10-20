//! Installing on a held server: a loader, a pack, a pack update, repair, and the owner's
//! decisions on files a pack update left alone.
//!
//! A pack install downloads every file it needs into `runs/<server>/staging/` before it changes
//! anything, so a download that fails leaves the server as it was. Then it applies the plan
//! (`pack::plan`) file by file, recording each as it goes; a failure there is recorded on the
//! pack and repair finishes the job. Updates copy the world first.

use std::{
    collections::{BTreeMap, HashMap},
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use futures::{StreamExt, TryStreamExt};
use tracing::{info, warn};

use super::{
    Content, Error, Progress,
    folder::Folder,
    modrinth,
    pack::{self, Index, Platform, Source, Step, Wanted},
    properties,
    records::{self, ContentFile, Origin, Pack, Pending},
};
use crate::{
    api::{
        events::{ContentPlatform, InstallProgressKey},
        properties::PropertiesFields,
    },
    backups,
    installs::{
        self, ModLoader,
        daedalus::SharedDirs,
        download::{self, FileDownload, Hash},
    },
    servers::{Hold, records as servers_records},
};

/// Pack files downloaded at once.
const CONCURRENCY: usize = 6;
/// The largest single file a pack may list.
const MAX_FILE: u64 = 2 * 1024 * 1024 * 1024;
/// The largest `.mrpack`.
pub const MAX_PACK: u64 = 4 * 1024 * 1024 * 1024;

/// Which pack to install.
#[derive(Debug, Clone)]
pub enum PackSource {
    Modrinth {
        project_id: String,
        version_id: String,
    },
    /// An `.mrpack` the App uploaded, already in the shared store. `instance` is the App
    /// instance it came from (id, name).
    Stored {
        sha1: String,
        instance: Option<(String, String)>,
    },
    /// The pack the server follows now: repair.
    Current,
}

fn platform_name(loader: ModLoader) -> ContentPlatform {
    match loader {
        ModLoader::Vanilla => ContentPlatform::Vanilla,
        ModLoader::Paper => ContentPlatform::Paper,
        ModLoader::Fabric => ContentPlatform::Fabric,
        ModLoader::Forge => ContentPlatform::Forge,
        ModLoader::NeoForge => ContentPlatform::Neoforge,
        ModLoader::Quilt => ContentPlatform::Quilt,
    }
}

/// Where a pack install stages its downloads. Uploads and addons stage beside it.
fn pack_staging(content: &Content, server_id: &str) -> PathBuf {
    content.paths.staging(server_id).join("pack")
}

/// Install `loader` on the held server. A server with something installed is copied first,
/// unless this is a repair (`verify`), which changes no versions.
pub async fn loader(
    content: &Content,
    hold: &Hold,
    progress: &Progress,
    loader: ModLoader,
    game_version: &str,
    requested: Option<&str>,
    verify: bool,
) -> Result<(), Error> {
    let id = hold.id();
    let server = servers_records::get(&content.db, id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("There is no server {id}.")))?;
    if server.game_version.is_some() && !verify {
        backups::pre_update(&content.paths, id).await?;
    }
    let key = InstallProgressKey::Platform {
        platform: platform_name(loader),
        platform_version: requested.unwrap_or_default().to_owned(),
        game_version: game_version.to_owned(),
    };
    let installed = installs::install(
        &content.servers,
        hold,
        &content.http,
        &server,
        loader,
        game_version,
        requested,
        verify,
        &|share| progress.set("platform", &key, share),
    )
    .await?;
    servers_records::set_installed(
        &content.db,
        id,
        loader.as_str(),
        game_version,
        installed.as_deref(),
    )
    .await?;
    info!(server = %id, %loader, game_version, version = ?installed, "installed");
    Ok(())
}

/// Install or update the held server's pack, or repair it (`PackSource::Current`). Settings in
/// `properties` are written once the pack is in place.
pub async fn pack(
    content: &Content,
    hold: &Hold,
    progress: &Progress,
    source: PackSource,
    properties: Option<PropertiesFields>,
) -> Result<(), Error> {
    let id = hold.id();
    let current = records::pack(&content.db, id).await?;
    let result = install_pack(
        content,
        hold,
        progress,
        source,
        current.as_ref(),
        properties,
    )
    .await;
    if let Err(error) = &result {
        records::set_pack_error(&content.db, id, Some(&format!("{error:#}")))
            .await?;
    }
    let _ = tokio::fs::remove_dir_all(pack_staging(content, id)).await;
    result
}

async fn install_pack(
    content: &Content,
    hold: &Hold,
    progress: &Progress,
    source: PackSource,
    current: Option<&Pack>,
    properties: Option<PropertiesFields>,
) -> Result<(), Error> {
    let id = hold.id();
    let dirs = SharedDirs::new(content.paths.root());
    let repair = matches!(source, PackSource::Current);
    let (sha1, project, instance) = match source {
        PackSource::Modrinth {
            project_id,
            version_id,
        } => {
            let key = InstallProgressKey::ModrinthModpack {
                project_id: project_id.clone(),
                version_id: version_id.clone(),
            };
            let sha1 =
                fetch_modrinth_pack(content, &dirs, &version_id, &|share| {
                    progress.set("pack", &key, share);
                })
                .await?;
            (sha1, Some((project_id, version_id)), None)
        }
        PackSource::Stored { sha1, instance } => (sha1, None, instance),
        PackSource::Current => {
            let pack = current.ok_or_else(|| {
                Error::Invalid("This server follows no pack to repair.".into())
            })?;
            let project = pack.project_id.clone().zip(pack.version_id.clone());
            let instance =
                pack.instance_id.clone().zip(pack.instance_name.clone());
            (pack.sha1.clone(), project, instance)
        }
    };
    let archive_path = dirs.pack(&sha1);
    let (index, wanted) = read_pack(&archive_path).await?;
    let platform = pack::platform(&index)?;

    let server = servers_records::get(&content.db, id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("There is no server {id}.")))?;
    let same_platform = server.loader.as_deref()
        == Some(platform.loader.as_str())
        && server.game_version.as_deref()
            == Some(platform.game_version.as_str())
        && (platform.loader_version.is_none()
            || server.loader_version == platform.loader_version);

    // Every download first: a failure here changes nothing.
    let folder = Arc::new(Folder::open(&content.paths.server(id))?);
    let rows = records::files(&content.db, id).await?;
    let steps = {
        let folder = Arc::clone(&folder);
        let wanted = wanted.clone();
        tokio::task::spawn_blocking(move || {
            let level = folder.level_name();
            let sha1 = |path: &str| folder.sha1(path).ok().flatten();
            pack::plan(
                &rows,
                &wanted,
                &pack::Disk {
                    sha1: &sha1,
                    level_name: &level,
                },
                repair,
            )
        })
        .await
        .map_err(std::io::Error::other)?
    };
    let staged = stage_downloads(content, id, &index, &steps, progress).await?;

    if server.game_version.is_some() && !repair {
        backups::pre_update(&content.paths, id).await?;
    }
    if !same_platform || repair {
        install_platform(content, hold, progress, &platform, repair).await?;
    }

    let summary = index.summary.clone();
    records::set_pack(
        &content.db,
        id,
        &Pack {
            project_id: project.as_ref().map(|(project, _)| project.clone()),
            version_id: project.as_ref().map(|(_, version)| version.clone()),
            name: index.name.clone(),
            version_number: index.version_id.clone(),
            summary,
            sha1: sha1.clone(),
            instance_id: instance.as_ref().map(|(id, _)| id.clone()),
            instance_name: instance.as_ref().map(|(_, name)| name.clone()),
            error: None,
        },
    )
    .await?;
    apply(content, id, &folder, &archive_path, &index, steps, &staged).await?;
    if let Some(properties) = properties {
        write_properties(&content.paths.server(id), &properties)?;
    }
    if let Some(old) = current.map(|pack| &pack.sha1)
        && *old != sha1
        && !records::pack_in_use(&content.db, old).await?
    {
        let _ = tokio::fs::remove_file(dirs.pack(old)).await;
    }
    identify(content, id).await;
    info!(server = %id, pack = %index.name, version = %index.version_id, "pack installed");
    Ok(())
}

/// Install the loader a pack runs on.
async fn install_platform(
    content: &Content,
    hold: &Hold,
    progress: &Progress,
    platform: &Platform,
    verify: bool,
) -> Result<(), Error> {
    let server = servers_records::get(&content.db, hold.id())
        .await?
        .ok_or_else(|| Error::NotFound("The server is gone.".into()))?;
    let key = InstallProgressKey::Platform {
        platform: platform_name(platform.loader),
        platform_version: platform.loader_version.clone().unwrap_or_default(),
        game_version: platform.game_version.clone(),
    };
    let installed = installs::install(
        &content.servers,
        hold,
        &content.http,
        &server,
        platform.loader,
        &platform.game_version,
        platform.loader_version.as_deref(),
        verify,
        &|share| progress.set("platform", &key, share),
    )
    .await?;
    servers_records::set_installed(
        &content.db,
        hold.id(),
        platform.loader.as_str(),
        &platform.game_version,
        installed.as_deref(),
    )
    .await?;
    Ok(())
}

/// Download a Modrinth pack version's `.mrpack` into the shared store and return its SHA-1.
async fn fetch_modrinth_pack(
    content: &Content,
    dirs: &SharedDirs,
    version_id: &str,
    progress: &(dyn Fn(f64) + Sync),
) -> Result<String, Error> {
    let version = content.modrinth.version(version_id).await?;
    let file = version.primary().ok_or_else(|| {
        Error::Invalid(format!("Modrinth version {version_id} has no file."))
    })?;
    let url = allowed_url(&file.url)?;
    let sha1 = file.hashes.sha1.to_ascii_lowercase();
    let path = dirs.pack(&sha1);
    if !download::has_hash(&path, Hash::Sha1(&sha1)).await {
        let received = AtomicU64::new(0);
        let total = file.size.max(1);
        download::file(
            &content.downloads,
            url.as_str(),
            &path,
            &FileDownload {
                hash: Hash::Sha1(&sha1),
                limit: Some(MAX_PACK),
                progress: &|update| match update {
                    download::Progress::Bytes { bytes, .. } => {
                        let done = received.fetch_add(bytes, Ordering::Relaxed)
                            + bytes;
                        progress(done as f64 / total as f64);
                    }
                    download::Progress::Reset => {
                        received.store(0, Ordering::Relaxed)
                    }
                },
            },
        )
        .await?;
    }
    progress(1.0);
    Ok(sha1)
}

fn allowed_url(url: &str) -> Result<reqwest::Url, Error> {
    reqwest::Url::parse(url)
        .ok()
        .filter(|url| super::hosts::allowed(url, false))
        .ok_or_else(|| {
            Error::Invalid(format!(
                "{url} is not on a host packs may download from."
            ))
        })
}

/// A pack's index and the files it wants, read from the `.mrpack` at `path`.
pub async fn read_pack(
    path: &Path,
) -> Result<(Index, BTreeMap<String, Wanted>), Error> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(super::archive::ArchiveError::from)?;
        let index = pack::read_index(&mut archive)?;
        let wanted = pack::wanted(&index, &mut archive)?;
        Ok((index, wanted))
    })
    .await
    .map_err(std::io::Error::other)?
}

/// Download every file the plan places from the index, checked, into staging. Returns where each
/// landed, by index file.
async fn stage_downloads(
    content: &Content,
    server_id: &str,
    index: &Index,
    steps: &[Step],
    progress: &Progress,
) -> Result<HashMap<usize, PathBuf>, Error> {
    let staging = pack_staging(content, server_id);
    let _ = tokio::fs::remove_dir_all(&staging).await;
    tokio::fs::create_dir_all(&staging).await?;
    let needed: Vec<usize> = steps
        .iter()
        .filter_map(|step| match step {
            Step::Place {
                wanted:
                    Wanted {
                        source: Source::Download(n),
                        ..
                    },
                ..
            } => Some(*n),
            _ => None,
        })
        .collect();
    let total: u64 = needed
        .iter()
        .map(|n| index.files[*n].file_size)
        .sum::<u64>()
        .max(1);
    let received = AtomicU64::new(0);
    let key = InstallProgressKey::LocalModpack {
        filename: index.name.clone(),
    };
    let staged = futures::stream::iter(needed)
        .map(|n| {
            let staging = &staging;
            let received = &received;
            let key = &key;
            async move {
                let file = &index.files[n];
                let path = staging.join(n.to_string());
                fetch_index_file(content, file, &path, &|bytes| {
                    let done =
                        received.fetch_add(bytes, Ordering::Relaxed) + bytes;
                    progress.set("files", key, done as f64 / total as f64);
                })
                .await?;
                Ok::<_, Error>((n, path))
            }
        })
        .buffer_unordered(CONCURRENCY)
        .try_collect()
        .await?;
    progress.set("files", &key, 1.0);
    Ok(staged)
}

/// Download one file of a pack index from the first allowed URL that works.
async fn fetch_index_file(
    content: &Content,
    file: &pack::IndexFile,
    path: &Path,
    progress: &(dyn Fn(u64) + Sync),
) -> Result<(), Error> {
    let hash = match &file.hashes.sha512 {
        Some(sha512) => Hash::Sha512(sha512),
        None => Hash::Sha1(&file.hashes.sha1),
    };
    let mut last =
        Error::Invalid(format!("{} has no allowed download.", file.path));
    for url in &file.downloads {
        let Ok(url) = allowed_url(url) else { continue };
        let result = download::file(
            &content.downloads,
            url.as_str(),
            path,
            &FileDownload {
                hash,
                limit: Some(file.file_size.clamp(1, MAX_FILE)),
                progress: &|update| {
                    if let download::Progress::Bytes { bytes, .. } = update {
                        progress(bytes);
                    }
                },
            },
        )
        .await;
        match result {
            Ok(()) => return Ok(()),
            Err(error) => last = error.into(),
        }
    }
    Err(last)
}

/// Carry out a plan, recording every file as it is placed.
async fn apply(
    content: &Content,
    server_id: &str,
    folder: &Arc<Folder>,
    archive_path: &Path,
    index: &Index,
    steps: Vec<Step>,
    staged: &HashMap<usize, PathBuf>,
) -> Result<(), Error> {
    for step in steps {
        match step {
            Step::Place { path, wanted, row } => {
                place(folder, archive_path, &path, &wanted.source, staged)
                    .await?;
                if let Some(mut row) = row {
                    if let Source::Download(n) = wanted.source
                        && let Some((project, version)) = index.files[n]
                            .downloads
                            .iter()
                            .find_map(|url| modrinth::ids_from_cdn(url))
                    {
                        row.project_id = Some(project);
                        row.version_id = Some(version);
                    }
                    records::put(&content.db, server_id, &row).await?;
                }
            }
            Step::Record(row) => {
                records::put(&content.db, server_id, &row).await?
            }
            Step::Delete(path) => {
                let folder = Arc::clone(folder);
                let target = path.clone();
                tokio::task::spawn_blocking(move || folder.remove(&target))
                    .await
                    .map_err(std::io::Error::other)??;
                records::remove(&content.db, server_id, &path).await?;
            }
            Step::Forget(path) => {
                records::remove(&content.db, server_id, &path).await?
            }
        }
    }
    Ok(())
}

/// Write one file the pack wants: a staged download, or an entry of the pack's own archive.
async fn place(
    folder: &Arc<Folder>,
    archive_path: &Path,
    path: &str,
    source: &Source,
    staged: &HashMap<usize, PathBuf>,
) -> Result<(), Error> {
    let folder = Arc::clone(folder);
    let path = path.to_owned();
    let source = match source {
        Source::Download(n) => {
            Err(staged.get(n).cloned().ok_or_else(|| {
                Error::Invalid(format!("{path} was not downloaded."))
            })?)
        }
        Source::Override(entry) => Ok(*entry),
    };
    let archive_path = archive_path.to_owned();
    tokio::task::spawn_blocking(move || -> Result<(), Error> {
        match source {
            Err(staged) => {
                folder.write(&path, &mut std::fs::File::open(staged)?)?;
            }
            Ok(entry) => {
                let mut archive =
                    zip::ZipArchive::new(std::fs::File::open(archive_path)?)
                        .map_err(super::archive::ArchiveError::from)?;
                let mut entry = archive
                    .by_index(entry)
                    .map_err(super::archive::ArchiveError::from)?
                    .take(super::archive::MAX_UNPACKED);
                folder.write(&path, &mut entry)?;
            }
        }
        Ok(())
    })
    .await
    .map_err(std::io::Error::other)?
}

/// Write the settings an install came with into `server.properties`.
pub fn write_properties(
    dir: &Path,
    fields: &PropertiesFields,
) -> std::io::Result<()> {
    let mut changes: BTreeMap<String, Option<String>> =
        crate::api::properties::known_pairs(&fields.known)
            .into_iter()
            .map(|(key, value)| (key, Some(value)))
            .collect();
    for (key, value) in fields.custom.iter().flatten() {
        changes.insert(key.clone(), Some(value.clone()));
    }
    properties::patch(dir, &changes)
}

/// Fill in the Modrinth project of tracked files that lack one, by their hash. Best effort.
pub async fn identify(content: &Content, server_id: &str) {
    let Ok(rows) = records::files(&content.db, server_id).await else {
        return;
    };
    let unknown: Vec<&ContentFile> =
        rows.iter().filter(|row| row.project_id.is_none()).collect();
    if unknown.is_empty() {
        return;
    }
    let sha1s: Vec<String> =
        unknown.iter().map(|row| row.sha1.clone()).collect();
    match content.modrinth.by_hashes(&sha1s).await {
        Ok(found) => {
            for row in unknown {
                if let Some(version) = found.get(&row.sha1) {
                    let _ = records::set_project(
                        &content.db,
                        server_id,
                        &row.path,
                        &version.project_id,
                        &version.id,
                    )
                    .await;
                }
            }
        }
        Err(error) => {
            warn!(server = %server_id, %error, "could not look files up on Modrinth")
        }
    }
}

/// The owner's answer for a file a pack update left alone: keep their version, or take the
/// pack's (replace).
pub async fn decide(
    content: &Content,
    server_id: &str,
    path: &str,
    replace: bool,
) -> Result<(), Error> {
    let row = records::file(&content.db, server_id, path)
        .await?
        .filter(|row| row.pending.is_some())
        .ok_or_else(|| {
            Error::NotFound(format!("{path} waits for no decision."))
        })?;
    let folder = Arc::new(Folder::open(&content.paths.server(server_id))?);
    match (row.pending, replace) {
        (Some(Pending::Removed), false) => {
            let sha1 = folder.sha1(path)?.unwrap_or(row.sha1.clone());
            records::put(
                &content.db,
                server_id,
                &ContentFile {
                    origin: Origin::Added,
                    sha1,
                    pending: None,
                    ..row
                },
            )
            .await?;
        }
        (Some(Pending::Removed), true) => {
            folder.remove(path)?;
            records::remove(&content.db, server_id, path).await?;
        }
        (_, replace) => {
            let pack =
                records::pack(&content.db, server_id).await?.ok_or_else(
                    || Error::Invalid("This server follows no pack.".into()),
                )?;
            let archive_path =
                SharedDirs::new(content.paths.root()).pack(&pack.sha1);
            let (index, wanted) = read_pack(&archive_path).await?;
            let want = wanted.get(path).ok_or_else(|| {
                Error::NotFound(format!("The pack has no {path}."))
            })?;
            if replace {
                let mut staged = HashMap::new();
                if let Source::Download(n) = want.source {
                    let staging = pack_staging(content, server_id);
                    tokio::fs::create_dir_all(&staging).await?;
                    let target = staging.join(n.to_string());
                    fetch_index_file(
                        content,
                        &index.files[n],
                        &target,
                        &|_| {},
                    )
                    .await?;
                    staged.insert(n, target);
                }
                let placed =
                    place(&folder, &archive_path, path, &want.source, &staged)
                        .await;
                let _ =
                    tokio::fs::remove_dir_all(pack_staging(content, server_id))
                        .await;
                placed?;
            }
            records::put(
                &content.db,
                server_id,
                &ContentFile {
                    origin: Origin::Pack,
                    sha1: want.sha1.clone(),
                    pending: None,
                    ..row
                },
            )
            .await?;
        }
    }
    Ok(())
}
