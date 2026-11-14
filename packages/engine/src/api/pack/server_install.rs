//! Installing a pack on a server engine: a new one, an update, or repair.
//!
//! Every file the plan downloads is staged first, through the engine's pack client, so a download
//! that fails leaves the server as it was. Then the plan ([`server::plan`]) is applied file by file
//! through the folder sandbox, recording each as it goes. The pack is kept, so repair, a
//! client-only file turned on and a kept or replaced edit can read it again.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use futures::{StreamExt, TryStreamExt};

use super::install_from::CreatePackFile;
use super::server::{
    self, Index, IndexFile, KnownFile, PackError, ServerOn, Source, Split,
    Step, TrackedFile, Wanted,
};
use crate::Engine;
use crate::install::{
    InstallPhaseDetails, InstallPhaseId, InstallProgressReporter,
};
use crate::state::instances::adapters::sqlite::server_rows;
use crate::state::{
    CachedEntry, ContentSourceKind, InstanceLink, InstanceMetadata, ModLoader,
    PendingChange,
};
use crate::util::archive::{self, NoRoom};
use crate::util::fetch::{PackHash, fetch_pack_file};
use crate::util::sandbox::{self, Folder};

/// Pack files downloaded at once.
const CONCURRENCY: usize = 6;

/// The `.mrpack` a server instance follows.
fn kept_pack(state: &Engine, instance_id: &str) -> PathBuf {
    state
        .directories
        .config_dir
        .join("packs")
        .join(format!("{instance_id}.mrpack"))
}

fn staging(state: &Engine, instance_id: &str) -> PathBuf {
    state
        .directories
        .config_dir
        .join("pack-staging")
        .join(instance_id)
}

/// The source kind every file of the instance's pack takes.
pub(crate) fn pack_kind(link: &InstanceLink) -> ContentSourceKind {
    match link {
        InstanceLink::SharedInstance { .. } => {
            ContentSourceKind::SharedInstance
        }
        InstanceLink::ModrinthModpack { .. }
        | InstanceLink::ServerProjectModpack { .. } => {
            ContentSourceKind::ModrinthModpack
        }
        _ => ContentSourceKind::ImportedModpack,
    }
}

pub(crate) fn instance_dir(
    state: &Engine,
    metadata: &InstanceMetadata,
) -> PathBuf {
    state
        .directories
        .instances_dir()
        .join(&metadata.instance.path)
}

pub(crate) async fn metadata(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<InstanceMetadata> {
    crate::api::instance::get(state, instance_id)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "unknown instance {instance_id}"
            ))
            .into()
        })
}

pub(crate) async fn open_folder(dir: &Path) -> crate::Result<Arc<Folder>> {
    tokio::fs::create_dir_all(dir).await?;
    Ok(Arc::new(Folder::open(dir)?))
}

/// Install the pack in `file` over the instance's files, the step of a pack install that places
/// its content. The pack is kept for later.
pub(super) async fn install_pack_files(
    state: &Engine,
    instance_id: &str,
    file: &CreatePackFile,
    reporter: &InstallProgressReporter,
    details: InstallPhaseDetails,
) -> crate::Result<()> {
    keep(file, &kept_pack(state, instance_id)).await?;
    reporter
        .update(InstallPhaseId::DownloadingContent, None, details)
        .await?;
    install(state, &metadata(state, instance_id).await?, false).await
}

/// Write the pack to `kept` as a new file: the content store's files are read-only, and a copy
/// (`fs::copy`) would be too, so the next pack could not replace it.
async fn keep(file: &CreatePackFile, kept: &Path) -> crate::Result<()> {
    if let Some(parent) = kept.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let source = match file {
        CreatePackFile::Bytes(bytes) => {
            return Ok(tokio::fs::write(kept, bytes).await?);
        }
        CreatePackFile::Downloaded(downloaded) => downloaded.path(),
        CreatePackFile::Path(path) => path.as_path(),
    };
    let (source, kept) = (source.to_owned(), kept.to_owned());
    tokio::task::spawn_blocking(move || {
        std::io::copy(
            &mut std::fs::File::open(source)?,
            &mut std::fs::File::create(kept)?,
        )
    })
    .await??;
    Ok(())
}

/// Place every file of the pack the instance follows that is missing, as it was installed.
pub(crate) async fn repair(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<()> {
    let metadata = metadata(state, instance_id).await?;
    let _content = state.instance_locks.lock_content(instance_id).await;
    install(state, &metadata, true).await
}

async fn install(
    state: &Engine,
    metadata: &InstanceMetadata,
    repair: bool,
) -> crate::Result<()> {
    let id = metadata.instance.id.as_str();
    let set_id = metadata.applied_content_set.id.as_str();
    let kind = pack_kind(&metadata.link);
    let dir = instance_dir(state, metadata);
    let archive_path = kept_pack(state, id);
    if !archive_path.exists() {
        return Err(crate::ErrorKind::InputError(
            "This server follows no pack.".to_string(),
        )
        .into());
    }
    let folder = open_folder(&dir).await?;
    let case_insensitive = folder.case_insensitive()?;
    let server_on = server_rows::server_overrides(&state.pool, id).await?;
    let (
        index,
        Split {
            wanted,
            skipped,
            turned_on,
        },
    ) = read_pack(
        state,
        &archive_path,
        &dir,
        metadata.applied_content_set.loader,
        &server_on,
        case_insensitive,
    )
    .await?;
    let rows: Vec<TrackedFile> = server_rows::tracked_files(&state.pool, id)
        .await?
        .into_iter()
        .map(|(row, _)| row)
        .collect();
    let steps = {
        let folder = Arc::clone(&folder);
        tokio::task::spawn_blocking(move || {
            let level = folder.level_name();
            let sha1 = |path: &str| folder.sha1(path).ok().flatten();
            let disabled =
                |path: &str| folder.exists(&format!("{path}.disabled"));
            server::plan(
                &rows,
                &wanted,
                &server::Disk {
                    sha1: &sha1,
                    disabled: &disabled,
                    level_name: &level,
                    case_insensitive,
                },
                repair,
                kind,
            )
        })
        .await?
    };
    // What is replaced is still counted, so an update asks for a little more than it needs.
    server::check_room(&steps, archive::room(&dir)?)?;
    let staging = staging(state, id);
    let result = async {
        let staged = stage_downloads(state, &staging, &index, &steps).await?;
        apply(
            state,
            id,
            set_id,
            &folder,
            &archive_path,
            &index,
            steps,
            &staged,
        )
        .await?;
        server_rows::set_server_overrides(&state.pool, id, &turned_on).await?;
        server_rows::set_skipped_files(&state.pool, id, set_id, &skipped).await
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&staging).await;
    result
}

/// The jar metadata a server with `loader` reads first. Vanilla reads none first.
fn jar_platform(loader: ModLoader) -> Option<crate::server_jar::Platform> {
    use crate::server_jar::Platform;
    Some(match loader {
        ModLoader::Fabric => Platform::Fabric,
        ModLoader::Quilt => Platform::Quilt,
        ModLoader::NeoForge => Platform::NeoForge,
        ModLoader::Forge => Platform::Forge,
        ModLoader::Paper => Platform::Paper,
        ModLoader::Vanilla => return None,
    })
}

/// A pack's index, the files a server takes from it, and its client-only files, read from the
/// `.mrpack` at `path`. Files the index gives no sides for are looked up on Modrinth by hash;
/// when Modrinth cannot be reached they are taken, as a server is safer with them. Client-only
/// files in `server_on` are taken too (see `server::split`).
pub(crate) async fn read_pack(
    state: &Engine,
    path: &Path,
    dir: &Path,
    loader: ModLoader,
    server_on: &ServerOn,
    case_insensitive: bool,
) -> crate::Result<(Index, Split)> {
    let path = path.to_owned();
    let room = archive::room(dir)?;
    let (index, candidates) = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;
        let mut archive = zip::ZipArchive::new(file).map_err(|error| {
            PackError::from(archive::ArchiveError::from(error))
        })?;
        let index = server::read_index(&mut archive)?;
        let candidates = server::candidates(
            &index,
            &mut archive,
            jar_platform(loader),
            room,
        )?;
        Ok::<_, crate::Error>((index, candidates))
    })
    .await??;
    let ask: Vec<&str> = candidates
        .iter()
        .filter(|candidate| candidate.ask_modrinth())
        .map(|candidate| candidate.wanted.sha1.as_str())
        .collect();
    let known = known(state, &ask).await.unwrap_or_else(|error| {
        tracing::warn!(%error, "could not ask Modrinth where a pack's files run");
        server::Known::new()
    });
    let split = server::split(candidates, &known, server_on, case_insensitive)?;
    Ok((index, split))
}

/// What Modrinth knows about the files with these SHA-1s.
async fn known(state: &Engine, sha1s: &[&str]) -> crate::Result<server::Known> {
    if sha1s.is_empty() {
        return Ok(server::Known::new());
    }
    let files = CachedEntry::get_file_many(
        sha1s,
        None,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?;
    let version_ids: Vec<&str> =
        files.iter().map(|file| file.version_id.as_str()).collect();
    let versions = CachedEntry::get_version_v3_many(
        &version_ids,
        None,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?;
    Ok(files
        .into_iter()
        .map(|file| {
            let environment = versions
                .iter()
                .find(|version| version.id == file.version_id)
                .and_then(|version| version.environment);
            (
                file.hash,
                KnownFile {
                    project_id: file.project_id,
                    version_id: file.version_id,
                    environment,
                },
            )
        })
        .collect())
}

/// Download every file the plan places from the index, checked, into `staging`. Returns where
/// each landed, by index file.
async fn stage_downloads(
    state: &Engine,
    staging: &Path,
    index: &Index,
    steps: &[Step],
) -> crate::Result<HashMap<usize, PathBuf>> {
    let _ = tokio::fs::remove_dir_all(staging).await;
    tokio::fs::create_dir_all(staging).await?;
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
    futures::stream::iter(needed)
        .map(|n| async move {
            let path = staging.join(n.to_string());
            fetch_index_file(state, &index.files[n], &path).await?;
            Ok::<_, crate::Error>((n, path))
        })
        .buffer_unordered(CONCURRENCY)
        .try_collect()
        .await
}

/// Download one file of a pack index from the first allowed URL that works. The room is read when
/// the download starts, so downloads that run at once each count what the others have not
/// written yet; the whole install was checked against the disk before any of them started.
async fn fetch_index_file(
    state: &Engine,
    file: &IndexFile,
    path: &Path,
) -> crate::Result<()> {
    let client = state.pack_client.as_ref().ok_or_else(|| {
        crate::ErrorKind::InputError(
            "only a server engine installs a pack this way".to_string(),
        )
    })?;
    let hash = match &file.hashes.sha512 {
        Some(sha512) => PackHash::Sha512(sha512),
        None => PackHash::Sha1(&file.hashes.sha1),
    };
    let room = archive::room(path.parent().unwrap_or(path))?;
    let mut last = None;
    for url in &file.downloads {
        match fetch_pack_file(client, url, path, hash, room).await {
            Ok(()) => return Ok(()),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "{} has no allowed download",
            file.path
        ))
        .into()
    }))
}

/// Carry out a plan, recording every file as it is placed. Each file is hashed again right
/// before it is replaced or deleted: one that changed since the plan was made is left as it is
/// and marked pending, the way the plan treats an edit it saw.
#[allow(clippy::too_many_arguments)]
async fn apply(
    state: &Engine,
    id: &str,
    set_id: &str,
    folder: &Arc<Folder>,
    archive_path: &Path,
    index: &Index,
    steps: Vec<Step>,
    staged: &HashMap<usize, PathBuf>,
) -> crate::Result<()> {
    let pool = &state.pool;
    for step in steps {
        match step {
            Step::Place {
                path,
                wanted,
                row,
                expected,
                before,
            } => {
                let now = sha1_now(folder, &path).await?;
                if now != expected {
                    if let Some(theirs) =
                        changed_meanwhile(&path, row.is_some(), before, now)
                    {
                        server_rows::upsert_tracked_file(
                            pool, id, set_id, &theirs,
                        )
                        .await?;
                    }
                    continue;
                }
                place(folder, archive_path, &path, &wanted, staged).await?;
                if let Some(mut row) = row {
                    if let Source::Download(n) = wanted.source
                        && let Some((project, version)) = index.files[n]
                            .downloads
                            .iter()
                            .find_map(|url| server::ids_from_cdn(url))
                    {
                        row.project_id = Some(project);
                        row.version_id = Some(version);
                    }
                    server_rows::upsert_tracked_file(pool, id, set_id, &row)
                        .await?;
                }
            }
            Step::Record(row) => {
                server_rows::upsert_tracked_file(pool, id, set_id, &row).await?
            }
            // A path that became the server's since the plan is only forgotten.
            Step::Delete(row)
                if sandbox::is_server_owned(
                    &row.relative_path,
                    &folder.level_name(),
                ) =>
            {
                server_rows::remove_tracked_file(pool, id, &row.relative_path)
                    .await?
            }
            Step::Delete(row) => {
                match sha1_now(folder, &row.relative_path).await? {
                    Some(sha1) if sha1 == row.sha1 => {
                        let folder = Arc::clone(folder);
                        let target = row.relative_path.clone();
                        tokio::task::spawn_blocking(move || {
                            folder.remove(&target)
                        })
                        .await??;
                        server_rows::remove_tracked_file(
                            pool,
                            id,
                            &row.relative_path,
                        )
                        .await?;
                    }
                    None => {
                        server_rows::remove_tracked_file(
                            pool,
                            id,
                            &row.relative_path,
                        )
                        .await?
                    }
                    Some(_) => {
                        let row = TrackedFile {
                            pending: Some(PendingChange::Removed),
                            ..row
                        };
                        server_rows::upsert_tracked_file(
                            pool, id, set_id, &row,
                        )
                        .await?;
                    }
                }
            }
            Step::Forget(path) => {
                server_rows::remove_tracked_file(pool, id, &path).await?
            }
        }
    }
    Ok(())
}

/// The row to keep for a file a place step found changed since the plan: what was recorded,
/// now pending; or, for a file nobody recorded, the owner's file with its hash, pending. `None`
/// for a server-owned file someone made meanwhile, which is theirs.
fn changed_meanwhile(
    path: &str,
    tracked: bool,
    before: Option<TrackedFile>,
    now: Option<String>,
) -> Option<TrackedFile> {
    let theirs = match (before, now) {
        (Some(before), _) => before,
        (None, Some(sha1)) if tracked => TrackedFile {
            relative_path: path.to_owned(),
            source_kind: ContentSourceKind::Local,
            sha1,
            size: 0,
            project_id: None,
            version_id: None,
            pending: None,
        },
        _ => return None,
    };
    Some(TrackedFile {
        pending: Some(PendingChange::Changed),
        ..theirs
    })
}

/// The SHA-1 of the file at `path` as it is now.
pub(crate) async fn sha1_now(
    folder: &Arc<Folder>,
    path: &str,
) -> crate::Result<Option<String>> {
    let folder = Arc::clone(folder);
    let path = path.to_owned();
    Ok(tokio::task::spawn_blocking(move || folder.sha1(&path)).await??)
}

/// Write one file the pack wants: a staged download, or an entry of the pack's own archive. An
/// entry that inflates past the size it was planned with is refused before it replaces
/// anything, so the budget checked before the install holds.
async fn place(
    folder: &Arc<Folder>,
    archive_path: &Path,
    path: &str,
    wanted: &Wanted,
    staged: &HashMap<usize, PathBuf>,
) -> crate::Result<()> {
    let folder = Arc::clone(folder);
    let path = path.to_owned();
    let size = wanted.size;
    let source = match &wanted.source {
        Source::Download(n) => {
            Err(staged.get(n).cloned().ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "{path} was not downloaded"
                ))
            })?)
        }
        Source::Override(entry) => Ok(*entry),
    };
    let archive_path = archive_path.to_owned();
    tokio::task::spawn_blocking(move || -> crate::Result<()> {
        match source {
            Err(staged) => {
                folder.write(&path, &mut std::fs::File::open(staged)?)?;
            }
            Ok(entry) => {
                let mut archive =
                    zip::ZipArchive::new(std::fs::File::open(archive_path)?)?;
                let entry = archive.by_index(entry)?;
                folder.write(
                    &path,
                    &mut archive::AtMost {
                        inner: entry,
                        left: size,
                    },
                )?;
            }
        }
        Ok(())
    })
    .await?
}

/// Write the pack's file at `path` on its own, outside an install: a download is staged first,
/// and the file must fit on the disk.
pub(crate) async fn place_one(
    state: &Engine,
    metadata: &InstanceMetadata,
    folder: &Arc<Folder>,
    index: &Index,
    path: &str,
    want: &Wanted,
) -> crate::Result<()> {
    let id = metadata.instance.id.as_str();
    NoRoom::check(
        path,
        want.size,
        archive::room(&instance_dir(state, metadata))?,
    )
    .map_err(PackError::from)?;
    let staging = staging(state, id);
    let result = async {
        let mut staged = HashMap::new();
        if let Source::Download(n) = want.source {
            tokio::fs::create_dir_all(&staging).await?;
            let target = staging.join(n.to_string());
            fetch_index_file(state, &index.files[n], &target).await?;
            staged.insert(n, target);
        }
        place(folder, &kept_pack(state, id), path, want, &staged).await
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&staging).await;
    result
}

/// The pack the instance follows, read again: its index and what the server takes, with
/// `server_on` turned on.
pub(crate) async fn read_kept_pack(
    state: &Engine,
    metadata: &InstanceMetadata,
    folder: &Folder,
    server_on: &ServerOn,
) -> crate::Result<(Index, Split)> {
    let id = metadata.instance.id.as_str();
    let path = kept_pack(state, id);
    if !path.exists() {
        return Err(crate::ErrorKind::InputError(
            "This server follows no pack.".to_string(),
        )
        .into());
    }
    read_pack(
        state,
        &path,
        &instance_dir(state, metadata),
        metadata.applied_content_set.loader,
        server_on,
        folder.case_insensitive()?,
    )
    .await
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use contracts::Side;

    use super::*;
    use crate::api::instance::{decide_edited, edited_files};
    use crate::state::tests::{add_instance, config};

    fn pack(files: &[(&str, &str)]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("modrinth.index.json", options).unwrap();
        writer
            .write_all(
                br#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"p","dependencies":{"minecraft":"1.21.1"}}"#,
            )
            .unwrap();
        for (name, text) in files {
            writer.start_file(*name, options).unwrap();
            writer.write_all(text.as_bytes()).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[tokio::test]
    async fn a_pack_from_the_content_store_is_kept_so_the_next_one_replaces_it()
    {
        let tmp = tempfile::tempdir().unwrap();
        let stored = tmp.path().join("stored.mrpack");
        let kept = tmp.path().join("packs/s.mrpack");
        std::fs::write(&stored, pack(&[])).unwrap();
        let mut permissions = std::fs::metadata(&stored).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&stored, permissions).unwrap();

        let file = CreatePackFile::Path(stored.clone());
        keep(&file, &kept).await.unwrap();
        keep(&file, &kept).await.unwrap();
        assert_eq!(std::fs::read(&kept).unwrap(), pack(&[]));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_update_leaves_an_edited_file_for_the_owner_to_keep_or_replace()
    {
        crate::install_crypto_provider();
        let tmp = tempfile::tempdir().unwrap();
        let state = Engine::new(crate::state::EngineConfig {
            side: Side::Server,
            ..config(tmp.path().to_path_buf())
        })
        .await
        .unwrap();
        add_instance(&state, "s", "srv").await;
        let kept = kept_pack(&state, "s");
        std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
        let dir = state.directories.instances_dir().join("srv");

        std::fs::write(
            &kept,
            pack(&[
                ("overrides/config/a.toml", "one"),
                ("server-overrides/config/b.toml", "server"),
            ]),
        )
        .unwrap();
        repair(&state, "s").await.unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("config/a.toml")).unwrap(),
            "one"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("config/b.toml")).unwrap(),
            "server"
        );
        assert!(edited_files(&state, "s").await.unwrap().is_empty());

        std::fs::write(dir.join("config/a.toml"), "mine").unwrap();
        std::fs::write(&kept, pack(&[("overrides/config/a.toml", "two")]))
            .unwrap();
        repair(&state, "s").await.unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("config/a.toml")).unwrap(),
            "mine"
        );
        // The pack dropped b.toml, which nobody edited.
        assert!(!dir.join("config/b.toml").exists());
        let edited = edited_files(&state, "s").await.unwrap();
        assert_eq!(
            edited,
            [crate::api::instance::EditedFile {
                path: "config/a.toml".into(),
                deleted: false,
                pending: Some(PendingChange::Changed),
            }]
        );

        decide_edited(&state, "s", "config/a.toml", true)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("config/a.toml")).unwrap(),
            "two"
        );
        assert!(edited_files(&state, "s").await.unwrap().is_empty());
    }
}
