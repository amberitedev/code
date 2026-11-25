//! A server engine installs a pack its own way. It takes only the files that run on a server
//! (`server_split`) and places them through the folder sandbox, after staging every download
//! through its pack client, so a download that fails leaves the server as it was. The pack is kept,
//! so repair and a client-only file turned on can read it again. An update overwrites the files the
//! pack placed, edited or not, and never a file the server owns (`sandbox::is_server_owned`).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Cursor, ErrorKind, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::{StreamExt, TryStreamExt};

use super::install_from::{CreatePackFile, PackFormat};
use crate::Engine;
use crate::install::{
    InstallPhaseDetails, InstallPhaseId, InstallProgressReporter,
};
use crate::pack::install_from::{EnvType, PackFile, PackFileHash};
use crate::state::instances::ContentSourceKind;
use crate::state::instances::adapters::sqlite::content_rows::{
    self, ServerOn, SkippedFile, TrackedFile,
};
use crate::state::instances::commands::names_client_only;
use crate::state::{
    CachedEntry, InstanceLink, InstanceMetadata, ModLoader, SideType,
    VersionEnvironment,
};
use crate::util::archive::{self, ArchiveError, NoRoom};
use crate::util::fetch::{PackHash, fetch_pack_file};
use crate::util::pack_hosts;
use crate::util::sandbox::{self, Folder, PathError};

/// Pack files a server downloads at once.
const SERVER_DOWNLOAD_CONCURRENCY: usize = 6;

/// The index is small; a pack claiming a larger one is refused.
const MAX_INDEX: u64 = 32 * 1024 * 1024;

/// The largest jar of `overrides/` whose metadata is read. A jar is read into memory whole to
/// inspect it; one past this is not inspected, and is taken unless Modrinth knows it.
const MAX_INSPECTED_JAR: u64 = 128 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error("this is not a Modrinth pack: {0}")]
    NotAPack(String),
    #[error("the pack lists a file {0} that is refused")]
    Path(#[from] PathError),
    #[error(
        "the pack lists {0}, which cannot be downloaded from an allowed host"
    )]
    Host(String),
    #[error("the pack lists both {0} and {1}, which are the same file here")]
    Collision(String, String),
    #[error(transparent)]
    NoRoom(#[from] NoRoom),
}

/// Read and check a pack's index.
fn read_server_index<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<PackFormat, PackError> {
    let not_a_pack = |reason: &str| PackError::NotAPack(reason.to_owned());
    let mut text = String::new();
    archive
        .by_name("modrinth.index.json")
        .map_err(|_| not_a_pack("it has no modrinth.index.json"))?
        .take(MAX_INDEX)
        .read_to_string(&mut text)
        .map_err(|_| not_a_pack("its index is unreadable"))?;
    let index: PackFormat = serde_json::from_str(&text)
        .map_err(|error| PackError::NotAPack(error.to_string()))?;
    if index.format_version != 1 || index.game != "minecraft" {
        return Err(not_a_pack("it is not a Minecraft pack of format 1"));
    }
    Ok(index)
}

/// The project and version of a file on Modrinth's CDN, from its URL:
/// `https://cdn.modrinth.com/data/<project>/versions/<version>/<file>`.
fn ids_from_cdn(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://cdn.modrinth.com/data/")?;
    let mut parts = rest.split('/');
    let project = parts.next()?;
    (parts.next()? == "versions").then_some(())?;
    let version = parts.next()?;
    parts.next()?;
    Some((project.to_owned(), version.to_owned()))
}

/// Whether a server takes a file of a pack, by the first of these that knows:
///
/// 1. `env`, the server side of the pack index's `env`: a server leaves out a file whose server
///    side is unsupported. A file of `overrides/` has none.
/// 2. `modrinth`, where Modrinth says the version with the file's hash runs, by upstream's export
///    rule (`get_mrpack_environment`).
/// 3. `jar_client_only`, a jar Modrinth does not know naming itself client only in its metadata.
///
/// A file none of them rules out is taken: guessing a server-side mod is client only breaks the
/// server, while an extra client mod on a server only costs a warning in its log.
fn server_takes(
    env: Option<SideType>,
    modrinth: Option<VersionEnvironment>,
    jar_client_only: bool,
) -> bool {
    if let Some(server) = env {
        return server != SideType::Unsupported;
    }
    if modrinth.is_some() {
        return crate::api::instance::get_mrpack_environment(modrinth)
            .get(&EnvType::Server)
            != Some(&SideType::Unsupported);
    }
    !jar_client_only
}

/// What Modrinth knows about a file of a pack, found by its hash.
#[derive(Debug, Clone)]
struct KnownFile {
    project_id: String,
    version_id: String,
    environment: Option<VersionEnvironment>,
}

/// What Modrinth knows about the files that ask, by SHA-1.
type Known = HashMap<String, KnownFile>;

/// Where the pack gets a file from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// `files[n]` of the index.
    Download(usize),
    /// Entry `n` of the pack archive.
    Override(usize),
}

/// A file the pack wants at a path.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Wanted {
    sha1: String,
    source: Source,
    /// Its size in bytes, as the index lists it or the archive holds it.
    size: u64,
}

/// What [`server_split`] makes of a pack's files.
#[derive(Debug, Default)]
struct Split {
    /// The files a server takes, by path.
    wanted: BTreeMap<String, Wanted>,
    /// The client-only files it leaves out.
    skipped: Vec<SkippedFile>,
    /// The paths of `wanted` taken only because someone turned them on ([`ServerOn`]).
    turned_on: HashSet<String>,
}

/// A path as the file system compares it: folded when it treats `Config/a.toml` and
/// `config/a.toml` as one file (see `Folder::case_insensitive`), exact otherwise.
fn fold(path: &str, case_insensitive: bool) -> String {
    if case_insensitive {
        sandbox::fold_case(path)
    } else {
        path.to_owned()
    }
}

/// A file of a pack, before it is known whether the server takes it.
#[derive(Debug, Clone)]
struct Candidate {
    path: String,
    wanted: Wanted,
    /// The server side of the index's `env`; `server-overrides/` are the server's.
    env: Option<SideType>,
    /// For a jar of `overrides/`: its own metadata says it is client only.
    jar_client_only: bool,
    /// For an index file, its download URLs, which may name its Modrinth project and version.
    downloads: Vec<String>,
}

impl Candidate {
    /// Whether Modrinth is asked where the file runs: the index does not say, for a file it
    /// downloads or a jar of `overrides/`.
    fn ask_modrinth(&self) -> bool {
        self.env.is_none()
            && (matches!(self.wanted.source, Source::Download(_))
                || self.path.ends_with(".jar"))
    }
}

/// Every file of a pack, with what the index and, for jars of `overrides/`, their metadata say
/// about their sides. `loader` is the server's, whose jar metadata is read first. Every override
/// is inflated to hash it, all of them drawing on `room` (see `archive::room`): a pack whose
/// overrides do not fit on the disk could not be installed anyway.
fn server_candidates<R: Read + Seek>(
    index: &PackFormat,
    archive: &mut zip::ZipArchive<R>,
    loader: ModLoader,
    mut room: u64,
) -> Result<Vec<Candidate>, PackError> {
    let mut candidates = Vec::new();
    for (n, file) in index.files.iter().enumerate() {
        let sha1 = file.hashes.get(&PackFileHash::Sha1).ok_or_else(|| {
            PackError::NotAPack(format!("{} has no SHA-1", file.path))
        })?;
        candidates.push(Candidate {
            path: sandbox::clean(file.path.as_str())?,
            wanted: Wanted {
                sha1: sha1.to_ascii_lowercase(),
                source: Source::Download(n),
                size: u64::from(file.file_size),
            },
            env: file.env.as_ref().map(|env| {
                env.get(&EnvType::Server)
                    .cloned()
                    .unwrap_or(SideType::Required)
            }),
            jar_client_only: false,
            downloads: file.downloads.clone(),
        });
    }
    let entries = archive::entries(archive)?;
    for (prefix, env) in [
        ("overrides/", None),
        ("server-overrides/", Some(SideType::Required)),
    ] {
        for (n, (entry, is_dir)) in entries.iter().enumerate() {
            let Some(path) = entry.strip_prefix(prefix).filter(|_| !is_dir)
            else {
                continue;
            };
            let (sha1, size) =
                archive::entry_sha1(archive, n, entry, &mut room)?;
            let jar_client_only = env.is_none()
                && path.ends_with(".jar")
                && jar_names_client_only(archive, n, size, loader);
            candidates.push(Candidate {
                path: path.to_owned(),
                wanted: Wanted {
                    sha1,
                    source: Source::Override(n),
                    size,
                },
                env: env.clone(),
                jar_client_only,
                downloads: Vec::new(),
            });
        }
    }
    Ok(candidates)
}

/// Whether the jar at entry `n` of the pack names itself client only. Best effort: `false` for a
/// jar that cannot be read or is too large to read ([`MAX_INSPECTED_JAR`]).
fn jar_names_client_only<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    n: usize,
    size: u64,
    loader: ModLoader,
) -> bool {
    if size > MAX_INSPECTED_JAR {
        return false;
    }
    let mut bytes = Vec::new();
    archive
        .by_index(n)
        .is_ok_and(|entry| entry.take(size).read_to_end(&mut bytes).is_ok())
        && zip::ZipArchive::new(Cursor::new(bytes))
            .is_ok_and(|mut jar| names_client_only(&mut jar, loader))
}

/// The files a server takes, by path, and the ones it leaves out because they are client only,
/// by [`server_takes`]. `server-overrides/` wins over `overrides/`. A client-only file in
/// `server_on`, by path or Modrinth project, is taken anyway: someone turned it on for the server.
/// A download the server takes must come from an allowed host.
fn server_split(
    candidates: Vec<Candidate>,
    known: &Known,
    server_on: &ServerOn,
    case_insensitive: bool,
) -> Result<Split, PackError> {
    let mut wanted = BTreeMap::new();
    let mut skipped = Vec::new();
    let mut turned_on = HashSet::new();
    // Two spellings of one file are refused; the same spelling twice is an override.
    let mut spellings: HashMap<String, String> = HashMap::new();
    for candidate in candidates {
        let modrinth = candidate
            .ask_modrinth()
            .then(|| known.get(&candidate.wanted.sha1))
            .flatten();
        let (project_id, version_id) = modrinth
            .map(|file| (file.project_id.clone(), file.version_id.clone()))
            .or_else(|| {
                candidate.downloads.iter().find_map(|url| ids_from_cdn(url))
            })
            .unzip();
        let client_only = !server_takes(
            candidate.env.clone(),
            modrinth.and_then(|file| file.environment),
            candidate.jar_client_only,
        );
        let on = client_only
            && (server_on.paths.contains(&candidate.path)
                || project_id
                    .as_ref()
                    .is_some_and(|id| server_on.projects.contains(id)));
        let skip = |candidate: Candidate| SkippedFile {
            relative_path: candidate.path,
            sha1: candidate.wanted.sha1,
            size: candidate.wanted.size,
            project_id,
            version_id,
        };
        if client_only && !on {
            skipped.push(skip(candidate));
            continue;
        }
        let usable = matches!(candidate.wanted.source, Source::Override(_))
            || candidate.downloads.iter().any(|url| {
                reqwest::Url::parse(url)
                    .is_ok_and(|url| pack_hosts::allowed(&url, false))
            });
        // A file turned on by hand stays client only rather than fail the whole pack.
        if !usable && on {
            skipped.push(skip(candidate));
            continue;
        }
        if !usable {
            return Err(PackError::Host(candidate.path));
        }
        let path = candidate.path;
        match spellings.insert(fold(&path, case_insensitive), path.clone()) {
            Some(other) if other != path => {
                return Err(PackError::Collision(other, path));
            }
            _ => {
                if on {
                    turned_on.insert(path.clone());
                } else {
                    turned_on.remove(&path);
                }
                wanted.insert(path, candidate.wanted);
            }
        }
    }
    // A file the server takes from elsewhere in the pack is not left out.
    skipped.retain(|file| !wanted.contains_key(&file.relative_path));
    Ok(Split {
        wanted,
        skipped,
        turned_on,
    })
}

/// One thing a server's pack install does to a path.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    /// Write the pack's file, then record `row`. `row` is `None` for a server-owned file, which
    /// is placed only where none exists and then belongs to the server. `disabled`: the owner
    /// turned the file off, so the pack's version replaces it at `<path>.disabled`.
    Place {
        path: String,
        wanted: Wanted,
        row: Option<TrackedFile>,
        disabled: bool,
    },
    /// The file is right as it is; record `row`.
    Record(TrackedFile),
    /// The pack dropped a file it placed: delete it, unless the owner edited it, which makes it
    /// theirs.
    Drop(TrackedFile),
    /// Forget the row: it names a file another row now names with the pack's new spelling, or a
    /// server-owned file the pack wants.
    Forget(String),
}

/// The most a set of steps writes, refused before anything is written unless it fits in `room`
/// (see `archive::room`): one check for the whole install. A download is written twice, staged
/// and then placed, so it counts twice. Download sizes are the index's word, so for them this is
/// an estimate; each download is cut off once it no longer fits.
fn check_room(steps: &[Step], room: u64) -> Result<u64, PackError> {
    let total = steps
        .iter()
        .map(|step| match step {
            Step::Place {
                wanted:
                    Wanted {
                        source: Source::Download(_),
                        size,
                        ..
                    },
                ..
            } => size.saturating_mul(2),
            Step::Place { wanted, .. } => wanted.size,
            _ => 0,
        })
        .fold(0u64, u64::saturating_add);
    NoRoom::check("the pack", total, room)?;
    Ok(total)
}

/// What installing `wanted` over a server's files does, given the rows recorded for them and
/// `sha1`, the hash of the file at a path on disk. `kind` is the source kind of the pack being
/// installed, which every file it places or keeps takes. Rows and wanted paths are matched the
/// way the file system compares paths.
fn plan(
    rows: &[TrackedFile],
    wanted: &BTreeMap<String, Wanted>,
    sha1: &dyn Fn(&str) -> Option<String>,
    level_name: &str,
    case_insensitive: bool,
    kind: ContentSourceKind,
) -> Vec<Step> {
    let key = |path: &str| fold(path, case_insensitive);
    let rows: HashMap<String, &TrackedFile> = rows
        .iter()
        .map(|row| (key(&row.relative_path), row))
        .collect();
    let wanted_keys: HashSet<String> =
        wanted.keys().map(|path| key(path)).collect();
    let mut steps = Vec::new();
    for (path, want) in wanted {
        let row = rows.get(&key(path)).copied();
        // The pack spells a file it already placed another way: the row follows the new
        // spelling, and the old one is only forgotten, never deleted, since it is this file.
        if let Some(row) = row.filter(|row| row.relative_path != *path) {
            steps.push(Step::Forget(row.relative_path.clone()));
        }
        if sandbox::is_server_owned(path, level_name) {
            if sha1(path).is_none() {
                steps.push(Step::Place {
                    path: path.clone(),
                    wanted: want.clone(),
                    row: None,
                    disabled: false,
                });
            }
            if row.is_some() {
                steps.push(Step::Forget(path.clone()));
            }
            continue;
        }
        let enabled = sha1(path);
        let off = enabled
            .is_none()
            .then(|| sha1(&format!("{path}.disabled")))
            .flatten();
        let disabled = off.is_some();
        let on_disk = enabled.or(off);
        // The project a row names stays with the file it was recorded for.
        let same = row.filter(|row| row.sha1 == want.sha1);
        let recorded = TrackedFile {
            relative_path: path.clone(),
            source_kind: kind,
            sha1: want.sha1.clone(),
            size: want.size,
            project_id: same.and_then(|row| row.project_id.clone()),
            version_id: same.and_then(|row| row.version_id.clone()),
        };
        steps.push(if on_disk.as_ref() == Some(&want.sha1) {
            Step::Record(recorded)
        } else {
            Step::Place {
                path: path.clone(),
                wanted: want.clone(),
                row: Some(recorded),
                disabled,
            }
        });
    }
    for row in rows.values() {
        if row.by_pack() && !wanted_keys.contains(&key(&row.relative_path)) {
            steps.push(Step::Drop((*row).clone()));
        }
    }
    // Forgetting an old spelling comes before recording the new one.
    steps.sort_by(|a, b| {
        step_path(a).cmp(step_path(b)).then_with(|| {
            matches!(b, Step::Forget(_)).cmp(&matches!(a, Step::Forget(_)))
        })
    });
    steps
}

fn step_path(step: &Step) -> &str {
    match step {
        Step::Place { path, .. } | Step::Forget(path) => path,
        Step::Drop(row) | Step::Record(row) => &row.relative_path,
    }
}

/// The `.mrpack` a server instance follows.
fn kept_pack(state: &Engine, instance_id: &str) -> PathBuf {
    state
        .directories
        .config_dir
        .join("packs")
        .join(format!("{instance_id}.mrpack"))
}

fn pack_staging(state: &Engine, instance_id: &str) -> PathBuf {
    state
        .directories
        .config_dir
        .join("pack-staging")
        .join(instance_id)
}

/// Delete the pack a removed server followed and what its last install staged.
pub(crate) async fn forget_server_pack(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<()> {
    for result in [
        tokio::fs::remove_file(kept_pack(state, instance_id)).await,
        tokio::fs::remove_dir_all(pack_staging(state, instance_id)).await,
    ] {
        match result {
            Err(error) if error.kind() != ErrorKind::NotFound => {
                return Err(error.into());
            }
            _ => {}
        }
    }
    Ok(())
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

pub(crate) async fn instance_metadata(
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

/// A server engine's pack install, once the instance has the pack's information: keep the pack
/// in `file`, install it over the server's files, then install the game.
pub(super) async fn install_pack(
    state: &Engine,
    instance_id: String,
    file: &CreatePackFile,
    reporter: &InstallProgressReporter,
    details: InstallPhaseDetails,
) -> crate::Result<String> {
    keep_pack(file, &kept_pack(state, &instance_id)).await?;
    reporter
        .update(InstallPhaseId::DownloadingContent, None, details)
        .await?;
    install_kept_pack(state, &instance_metadata(state, &instance_id).await?)
        .await?;
    crate::launcher::install_minecraft_for_instance_id_with_reporter(
        &instance_id,
        false,
        Some(reporter.clone()),
        state,
    )
    .await?;
    reporter.clear_context().await?;
    Ok(instance_id)
}

/// Write the pack to `kept` as a new file: the content store's files are read-only, and a copy
/// (`fs::copy`) would be too, so the next pack could not replace it.
async fn keep_pack(file: &CreatePackFile, kept: &Path) -> crate::Result<()> {
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

/// Install the pack the server follows again, placing every file of it that is missing or
/// changed.
pub(crate) async fn repair_server_pack(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<()> {
    let metadata = instance_metadata(state, instance_id).await?;
    let _content = state.instance_locks.lock_content(instance_id).await;
    install_kept_pack(state, &metadata).await
}

async fn install_kept_pack(
    state: &Engine,
    metadata: &InstanceMetadata,
) -> crate::Result<()> {
    let id = metadata.instance.id.as_str();
    let set_id = metadata.applied_content_set.id.as_str();
    let folder = open_folder(&instance_dir(state, metadata)).await?;
    let server_on = content_rows::server_overrides(&state.pool, id).await?;
    let (
        index,
        Split {
            wanted,
            skipped,
            turned_on,
        },
    ) = read_kept_pack(state, metadata, &folder, &server_on).await?;
    let rows: Vec<TrackedFile> = content_rows::tracked_files(&state.pool, id)
        .await?
        .into_iter()
        .map(|(row, _)| row)
        .collect();
    let kind = pack_kind(&metadata.link);
    let case_insensitive = folder.case_insensitive()?;
    let steps = {
        let folder = Arc::clone(&folder);
        tokio::task::spawn_blocking(move || {
            plan(
                &rows,
                &wanted,
                &|path| folder.sha1(path).ok().flatten(),
                &folder.level_name(),
                case_insensitive,
                kind,
            )
        })
        .await?
    };
    // What is replaced is still counted, so an update asks for a little more than it needs.
    check_room(&steps, archive::room(&instance_dir(state, metadata))?)?;
    let staging = pack_staging(state, id);
    let result = async {
        let staged = stage_downloads(state, &staging, &index, &steps).await?;
        apply_steps(state, metadata, &folder, &index, steps, &staged).await?;
        content_rows::set_server_overrides(&state.pool, id, &turned_on).await?;
        content_rows::set_skipped_files(&state.pool, id, set_id, &skipped).await
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&staging).await;
    result
}

/// The pack the server follows, read again: its index and what the server takes, with
/// `server_on` turned on. Files the index gives no sides for are looked up on Modrinth by hash;
/// when Modrinth cannot be reached they are taken, as a server is safer with them.
async fn read_kept_pack(
    state: &Engine,
    metadata: &InstanceMetadata,
    folder: &Folder,
    server_on: &ServerOn,
) -> crate::Result<(PackFormat, Split)> {
    let path = kept_pack(state, &metadata.instance.id);
    if !path.exists() {
        return Err(crate::ErrorKind::InputError(
            "This server follows no pack.".to_string(),
        )
        .into());
    }
    let room = archive::room(&instance_dir(state, metadata))?;
    let loader = metadata.applied_content_set.loader;
    let (index, candidates) = tokio::task::spawn_blocking(move || {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path)?)
            .map_err(|error| PackError::from(ArchiveError::from(error)))?;
        let index = read_server_index(&mut archive)?;
        let candidates = server_candidates(&index, &mut archive, loader, room)?;
        Ok::<_, crate::Error>((index, candidates))
    })
    .await??;
    let ask: Vec<&str> = candidates
        .iter()
        .filter(|candidate| candidate.ask_modrinth())
        .map(|candidate| candidate.wanted.sha1.as_str())
        .collect();
    let known = known_files(state, &ask).await.unwrap_or_else(|error| {
        tracing::warn!(%error, "could not ask Modrinth where a pack's files run");
        Known::new()
    });
    let split = server_split(
        candidates,
        &known,
        server_on,
        folder.case_insensitive()?,
    )?;
    Ok((index, split))
}

/// What Modrinth knows about the files with these SHA-1s.
async fn known_files(state: &Engine, sha1s: &[&str]) -> crate::Result<Known> {
    if sha1s.is_empty() {
        return Ok(Known::new());
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
    index: &PackFormat,
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
        .buffer_unordered(SERVER_DOWNLOAD_CONCURRENCY)
        .try_collect()
        .await
}

/// Download one file of a pack index from the first allowed URL that works. The room is read when
/// the download starts, so downloads that run at once each count what the others have not
/// written yet; the whole install was checked against the disk before any of them started.
async fn fetch_index_file(
    state: &Engine,
    file: &PackFile,
    path: &Path,
) -> crate::Result<()> {
    let client = state.pack_client.as_ref().ok_or_else(|| {
        crate::ErrorKind::InputError(
            "only a server engine installs a pack this way".to_string(),
        )
    })?;
    let hash = match (
        file.hashes.get(&PackFileHash::Sha512),
        file.hashes.get(&PackFileHash::Sha1),
    ) {
        (Some(sha512), _) => PackHash::Sha512(sha512),
        (None, Some(sha1)) => PackHash::Sha1(sha1),
        (None, None) => {
            return Err(PackError::NotAPack(format!(
                "{} has no SHA-1",
                file.path
            ))
            .into());
        }
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

/// Carry out a plan, recording every file as it is placed. A file the plan drops is hashed again
/// right before it is deleted, so one edited meanwhile is kept as the owner's.
async fn apply_steps(
    state: &Engine,
    metadata: &InstanceMetadata,
    folder: &Arc<Folder>,
    index: &PackFormat,
    steps: Vec<Step>,
    staged: &HashMap<usize, PathBuf>,
) -> crate::Result<()> {
    let (pool, id) = (&state.pool, metadata.instance.id.as_str());
    let set_id = metadata.applied_content_set.id.as_str();
    let archive_path = kept_pack(state, id);
    for step in steps {
        match step {
            // A server-owned file someone made since the plan is theirs.
            Step::Place {
                path, row: None, ..
            } if folder.exists(&path) => {}
            Step::Place {
                path,
                wanted,
                row,
                disabled,
            } => {
                let target = if disabled {
                    format!("{path}.disabled")
                } else {
                    path
                };
                place(folder, &archive_path, &target, &wanted, staged).await?;
                if let Some(mut row) = row {
                    if let Source::Download(n) = wanted.source
                        && let Some((project, version)) = index.files[n]
                            .downloads
                            .iter()
                            .find_map(|url| ids_from_cdn(url))
                    {
                        row.project_id = Some(project);
                        row.version_id = Some(version);
                    }
                    content_rows::upsert_tracked_file(pool, id, set_id, &row)
                        .await?;
                }
            }
            Step::Record(row) => {
                content_rows::upsert_tracked_file(pool, id, set_id, &row)
                    .await?
            }
            Step::Drop(row) => {
                let server_owned = sandbox::is_server_owned(
                    &row.relative_path,
                    &folder.level_name(),
                );
                // The owner may have turned the file off, as `plan` reads it too.
                let mut on_disk = None;
                for path in [
                    row.relative_path.clone(),
                    format!("{}.disabled", row.relative_path),
                ] {
                    if let Some((sha1, size)) =
                        on_disk_now(folder, &path).await?
                    {
                        on_disk = Some((path, sha1, size));
                        break;
                    }
                }
                match on_disk {
                    Some((target, sha1, _))
                        if !server_owned && sha1 == row.sha1 =>
                    {
                        let folder = Arc::clone(folder);
                        tokio::task::spawn_blocking(move || {
                            folder.remove(&target)
                        })
                        .await??;
                        content_rows::remove_tracked_file(
                            pool,
                            id,
                            &row.relative_path,
                        )
                        .await?;
                    }
                    Some((_, sha1, size)) if !server_owned => {
                        let theirs = TrackedFile {
                            source_kind: ContentSourceKind::Local,
                            sha1,
                            size,
                            ..row
                        };
                        content_rows::upsert_tracked_file(
                            pool, id, set_id, &theirs,
                        )
                        .await?;
                    }
                    _ => {
                        content_rows::remove_tracked_file(
                            pool,
                            id,
                            &row.relative_path,
                        )
                        .await?
                    }
                }
            }
            Step::Forget(path) => {
                content_rows::remove_tracked_file(pool, id, &path).await?
            }
        }
    }
    Ok(())
}

/// The SHA-1 and size of the file at `path` as it is now.
async fn on_disk_now(
    folder: &Arc<Folder>,
    path: &str,
) -> crate::Result<Option<(String, u64)>> {
    let folder = Arc::clone(folder);
    let path = path.to_owned();
    Ok(tokio::task::spawn_blocking(move || -> std::io::Result<_> {
        let Some(sha1) = folder.sha1(&path)? else {
            return Ok(None);
        };
        Ok(Some((sha1, folder.metadata(&path)?.len())))
    })
    .await??)
}

/// Write one file the pack wants at `path`: a staged download, or an entry of the pack's own
/// archive. An entry that inflates past the size it was planned with is refused before it
/// replaces anything, so the budget checked before the install holds.
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

/// Place a client-only file of the pack the server follows, which someone turned on for the
/// server: the pack is read again with it on, its download staged first, and it must fit on the
/// disk. Returns the SHA-1 and size of what was placed.
pub(crate) async fn place_turned_on(
    state: &Engine,
    metadata: &InstanceMetadata,
    folder: &Arc<Folder>,
    path: &str,
) -> crate::Result<(String, u64)> {
    let id = metadata.instance.id.as_str();
    let mut server_on = content_rows::server_overrides(&state.pool, id).await?;
    server_on.paths.insert(path.to_owned());
    let (index, split) =
        read_kept_pack(state, metadata, folder, &server_on).await?;
    let want = split.wanted.get(path).cloned().ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "The pack cannot place {path} on a server: it is not on a host packs may download from."
        ))
    })?;
    NoRoom::check(
        path,
        want.size,
        archive::room(&instance_dir(state, metadata))?,
    )
    .map_err(PackError::from)?;
    let staging = pack_staging(state, id);
    let result = async {
        let mut staged = HashMap::new();
        if let Source::Download(n) = want.source {
            tokio::fs::create_dir_all(&staging).await?;
            let target = staging.join(n.to_string());
            fetch_index_file(state, &index.files[n], &target).await?;
            staged.insert(n, target);
        }
        place(folder, &kept_pack(state, id), path, &want, &staged).await
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&staging).await;
    result.map(|()| (want.sha1, want.size))
}

/// What a pack install does to a server's folder. Installs and updates both read the pack the
/// server keeps and run through `repair_content`, which needs no network for these packs: their
/// downloads are client only or already on disk.
#[cfg(test)]
mod server_pack_tests {
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Arc;

    use contracts::Side;

    use crate::Engine;
    use crate::api::instance::{
        ContentSide, repair_content, set_content_side, toggle_disable_project,
    };
    use crate::state::instances::adapters::sqlite::content_rows;
    use crate::state::tests::{add_instance, config};

    const ID: &str = "s";

    struct Server {
        _tmp: tempfile::TempDir,
        state: Arc<Engine>,
        dir: PathBuf,
    }

    async fn server() -> Server {
        crate::install_crypto_provider();
        let tmp = tempfile::tempdir().unwrap();
        let state = Engine::new(crate::state::EngineConfig {
            side: Side::Server,
            ..config(tmp.path().to_path_buf())
        })
        .await
        .unwrap();
        add_instance(&state, ID, "srv").await;
        let dir = state.directories.instances_dir().join("srv");
        std::fs::create_dir_all(&dir).unwrap();
        Server {
            _tmp: tmp,
            state,
            dir,
        }
    }

    /// A jar whose only metadata is `fabric.mod.json` with `environment`.
    fn fabric_jar(environment: &str) -> Vec<u8> {
        let mut jar = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        jar.start_file(
            "fabric.mod.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        write!(
            jar,
            r#"{{"id":"m","version":"1","environment":"{environment}"}}"#
        )
        .unwrap();
        jar.finish().unwrap().into_inner()
    }

    impl Server {
        /// Make the pack with index `files` and archive `entries` the one the server follows,
        /// and install it.
        async fn install(&self, files: &str, entries: &[(&str, &[u8])]) {
            let mut writer =
                zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("modrinth.index.json", options).unwrap();
            write!(
                writer,
                r#"{{"formatVersion":1,"game":"minecraft","versionId":"1","name":"p","dependencies":{{"minecraft":"1.21.1","fabric-loader":"0.16.0"}},"files":[{files}]}}"#
            )
            .unwrap();
            for (name, bytes) in entries {
                writer.start_file(*name, options).unwrap();
                writer.write_all(bytes).unwrap();
            }
            let kept = self
                .state
                .directories
                .config_dir
                .join("packs")
                .join(format!("{ID}.mrpack"));
            std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
            std::fs::write(kept, writer.finish().unwrap().into_inner())
                .unwrap();
            repair_content(&self.state, ID).await.unwrap();
        }

        fn read(&self, path: &str) -> Option<String> {
            std::fs::read_to_string(self.dir.join(path)).ok()
        }

        fn write(&self, path: &str, text: &str) {
            let path = self.dir.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }

        /// The paths the engine records, and whether each is a client-only file of the pack.
        async fn rows(&self) -> Vec<(String, bool)> {
            let mut rows: Vec<_> =
                content_rows::get_instance_files(ID, &self.state.pool)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|file| (file.relative_path, file.missing))
                    .collect();
            rows.sort();
            rows
        }
    }

    fn rows(rows: &[(&str, bool)]) -> Vec<(String, bool)> {
        rows.iter()
            .map(|(path, missing)| ((*path).to_owned(), *missing))
            .collect()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_server_takes_overrides_and_its_own_and_leaves_client_files_out()
    {
        let server = server().await;
        server
            .install(
                r#"{"path":"mods/sodium.jar","hashes":{"sha1":"c1"},"env":{"client":"required","server":"unsupported"},"downloads":["https://cdn.modrinth.com/data/P/versions/V/sodium.jar"],"fileSize":1}"#,
                &[
                    ("overrides/config/a.toml", b"a"),
                    ("overrides/config/b.toml", b"everyone"),
                    ("server-overrides/config/b.toml", b"server"),
                    ("client-overrides/config/c.toml", b"client"),
                    ("overrides/mods/both.jar", &fabric_jar("*")),
                    ("overrides/mods/menu.jar", &fabric_jar("client")),
                ],
            )
            .await;

        assert_eq!(server.read("config/a.toml").as_deref(), Some("a"));
        assert_eq!(server.read("config/b.toml").as_deref(), Some("server"));
        assert_eq!(server.read("config/c.toml"), None);
        assert!(server.dir.join("mods/both.jar").exists());
        assert!(!server.dir.join("mods/menu.jar").exists());
        assert!(!server.dir.join("mods/sodium.jar").exists());
        assert_eq!(
            server.rows().await,
            rows(&[
                ("config/a.toml", false),
                ("config/b.toml", false),
                ("mods/both.jar", false),
                ("mods/menu.jar", true),
                ("mods/sodium.jar", true),
            ])
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_pack_never_replaces_what_the_server_owns() {
        let server = server().await;
        server.write("server.properties", "motd=mine");
        server.write("world/level.dat", "mine");
        server
            .install(
                "",
                &[
                    ("overrides/server.properties", b"motd=pack"),
                    ("overrides/world/level.dat", b"pack"),
                    ("overrides/ops.json", b"[]"),
                ],
            )
            .await;

        assert_eq!(
            server.read("server.properties").as_deref(),
            Some("motd=mine")
        );
        assert_eq!(server.read("world/level.dat").as_deref(), Some("mine"));
        assert_eq!(server.read("ops.json").as_deref(), Some("[]"));
        assert_eq!(server.rows().await, rows(&[]), "they are the server's");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_index_file_already_on_disk_is_recorded_without_a_download() {
        let server = server().await;
        server.write("config/big.toml", "already");
        let sha1 = sha1_smol::Sha1::from(b"already").hexdigest();
        server
            .install(
                &format!(
                    r#"{{"path":"config/big.toml","hashes":{{"sha1":"{sha1}"}},"downloads":["https://cdn.modrinth.com/data/P/versions/V/big.toml"],"fileSize":7}}"#
                ),
                &[],
            )
            .await;

        assert_eq!(server.read("config/big.toml").as_deref(), Some("already"));
        assert_eq!(server.rows().await, rows(&[("config/big.toml", false)]));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_update_replaces_the_packs_files_and_leaves_the_owners() {
        let server = server().await;
        server
            .install(
                "",
                &[
                    ("overrides/config/changed.toml", b"one"),
                    ("overrides/config/same.toml", b"same"),
                    ("overrides/config/dropped.toml", b"old"),
                ],
            )
            .await;
        server.write("config/mine.toml", "mine");

        server
            .install(
                "",
                &[
                    ("overrides/config/changed.toml", b"two"),
                    ("overrides/config/same.toml", b"same"),
                    ("overrides/config/new.toml", b"new"),
                ],
            )
            .await;

        assert_eq!(server.read("config/changed.toml").as_deref(), Some("two"));
        assert_eq!(server.read("config/same.toml").as_deref(), Some("same"));
        assert_eq!(server.read("config/new.toml").as_deref(), Some("new"));
        assert_eq!(server.read("config/dropped.toml"), None);
        assert_eq!(server.read("config/mine.toml").as_deref(), Some("mine"));
        assert_eq!(
            server.rows().await,
            rows(&[
                ("config/changed.toml", false),
                ("config/new.toml", false),
                ("config/same.toml", false),
            ])
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_client_only_file_turned_on_for_the_server_stays_on_through_updates()
     {
        let server = server().await;
        let menu = fabric_jar("client");
        let pack: [(&str, &[u8]); 1] = [("overrides/mods/menu.jar", &menu)];
        server.install("", &pack).await;
        assert!(!server.dir.join("mods/menu.jar").exists());

        let set_server_side = |enabled| {
            set_content_side(
                &server.state,
                ID,
                "mods/menu.jar",
                ContentSide::Server,
                enabled,
            )
        };
        set_server_side(true).await.unwrap();
        assert!(server.dir.join("mods/menu.jar").exists());
        server.install("", &pack).await;
        assert!(server.dir.join("mods/menu.jar").exists());
        assert_eq!(server.rows().await, rows(&[("mods/menu.jar", false)]));

        set_server_side(false).await.unwrap();
        assert!(!server.dir.join("mods/menu.jar").exists());
        assert_eq!(server.rows().await, rows(&[("mods/menu.jar", true)]));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_update_overwrites_what_the_owner_edited_and_keeps_what_it_dropped()
     {
        let server = server().await;
        server
            .install(
                "",
                &[
                    ("overrides/config/edited.toml", b"one"),
                    ("overrides/config/deleted.toml", b"one"),
                    ("overrides/config/dropped.toml", b"one"),
                ],
            )
            .await;
        server.write("config/edited.toml", "mine");
        server.write("config/dropped.toml", "mine");
        std::fs::remove_file(server.dir.join("config/deleted.toml")).unwrap();

        server
            .install(
                "",
                &[
                    ("overrides/config/edited.toml", b"two"),
                    ("overrides/config/deleted.toml", b"one"),
                ],
            )
            .await;

        assert_eq!(server.read("config/edited.toml").as_deref(), Some("two"));
        assert_eq!(server.read("config/deleted.toml").as_deref(), Some("one"));
        assert_eq!(server.read("config/dropped.toml").as_deref(), Some("mine"));
        let dropped = content_rows::tracked_file(
            &server.state.pool,
            ID,
            "config/dropped.toml",
        )
        .await
        .unwrap()
        .unwrap()
        .0;
        assert!(
            !dropped.by_pack(),
            "an edit the pack dropped is the owner's"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_update_drops_a_file_the_owner_turned_off_unless_they_edited_it()
    {
        let server = server().await;
        let jar = fabric_jar("*");
        server
            .install(
                "",
                &[
                    ("overrides/mods/a.jar", &jar),
                    ("overrides/mods/b.jar", &jar),
                ],
            )
            .await;
        for name in ["mods/a.jar", "mods/b.jar"] {
            std::fs::rename(
                server.dir.join(name),
                server.dir.join(format!("{name}.disabled")),
            )
            .unwrap();
        }
        server.write("mods/b.jar.disabled", "mine");

        server.install("", &[]).await;

        assert!(!server.dir.join("mods/a.jar.disabled").exists());
        assert_eq!(server.read("mods/b.jar.disabled").as_deref(), Some("mine"));
        let pool = &server.state.pool;
        let a = content_rows::tracked_file(pool, ID, "mods/a.jar").await;
        assert!(a.unwrap().is_none());
        let b = content_rows::tracked_file(pool, ID, "mods/b.jar").await;
        assert!(
            !b.unwrap().unwrap().0.by_pack(),
            "an edit the pack dropped is the owner's"
        );
    }

    /// Back the server's mods up as the install runner does, let an install write half a jar
    /// and fail, and roll it back. The mods must be as they were.
    async fn fail_an_install(server: &Server) {
        use crate::api::pack::install_from::CreatePackLocation;
        use crate::install::model::{InstallJobState, InstallRollbackState};
        use crate::install::{InstallRequest, recovery};

        let state = &server.state;
        let mods = server.dir.join("mods");
        let read = |path: std::fs::DirEntry| {
            (path.file_name(), std::fs::read(path.path()).unwrap())
        };
        let mut was: Vec<_> = std::fs::read_dir(&mods)
            .unwrap()
            .map(|entry| read(entry.unwrap()))
            .collect();
        let metadata = super::instance_metadata(state, ID).await.unwrap();
        let request = InstallRequest::InstallPackToExistingInstance {
            instance_id: ID.to_owned(),
            location: CreatePackLocation::FromFile {
                path: "broken.mrpack".into(),
            },
            post_install_edit: None,
        };
        let mut job = InstallJobState::new(&request);
        job.paths.staging_dir = Some(
            recovery::prepare_instance_update_backup(
                uuid::Uuid::new_v4(),
                &metadata,
                Some(vec!["mods".to_owned()]),
                state,
            )
            .await
            .unwrap(),
        );
        job.rollback = Some(InstallRollbackState {
            install_stage: metadata.instance.install_stage,
            instance: metadata,
        });
        server.write("mods/broken.jar", "half");

        recovery::apply_cleanup(&job, state).await.unwrap();

        let mut is: Vec<_> = std::fs::read_dir(&mods)
            .unwrap()
            .map(|entry| read(entry.unwrap()))
            .collect();
        was.sort();
        is.sort();
        assert_eq!(is, was);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_failed_install_rolls_back_over_a_turned_off_edit_the_pack_dropped()
     {
        let server = server().await;
        let state = &server.state;
        let jar = fabric_jar("*");
        server.install("", &[("overrides/mods/a.jar", &jar)]).await;
        toggle_disable_project(state, ID, "mods/a.jar", Some(false))
            .await
            .unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(server.dir.join("mods/a.jar.disabled"))
            .unwrap()
            .write_all(b"mine")
            .unwrap();
        server.install("", &[]).await;

        fail_an_install(&server).await;

        toggle_disable_project(state, ID, "mods/a.jar", Some(true))
            .await
            .unwrap();
        assert!(server.dir.join("mods/a.jar").exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_failed_install_rolls_back_over_a_turned_off_file_the_pack_updated()
     {
        let server = server().await;
        let state = &server.state;
        server
            .install("", &[("overrides/mods/a.jar", &fabric_jar("*"))])
            .await;
        toggle_disable_project(state, ID, "mods/a.jar", Some(false))
            .await
            .unwrap();
        server
            .install("", &[("overrides/mods/a.jar", &fabric_jar("server"))])
            .await;

        fail_an_install(&server).await;

        toggle_disable_project(state, ID, "mods/a.jar", Some(true))
            .await
            .unwrap();
        assert!(server.dir.join("mods/a.jar").exists());
    }
}

#[cfg(test)]
mod server_plan_tests {
    use std::io::Write;

    use super::*;

    /// The source kind of the pack the tests install.
    const KIND: ContentSourceKind = ContentSourceKind::ImportedModpack;

    #[test]
    fn a_server_takes_a_file_unless_the_first_that_knows_rules_it_out() {
        use SideType::{Optional, Unsupported};
        // The index decides when it says.
        assert!(!server_takes(
            Some(Unsupported),
            Some(VersionEnvironment::ClientAndServer),
            false
        ));
        assert!(server_takes(
            Some(Optional),
            Some(VersionEnvironment::ClientOnly),
            true
        ));
        // Then Modrinth, then the jar.
        assert!(!server_takes(
            None,
            Some(VersionEnvironment::ClientOnly),
            false
        ));
        assert!(server_takes(
            None,
            Some(VersionEnvironment::ClientAndServer),
            true
        ));
        assert!(!server_takes(None, None, true));
        // A Forge jar names no sides, and nothing else knows: taken.
        assert!(server_takes(None, None, false));
    }

    fn row(
        path: &str,
        source_kind: ContentSourceKind,
        sha1: &str,
    ) -> TrackedFile {
        TrackedFile {
            relative_path: path.into(),
            source_kind,
            sha1: sha1.into(),
            size: 1,
            project_id: None,
            version_id: None,
        }
    }

    fn want(sha1: &str) -> Wanted {
        Wanted {
            sha1: sha1.into(),
            source: Source::Download(0),
            size: 1,
        }
    }

    fn run(
        rows: &[TrackedFile],
        wanted: &[(&str, &str)],
        on_disk: &[(&str, &str)],
    ) -> Vec<Step> {
        run_on(rows, wanted, on_disk, false)
    }

    /// As [`run`], on a file system that compares paths without case when `case_insensitive`.
    fn run_on(
        rows: &[TrackedFile],
        wanted: &[(&str, &str)],
        on_disk: &[(&str, &str)],
        case_insensitive: bool,
    ) -> Vec<Step> {
        let wanted = wanted
            .iter()
            .map(|(path, sha1)| ((*path).to_owned(), want(sha1)))
            .collect();
        let on_disk: HashMap<String, String> = on_disk
            .iter()
            .map(|(path, sha1)| {
                (fold(path, case_insensitive), (*sha1).to_owned())
            })
            .collect();
        plan(
            rows,
            &wanted,
            &|path| on_disk.get(&fold(path, case_insensitive)).cloned(),
            "world",
            case_insensitive,
            KIND,
        )
    }

    /// The pack's file placed at `path`.
    fn placed(path: &str, sha1: &str) -> Step {
        Step::Place {
            path: path.into(),
            wanted: want(sha1),
            row: Some(row(path, KIND, sha1)),
            disabled: false,
        }
    }

    #[test]
    fn a_first_install_places_everything_and_records_it_as_the_packs() {
        let steps =
            run(&[], &[("config/a.toml", "a1"), ("mods/m.jar", "m1")], &[]);
        assert_eq!(
            steps,
            [placed("config/a.toml", "a1"), placed("mods/m.jar", "m1")]
        );
    }

    #[test]
    fn an_update_overwrites_the_packs_files_edited_or_not() {
        let rows = [
            row("config/edited.toml", KIND, "e1"),
            row("config/same.toml", KIND, "s1"),
            row("mods/deleted.jar", KIND, "d1"),
            row("mods/kept.jar", KIND, "k1"),
            row("mods/mine.jar", ContentSourceKind::Local, "x1"),
        ];
        let steps = run(
            &rows,
            &[
                ("config/edited.toml", "e2"),
                ("config/same.toml", "s2"),
                ("mods/deleted.jar", "d1"),
                ("mods/kept.jar", "k1"),
                ("mods/new.jar", "n1"),
            ],
            &[
                ("config/edited.toml", "owner"),
                ("config/same.toml", "s1"),
                ("mods/kept.jar", "k1"),
                ("mods/mine.jar", "x1"),
            ],
        );
        assert_eq!(
            steps,
            [
                placed("config/edited.toml", "e2"),
                placed("config/same.toml", "s2"),
                placed("mods/deleted.jar", "d1"),
                Step::Record(rows[3].clone()),
                placed("mods/new.jar", "n1"),
            ]
        );
    }

    #[test]
    fn a_file_nobody_recorded_in_the_packs_way_is_replaced() {
        assert_eq!(
            run(
                &[],
                &[("config/a.toml", "a1")],
                &[("config/a.toml", "owner")]
            ),
            [placed("config/a.toml", "a1")]
        );
        assert_eq!(
            run(&[], &[("config/a.toml", "a1")], &[("config/a.toml", "a1")]),
            [Step::Record(row("config/a.toml", KIND, "a1"))]
        );
    }

    #[test]
    fn the_pack_drops_only_its_own_files() {
        let rows = [
            row("config/gone.toml", KIND, "g1"),
            row("mods/mine.jar", ContentSourceKind::Local, "x1"),
        ];
        assert_eq!(
            run(
                &rows,
                &[],
                &[("config/gone.toml", "g1"), ("mods/mine.jar", "x1")]
            ),
            [Step::Drop(rows[0].clone())]
        );
    }

    #[test]
    fn server_owned_files_are_placed_only_where_none_exists() {
        let steps = run(
            &[],
            &[
                ("server.properties", "p1"),
                ("ops.json", "o1"),
                ("world/level.dat", "w1"),
            ],
            &[("server.properties", "mine"), ("world/level.dat", "mine")],
        );
        assert_eq!(
            steps,
            [Step::Place {
                path: "ops.json".into(),
                wanted: want("o1"),
                row: None,
                disabled: false,
            }]
        );
    }

    #[test]
    fn a_pack_file_the_owner_disabled_is_updated_and_stays_disabled() {
        let rows = [row("mods/m.jar", KIND, "m1")];
        let disabled = [("mods/m.jar.disabled", "m1")];
        assert_eq!(
            run(&rows, &[("mods/m.jar", "m1")], &disabled),
            [Step::Record(rows[0].clone())]
        );
        assert_eq!(
            run(&rows, &[("mods/m.jar", "m2")], &disabled),
            [Step::Place {
                path: "mods/m.jar".into(),
                wanted: want("m2"),
                row: Some(row("mods/m.jar", KIND, "m2")),
                disabled: true,
            }]
        );
    }

    #[test]
    fn a_pack_that_respells_a_file_moves_its_row_without_deleting_it() {
        let rows = [row("Config/A.toml", KIND, "a1")];
        // One file on Windows and macOS: the row follows the new spelling.
        assert_eq!(
            run_on(
                &rows,
                &[("config/a.toml", "a1")],
                &[("Config/A.toml", "a1")],
                true
            ),
            [
                Step::Forget("Config/A.toml".into()),
                Step::Record(row("config/a.toml", KIND, "a1")),
            ]
        );
        // Two files on Linux: the old one goes, the new one is placed.
        assert_eq!(
            run_on(
                &rows,
                &[("config/a.toml", "a1")],
                &[("Config/A.toml", "a1")],
                false
            ),
            [Step::Drop(rows[0].clone()), placed("config/a.toml", "a1")]
        );
    }

    fn archive(
        index: &str,
        entries: &[(&str, &[u8])],
    ) -> zip::ZipArchive<Cursor<Vec<u8>>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("modrinth.index.json", options).unwrap();
        writer.write_all(index.as_bytes()).unwrap();
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        zip::ZipArchive::new(writer.finish().unwrap()).unwrap()
    }

    #[test]
    fn only_minecraft_packs_of_format_1_are_read() {
        let read = |index: &str| read_server_index(&mut archive(index, &[]));
        assert!(read(r#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"P","files":[],"dependencies":{}}"#).is_ok());
        assert!(read(r#"{"formatVersion":2,"game":"minecraft","versionId":"1","name":"P","files":[],"dependencies":{}}"#).is_err());
        assert!(read("{").is_err());
    }

    #[test]
    fn two_spellings_of_one_file_refuse_the_pack_where_case_does_not_count() {
        let mut archive = archive(
            r#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"p","files":[],"dependencies":{}}"#,
            &[
                ("overrides/config/A.toml", b"x"),
                ("overrides/config/a.toml", b"x"),
            ],
        );
        let index = read_server_index(&mut archive).unwrap();
        let candidates = server_candidates(
            &index,
            &mut archive,
            ModLoader::Fabric,
            u64::MAX,
        )
        .unwrap();
        assert!(matches!(
            server_split(
                candidates.clone(),
                &Known::new(),
                &ServerOn::default(),
                true
            ),
            Err(PackError::Collision(..))
        ));
        let both = server_split(
            candidates,
            &Known::new(),
            &ServerOn::default(),
            false,
        )
        .unwrap()
        .wanted;
        assert_eq!(both.len(), 2);
        assert_eq!(both["config/a.toml"].size, 1);
    }

    /// A jar whose only metadata is `fabric.mod.json` with `environment`.
    fn fabric_jar(environment: &str) -> Vec<u8> {
        let mut jar = zip::ZipWriter::new(Cursor::new(Vec::new()));
        jar.start_file(
            "fabric.mod.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        write!(
            jar,
            r#"{{"id":"m","version":"1","environment":"{environment}"}}"#
        )
        .unwrap();
        jar.finish().unwrap().into_inner()
    }

    #[test]
    fn a_server_leaves_out_what_the_index_modrinth_or_the_jar_says_is_client_only()
     {
        let mut archive = archive(
            r#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"p",
            "dependencies":{"minecraft":"1.21.1","fabric-loader":"0.16.0"},
            "files":[
            {"path":"mods/client.jar","hashes":{"sha1":"c1"},"env":{"client":"required","server":"unsupported"},"downloads":["https://cdn.modrinth.com/data/P/versions/V/client.jar"],"fileSize":1},
            {"path":"mods/asked.jar","hashes":{"sha1":"a1"},"downloads":["https://cdn.modrinth.com/data/Q/versions/W/asked.jar"],"fileSize":1}]}"#,
            &[
                ("overrides/mods/menu.jar", &fabric_jar("client")),
                ("overrides/mods/both.jar", &fabric_jar("*")),
                ("overrides/mods/forge.jar", b"not a jar"),
                ("server-overrides/mods/menu.jar", &fabric_jar("client")),
                ("overrides/resourcepacks/look.zip", b"x"),
            ],
        );
        let index = read_server_index(&mut archive).unwrap();
        let candidates = server_candidates(
            &index,
            &mut archive,
            ModLoader::Fabric,
            u64::MAX,
        )
        .unwrap();
        let known = Known::from([(
            "a1".to_owned(),
            KnownFile {
                project_id: "Q".into(),
                version_id: "W".into(),
                environment: Some(VersionEnvironment::ClientOnly),
            },
        )]);
        let by_path = server_split(
            candidates.clone(),
            &known,
            &ServerOn {
                paths: HashSet::from(["mods/client.jar".to_owned()]),
                ..ServerOn::default()
            },
            false,
        )
        .unwrap();
        assert!(
            by_path.wanted.contains_key("mods/client.jar"),
            "a client-only file someone turned on is the server's"
        );
        assert_eq!(
            by_path.turned_on,
            HashSet::from(["mods/client.jar".to_owned()])
        );
        // An update names a new version's jar for it: the project it is a file of stays on.
        let by_project = server_split(
            candidates.clone(),
            &known,
            &ServerOn {
                paths: HashSet::from(["mods/client-0.9.jar".to_owned()]),
                projects: HashSet::from(["P".to_owned()]),
            },
            false,
        )
        .unwrap();
        assert!(by_project.turned_on.contains("mods/client.jar"));
        let Split {
            wanted,
            skipped,
            turned_on,
        } = server_split(candidates, &known, &ServerOn::default(), false)
            .unwrap();
        assert!(turned_on.is_empty());
        assert_eq!(
            wanted.keys().collect::<Vec<_>>(),
            [
                "mods/both.jar",
                "mods/forge.jar",
                // `server-overrides/` is the server's, whatever the jar says.
                "mods/menu.jar",
                "resourcepacks/look.zip",
            ]
        );
        let skipped: Vec<_> = skipped
            .iter()
            .map(|file| {
                (
                    file.relative_path.as_str(),
                    file.project_id.as_deref(),
                    file.version_id.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            skipped,
            [
                ("mods/client.jar", Some("P"), Some("V")),
                ("mods/asked.jar", Some("Q"), Some("W")),
            ]
        );
    }

    #[test]
    fn the_whole_install_must_fit_on_the_disk() {
        let place = |size, source| Step::Place {
            path: "mods/m.jar".into(),
            wanted: Wanted {
                size,
                source,
                ..want("m1")
            },
            row: None,
            disabled: false,
        };
        let steps = [
            place(10, Source::Override(0)),
            place(10, Source::Download(0)),
        ];
        // The download is staged before it is placed.
        assert_eq!(check_room(&steps, 30).unwrap(), 30);
        // Each file fits on its own; together they do not.
        assert!(matches!(check_room(&steps, 29), Err(PackError::NoRoom(_))));
    }

    #[test]
    fn cdn_urls_name_their_project_and_version() {
        assert_eq!(
            ids_from_cdn(
                "https://cdn.modrinth.com/data/AANobbMI/versions/u6dRKJwZ/sodium.jar"
            ),
            Some(("AANobbMI".into(), "u6dRKJwZ".into()))
        );
        assert_eq!(ids_from_cdn("https://github.com/a/b/releases/x.jar"), None);
        assert_eq!(
            ids_from_cdn("https://cdn.modrinth.com/data/AANobbMI/icon.png"),
            None
        );
    }
}
