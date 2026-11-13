//! Packs on a server engine. A pack is an index of files to download plus `overrides/` and
//! `server-overrides/` to unpack. Installing or updating one is planned as a whole first
//! ([`plan`]): every file the pack wants is placed, kept, or, when the owner edited it since the
//! pack put it there, left alone and marked pending until the owner says keep or replace. Which
//! files a server takes is [`split`]'s rule; the rest are client only and never placed.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::{Read, Seek},
};

use bedrock::content::{Env, SideType, ids_from_cdn};

use crate::server_jar::{
    self, Environment as JarEnvironment, Platform as JarPlatform,
};
use crate::state::{
    ContentRequirement, ContentSourceKind, PendingChange as Pending,
    ProjectType, VersionEnvironment,
};
use crate::util::{
    archive::{self, ArchiveError, NoRoom},
    pack_hosts,
    sandbox::{self, PathError},
};

/// A file a server engine tracks, by its enabled path: an `instance_files` row and the entry that
/// says where it came from. This is what installs and the pack planner read and write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedFile {
    pub relative_path: String,
    /// `Local` for a file someone on the server added, or the source kind of the pack's set.
    pub source_kind: ContentSourceKind,
    /// As the engine placed it.
    pub sha1: String,
    /// As the engine placed it; 0 for a file it found there rather than placed.
    pub size: u64,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub pending: Option<Pending>,
}

impl TrackedFile {
    /// Whether the pack the server follows put it there.
    pub fn by_pack(&self) -> bool {
        self.source_kind != ContentSourceKind::Local
    }
}

/// A file of the pack the server leaves out because it is client only. It is never on the
/// server's disk; the content list shows it as not installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedFile {
    pub relative_path: String,
    pub sha1: String,
    pub size: u64,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
}

/// Which sides a tracked file is on for, from its entry. A file the engine does not track is on
/// for both, the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntrySides {
    /// Off for both sides: the file is `.disabled` on disk.
    pub enabled: bool,
    /// `Unsupported`: the file is `.disabled` on disk, and players still get it.
    pub server_requirement: ContentRequirement,
    /// `Unsupported`: the server runs it, and its share leaves it out.
    pub client_requirement: ContentRequirement,
    /// Someone set the sides by hand.
    pub unlocked: bool,
    /// A client-only file of the pack that someone turned on for the server, so it was placed.
    /// Pack updates keep it; turning the server side off makes it a [`SkippedFile`] again.
    pub server_override: bool,
}

impl Default for EntrySides {
    fn default() -> Self {
        Self {
            enabled: true,
            server_requirement: ContentRequirement::Required,
            client_requirement: ContentRequirement::Required,
            unlocked: false,
            server_override: false,
        }
    }
}

/// The project type of the file at `relative_path` on a server, from its folder: directly inside
/// `mods/`, `plugins/`, `resourcepacks/`, `shaderpacks/` or `<level>/datapacks/`. `None` for
/// anything else, such as a config.
pub fn project_type_of(relative_path: &str) -> Option<ProjectType> {
    let (folder, name) = relative_path.rsplit_once('/')?;
    if name.is_empty() {
        return None;
    }
    if folder.matches('/').count() == 1 {
        return folder
            .ends_with("/datapacks")
            .then_some(ProjectType::DataPack);
    }
    match ProjectType::from_name(folder)? {
        ProjectType::DataPack => None,
        project_type => Some(project_type),
    }
}

/// The pack `env` of a version that runs in `environment`. Upstream's rule
/// (`api/instance/export_mrpack.rs`): a version runs on both sides unless Modrinth says it is for
/// players only or for dedicated servers only.
pub fn mrpack_env(environment: Option<VersionEnvironment>) -> Env {
    use VersionEnvironment::*;
    let (client, server) = match environment.unwrap_or(Unknown) {
        ClientAndServer
        | ClientOnlyServerOptional
        | ServerOnly
        | ServerOnlyClientOptional
        | ClientOrServer
        | ClientOrServerPrefersBoth
        | Unknown => (SideType::Required, SideType::Required),
        ClientOnly | SingleplayerOnly => {
            (SideType::Required, SideType::Unsupported)
        }
        DedicatedServerOnly => (SideType::Unsupported, SideType::Required),
    };
    Env { client, server }
}

/// Whether a server takes a file of a pack, by the first of these that knows:
///
/// 1. `env`, the pack index's word: a server leaves out a file whose server side is
///    unsupported. A file of `overrides/` has none.
/// 2. `modrinth`, where Modrinth says the version with the file's hash runs, through
///    [`mrpack_env`].
/// 3. `jar`, the sides a jar Modrinth does not know names in its Fabric or Quilt metadata.
///
/// A file none of them rules out is taken: guessing a server-side mod is client only breaks the
/// server, while an extra client mod on a server only costs a warning in its log.
pub fn server_takes(
    env: Option<&Env>,
    modrinth: Option<VersionEnvironment>,
    jar: Option<JarEnvironment>,
) -> bool {
    if let Some(env) = env {
        return env.server != SideType::Unsupported;
    }
    if modrinth.is_some() {
        return mrpack_env(modrinth).server != SideType::Unsupported;
    }
    jar != Some(JarEnvironment::ClientOnly)
}

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

/// `modrinth.index.json`, as bedrock models it for both sides.
pub use bedrock::content::{Index, IndexFile};

/// What Modrinth knows about a file of a pack, found by its hash.
#[derive(Debug, Clone)]
pub struct KnownFile {
    pub project_id: String,
    pub version_id: String,
    pub environment: Option<VersionEnvironment>,
}

/// Read and check a pack's index.
pub fn read_index<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Index, PackError> {
    bedrock::content::read_index(archive).map_err(
        |bedrock::content::IndexError::NotAPack(reason)| {
            PackError::NotAPack(reason)
        },
    )
}

/// The loader and versions a pack runs on, as bedrock reads them.
pub use bedrock::content::PackPlatform as Platform;

/// The platform a pack runs on; one that names no Minecraft version is not a pack.
pub fn platform(index: &Index) -> Result<Platform, PackError> {
    bedrock::content::platform(index).ok_or_else(|| {
        PackError::NotAPack("it names no Minecraft version".into())
    })
}

/// How Minecraft `version` orders against `than` when both are releases (`1.20.1`, `26.3`),
/// by their numbers. `None` when either is not: a snapshot (`24w14a`), a pre-release or a
/// release candidate (`1.21-pre1`, `1.21-rc1`). Those cannot be strictly ordered here, and a
/// pre-release of 1.21 is older than 1.21 itself, so a caller that guards against going back a
/// version treats `None` as possibly older.
pub fn release_order(version: &str, than: &str) -> Option<std::cmp::Ordering> {
    let numbers = |version: &str| -> Option<Vec<u32>> {
        let mut numbers = version
            .split('.')
            .map(|part| {
                part.bytes()
                    .all(|byte| byte.is_ascii_digit())
                    .then(|| part.parse().ok())
                    .flatten()
            })
            .collect::<Option<Vec<u32>>>()?;
        numbers.resize(3.max(numbers.len()), 0);
        Some(numbers)
    };
    Some(numbers(version)?.cmp(&numbers(than)?))
}

/// Where the pack gets a file from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// `files[n]` of the index.
    Download(usize),
    /// Entry `n` of the pack archive.
    Override(usize),
}

/// The pack files someone turned on for the server although they are client only: by path, and
/// by Modrinth project for the ones that have one, so a pack update that renames a file (a new
/// version's jar is usually named for it) keeps it on.
#[derive(Debug, Clone, Default)]
pub struct ServerOn {
    pub paths: HashSet<String>,
    pub projects: HashSet<String>,
}

/// What [`split`] makes of a pack's files.
#[derive(Debug, Default)]
pub struct Split {
    /// The files a server takes, by path.
    pub wanted: BTreeMap<String, Wanted>,
    /// The client-only files it leaves out.
    pub skipped: Vec<SkippedFile>,
    /// The paths of `wanted` taken only because someone turned them on ([`ServerOn`]).
    pub turned_on: HashSet<String>,
}

/// A file the pack wants at a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    pub sha1: String,
    pub source: Source,
    /// Its size in bytes, as the index lists it or the archive holds it.
    pub size: u64,
}

/// A path as the file system compares it: folded when it treats `Config/a.toml` and
/// `config/a.toml` as one file (see `Folder::case_insensitive`), exact otherwise.
pub fn fold(path: &str, case_insensitive: bool) -> String {
    if case_insensitive {
        sandbox::fold_case(path)
    } else {
        path.to_owned()
    }
}

/// A file of a pack, before it is known whether the server takes it.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub path: String,
    pub wanted: Wanted,
    /// The index's word on its sides; `server-overrides/` are the server's.
    pub env: Option<Env>,
    /// For a jar of `overrides/`: the sides its own metadata names.
    pub jar: Option<JarEnvironment>,
    /// For an index file, its download URLs, which may name its Modrinth project and version.
    pub downloads: Vec<String>,
}

impl Candidate {
    /// Whether Modrinth is asked where the file runs: the index does not say, for a file it
    /// downloads or a jar of `overrides/`.
    pub fn ask_modrinth(&self) -> bool {
        self.env.is_none()
            && (matches!(self.wanted.source, Source::Download(_))
                || self.path.ends_with(".jar"))
    }
}

/// Every file of a pack, with what the index and, for jars of `overrides/`, their metadata say
/// about their sides. `loader` is the pack's, whose jar metadata is read first. Every override is
/// inflated to hash it, all of them drawing on `room` (see `archive::room`): a pack whose
/// overrides do not fit on the disk could not be installed anyway.
pub fn candidates<R: Read + Seek>(
    index: &Index,
    archive: &mut zip::ZipArchive<R>,
    loader: Option<JarPlatform>,
    mut room: u64,
) -> Result<Vec<Candidate>, PackError> {
    let mut candidates = Vec::new();
    for (n, file) in index.files.iter().enumerate() {
        candidates.push(Candidate {
            path: sandbox::clean(&file.path)?,
            wanted: Wanted {
                sha1: file.hashes.sha1.to_ascii_lowercase(),
                source: Source::Download(n),
                size: file.file_size,
            },
            env: file.env,
            jar: None,
            downloads: file.downloads.clone(),
        });
    }
    let entries = archive::entries(archive)?;
    let server_side = Env {
        client: SideType::Unsupported,
        server: SideType::Required,
    };
    for (prefix, env) in [
        ("overrides/", None),
        ("server-overrides/", Some(server_side)),
    ] {
        for (n, (entry, is_dir)) in entries.iter().enumerate() {
            let Some(path) = entry.strip_prefix(prefix).filter(|_| !is_dir)
            else {
                continue;
            };
            let (sha1, size) =
                archive::entry_sha1(archive, n, entry, &mut room)?;
            let jar = if env.is_none() && path.ends_with(".jar") {
                jar_environment(archive, n, size, loader)
            } else {
                None
            };
            candidates.push(Candidate {
                path: path.to_owned(),
                wanted: Wanted {
                    sha1,
                    source: Source::Override(n),
                    size,
                },
                env,
                jar,
                downloads: Vec::new(),
            });
        }
    }
    Ok(candidates)
}

/// The largest jar of `overrides/` whose metadata is read. A jar is read into memory whole to
/// inspect it; one past this is not inspected, and is taken unless Modrinth knows it.
const MAX_INSPECTED_JAR: u64 = 128 * 1024 * 1024;

/// The sides the jar at entry `n` of the pack names in its metadata. Best effort: `None` for a
/// jar that cannot be read or is too large to read ([`MAX_INSPECTED_JAR`]).
fn jar_environment<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    n: usize,
    size: u64,
    loader: Option<JarPlatform>,
) -> Option<JarEnvironment> {
    if size > MAX_INSPECTED_JAR {
        return None;
    }
    let mut bytes = Vec::with_capacity(usize::try_from(size).ok()?);
    archive
        .by_index(n)
        .ok()?
        .take(size)
        .read_to_end(&mut bytes)
        .ok()?;
    let mut jar = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    server_jar::inspect(&mut jar, loader)?.manifest.environment
}

/// What Modrinth knows about the files that ask, by SHA-1.
pub type Known = HashMap<String, KnownFile>;

/// The files a server takes, by path, and the ones it leaves out because they are client only,
/// by [`server_takes`]: the index first, then Modrinth by hash, then a jar's own
/// metadata. `server-overrides/` wins over `overrides/`. A client-only file in `server_on`, by
/// path or Modrinth project, is taken anyway: someone turned it on for the server. A download
/// the server takes must come from an allowed host.
pub fn split(
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
            candidate.env.as_ref(),
            modrinth.and_then(|file| file.environment),
            candidate.jar,
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

/// One thing an install does to a path.
// A plan is made once per install, so boxing the large variant saves nothing worth having.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Write the pack's file, then record `row`. `row` is `None` for a server-owned file, which
    /// is placed only where none exists and then belongs to the server. Done only if the file
    /// on disk still has the hash `expected` (`None`: no file) when its turn comes; otherwise
    /// someone changed it meanwhile and it is left as theirs, `before` marked pending.
    Place {
        path: String,
        wanted: Wanted,
        row: Option<TrackedFile>,
        expected: Option<String>,
        before: Option<TrackedFile>,
    },
    /// The file is right as it is; record `row`.
    Record(TrackedFile),
    /// Delete the file and its row: the pack dropped it and nobody edited it. Done only if the
    /// file still has the row's hash when its turn comes.
    Delete(TrackedFile),
    /// Forget the row: the pack dropped a file that is already gone, or the row names a file
    /// another row now names with the pack's new spelling.
    Forget(String),
}

/// How the files on disk compare to what the pack wants.
pub struct Disk<'a> {
    /// The SHA-1 of the file at a path, `None` when there is none.
    pub sha1: &'a dyn Fn(&str) -> Option<String>,
    /// Whether the owner disabled the file at a path: `<path>.disabled` is there instead.
    pub disabled: &'a dyn Fn(&str) -> bool,
    /// The world folder, for the deny-list.
    pub level_name: &'a str,
    /// Paths that differ only in case are one file (see `Folder::case_insensitive`).
    pub case_insensitive: bool,
}

/// The most a set of steps writes, refused before anything is written unless it fits in `room`
/// (see `archive::room`): one check for the whole install. A download is written twice, staged
/// and then placed, so it counts twice. Download sizes are the index's word, so for them this is
/// an estimate; each download is cut off once it no longer fits.
pub fn check_room(steps: &[Step], room: u64) -> Result<u64, PackError> {
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

/// What installing `wanted` over the files on disk does, given the rows recorded for them.
/// `kind` is the source kind of the pack being installed, which every file it places or keeps
/// takes.
///
/// A pack file the owner edited (its hash on disk is not the one recorded, or it is gone) is
/// never touched: when the new pack changes or drops it, the row is marked pending instead. A
/// file nobody recorded that is in the way is the owner's too. Repair (`repair`) also restores
/// pack files that are missing. Server-owned files are placed only where none exists. Rows and
/// wanted paths are matched the way the file system compares paths.
pub fn plan(
    rows: &[TrackedFile],
    wanted: &BTreeMap<String, Wanted>,
    disk: &Disk,
    repair: bool,
    kind: ContentSourceKind,
) -> Vec<Step> {
    let key = |path: &str| fold(path, disk.case_insensitive);
    let rows: HashMap<String, &TrackedFile> = rows
        .iter()
        .map(|row| (key(&row.relative_path), row))
        .collect();
    let wanted_keys: std::collections::HashSet<String> =
        wanted.keys().map(|path| key(path)).collect();
    let pack_row = |path: &str, wanted: &Wanted, pending| TrackedFile {
        relative_path: path.to_owned(),
        source_kind: kind,
        sha1: wanted.sha1.clone(),
        size: wanted.size,
        project_id: None,
        version_id: None,
        pending,
    };
    // A row of the pack takes the new pack's kind; one someone added stays theirs.
    let origin = |row: &TrackedFile| {
        if row.by_pack() {
            kind
        } else {
            ContentSourceKind::Local
        }
    };
    let mut steps = Vec::new();
    for (path, want) in wanted {
        let on_disk = (disk.sha1)(path);
        let row = rows.get(&key(path)).copied();
        // The pack spells a file it already placed another way: the row follows the new
        // spelling, and the old one is only forgotten, never deleted, since it is this file.
        if let Some(row) = row.filter(|row| row.relative_path != *path) {
            steps.push(Step::Forget(row.relative_path.clone()));
        }
        let row = row.map(|row| TrackedFile {
            relative_path: path.clone(),
            ..row.clone()
        });
        let row = row.as_ref();
        if sandbox::is_server_owned(path, disk.level_name) {
            if on_disk.is_none() {
                steps.push(Step::Place {
                    path: path.clone(),
                    wanted: want.clone(),
                    row: None,
                    expected: None,
                    before: None,
                });
            }
            if row.is_some() {
                steps.push(Step::Forget(path.clone()));
            }
            continue;
        }
        let place = || Step::Place {
            path: path.clone(),
            wanted: want.clone(),
            row: Some(TrackedFile {
                project_id: row
                    .filter(|row| row.sha1 == want.sha1)
                    .and_then(|row| row.project_id.clone()),
                version_id: row
                    .filter(|row| row.sha1 == want.sha1)
                    .and_then(|row| row.version_id.clone()),
                ..pack_row(path, want, None)
            }),
            expected: on_disk.clone(),
            before: row.cloned(),
        };
        let keep = |row: &TrackedFile, sha1: &str, source_kind, pending| {
            Step::Record(TrackedFile {
                sha1: sha1.to_owned(),
                source_kind,
                pending,
                ..row.clone()
            })
        };
        // The owner disabled it: left alone, like a file they removed, even by repair.
        if on_disk.is_none() && (disk.disabled)(path) {
            if let Some(row) = row {
                let pending =
                    (row.sha1 != want.sha1).then_some(Pending::Changed);
                steps.push(keep(row, &row.sha1, origin(row), pending));
            }
            continue;
        }
        let step = match (row, on_disk.as_deref()) {
            // Already the pack's file.
            (Some(row), Some(sha1)) if sha1 == want.sha1 => {
                keep(row, &want.sha1, kind, None)
            }
            (None, Some(sha1)) if sha1 == want.sha1 => {
                Step::Record(pack_row(path, want, None))
            }
            // Nothing there: the pack places it, unless the owner deleted a pack file.
            (None, None) => place(),
            (Some(row), None) if repair || !row.by_pack() => place(),
            (Some(row), None) if row.sha1 == want.sha1 => {
                keep(row, &row.sha1, kind, None)
            }
            (Some(row), None) => {
                keep(row, &row.sha1, kind, Some(Pending::Changed))
            }
            // A pack file as the engine placed it: replace it with the new one.
            (Some(row), Some(sha1)) if row.by_pack() && sha1 == row.sha1 => {
                place()
            }
            // Edited, but the pack did not change it: the edit stays, nothing to decide.
            (Some(row), Some(_)) if row.by_pack() && row.sha1 == want.sha1 => {
                keep(row, &row.sha1, kind, None)
            }
            // Edited, and the pack has a new version: the owner decides.
            (Some(row), Some(_)) => {
                keep(row, &row.sha1, origin(row), Some(Pending::Changed))
            }
            // A file nobody recorded is in the way: it is the owner's.
            (None, Some(sha1)) => Step::Record(TrackedFile {
                relative_path: path.clone(),
                source_kind: ContentSourceKind::Local,
                sha1: sha1.to_owned(),
                size: 0,
                project_id: None,
                version_id: None,
                pending: Some(Pending::Changed),
            }),
        };
        steps.push(step);
    }
    for row in rows.values() {
        if !row.by_pack() || wanted_keys.contains(&key(&row.relative_path)) {
            continue;
        }
        // The path became the server's (its world was renamed to it): never deleted.
        if sandbox::is_server_owned(&row.relative_path, disk.level_name) {
            steps.push(Step::Forget(row.relative_path.clone()));
            continue;
        }
        steps.push(match (disk.sha1)(&row.relative_path) {
            None => Step::Forget(row.relative_path.clone()),
            Some(sha1) if sha1 == row.sha1 => Step::Delete((*row).clone()),
            Some(_) => Step::Record(TrackedFile {
                pending: Some(Pending::Removed),
                ..(*row).clone()
            }),
        });
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
        Step::Delete(row) | Step::Record(row) => &row.relative_path,
    }
}

/// Files the owner must know about: pack files they edited since the pack placed them, or
/// deleted (the hash on disk is not the one recorded), and every file waiting on a decision,
/// whatever its origin or hash.
pub fn edited<'a>(
    rows: &'a [TrackedFile],
    sha1: &dyn Fn(&str) -> Option<String>,
) -> Vec<(&'a TrackedFile, Option<String>)> {
    rows.iter()
        .filter_map(|row| {
            let on_disk = sha1(&row.relative_path);
            let changed =
                row.by_pack() && on_disk.as_deref() != Some(row.sha1.as_str());
            (changed || row.pending.is_some()).then_some((row, on_disk))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The source kind of the pack the tests install.
    const KIND: ContentSourceKind = ContentSourceKind::ImportedModpack;

    #[test]
    fn a_server_takes_a_file_unless_the_first_that_knows_rules_it_out() {
        let client_only = Env {
            client: SideType::Required,
            server: SideType::Unsupported,
        };
        let both = Env {
            client: SideType::Required,
            server: SideType::Optional,
        };
        // The index decides when it says.
        assert!(!server_takes(
            Some(&client_only),
            Some(VersionEnvironment::ClientAndServer),
            None
        ));
        assert!(server_takes(
            Some(&both),
            Some(VersionEnvironment::ClientOnly),
            Some(JarEnvironment::ClientOnly)
        ));
        // Then Modrinth, then the jar.
        assert!(!server_takes(
            None,
            Some(VersionEnvironment::ClientOnly),
            None
        ));
        assert!(server_takes(
            None,
            Some(VersionEnvironment::ClientAndServer),
            Some(JarEnvironment::ClientOnly)
        ));
        assert!(!server_takes(None, None, Some(JarEnvironment::ClientOnly)));
        // A Forge jar names no sides, and nothing else knows: taken.
        assert!(server_takes(None, None, None));
    }

    #[test]
    fn players_only_and_server_only_versions_skip_the_other_side() {
        assert_eq!(
            mrpack_env(Some(VersionEnvironment::ClientOnly)).server,
            SideType::Unsupported
        );
        assert_eq!(
            mrpack_env(Some(VersionEnvironment::SingleplayerOnly)).server,
            SideType::Unsupported
        );
        assert_eq!(
            mrpack_env(Some(VersionEnvironment::DedicatedServerOnly)).client,
            SideType::Unsupported
        );
        let both = mrpack_env(None);
        assert_eq!(
            (both.client, both.server),
            (SideType::Required, SideType::Required)
        );
    }

    #[test]
    fn a_file_s_type_is_its_content_folder_s() {
        assert_eq!(project_type_of("mods/a.jar"), Some(ProjectType::Mod));
        assert_eq!(project_type_of("plugins/a.jar"), Some(ProjectType::Plugin));
        assert_eq!(
            project_type_of("world/datapacks/a.zip"),
            Some(ProjectType::DataPack)
        );
        assert_eq!(project_type_of("datapacks/a.zip"), None);
        assert_eq!(project_type_of("config/a.toml"), None);
        assert_eq!(project_type_of("mods/sub/a.jar"), None);
        assert_eq!(project_type_of("a/b/datapacks/a.zip"), None);
    }

    #[test]
    fn minecraft_versions_compare_by_release() {
        use std::cmp::Ordering::{Equal, Greater, Less};
        assert_eq!(release_order("26.2", "26.3"), Some(Less));
        assert_eq!(release_order("1.20.1", "1.20"), Some(Greater));
        assert_eq!(release_order("1.20", "1.20.0"), Some(Equal));
        assert_eq!(release_order("1.9", "1.10"), Some(Less));
        // Not releases: no strict order, on either side.
        assert_eq!(release_order("1.21-pre1", "1.21"), None);
        assert_eq!(release_order("1.21", "1.21-rc1"), None);
        assert_eq!(release_order("24w14a", "1.21"), None);
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
            pending: None,
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
        repair: bool,
    ) -> Vec<Step> {
        run_on(rows, wanted, on_disk, repair, false)
    }

    /// As [`run`], on a file system that compares paths without case when `case_insensitive`.
    fn run_on(
        rows: &[TrackedFile],
        wanted: &[(&str, &str)],
        on_disk: &[(&str, &str)],
        repair: bool,
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
            &Disk {
                sha1: &|path| {
                    on_disk.get(&fold(path, case_insensitive)).cloned()
                },
                disabled: &|path| {
                    on_disk.contains_key(&fold(
                        &format!("{path}.disabled"),
                        case_insensitive,
                    ))
                },
                level_name: "world",
                case_insensitive,
            },
            repair,
            KIND,
        )
    }

    /// The pack's file placed where there was none and no row.
    fn placed(path: &str, sha1: &str) -> Step {
        replaced(path, sha1, None, None)
    }

    /// The pack's file placed over `on_disk`, which `before` recorded.
    fn replaced(
        path: &str,
        sha1: &str,
        on_disk: Option<&str>,
        before: Option<&TrackedFile>,
    ) -> Step {
        Step::Place {
            path: path.into(),
            wanted: want(sha1),
            row: Some(row(path, KIND, sha1)),
            expected: on_disk.map(str::to_owned),
            before: before.cloned(),
        }
    }

    #[test]
    fn a_first_install_places_everything_and_records_it_as_the_packs() {
        let steps = run(
            &[],
            &[("config/a.toml", "a1"), ("mods/m.jar", "m1")],
            &[],
            false,
        );
        assert_eq!(
            steps,
            [placed("config/a.toml", "a1"), placed("mods/m.jar", "m1")]
        );
    }

    #[test]
    fn an_update_replaces_untouched_files_and_leaves_edited_ones_pending() {
        let rows = [
            row("config/edited.toml", KIND, "e1"),
            row("config/same.toml", KIND, "s1"),
            row("mods/old.jar", KIND, "o1"),
            row("mods/kept.jar", KIND, "k1"),
            row("mods/mine.jar", ContentSourceKind::Local, "x1"),
        ];
        let steps = run(
            &rows,
            &[
                ("config/edited.toml", "e2"),
                ("config/same.toml", "s2"),
                ("mods/kept.jar", "k1"),
                ("mods/new.jar", "n1"),
            ],
            &[
                // The owner changed this config after the pack placed it.
                ("config/edited.toml", "owner"),
                ("config/same.toml", "s1"),
                ("mods/old.jar", "o1"),
                ("mods/kept.jar", "k1"),
                ("mods/mine.jar", "x1"),
            ],
            false,
        );
        assert_eq!(
            steps,
            [
                Step::Record(TrackedFile {
                    pending: Some(Pending::Changed),
                    ..rows[0].clone()
                }),
                replaced("config/same.toml", "s2", Some("s1"), Some(&rows[1])),
                Step::Record(rows[3].clone()),
                placed("mods/new.jar", "n1"),
                Step::Delete(rows[2].clone()),
            ]
        );
    }

    #[test]
    fn an_edit_the_pack_did_not_touch_needs_no_decision() {
        let rows = [row("config/a.toml", KIND, "a1")];
        let steps = run(
            &rows,
            &[("config/a.toml", "a1")],
            &[("config/a.toml", "owner")],
            false,
        );
        assert_eq!(steps, [Step::Record(rows[0].clone())]);
    }

    #[test]
    fn edited_files_the_pack_dropped_are_kept_pending() {
        let rows = [
            row("config/gone.toml", KIND, "g1"),
            row("mods/deleted.jar", KIND, "d1"),
        ];
        let steps = run(&rows, &[], &[("config/gone.toml", "owner")], false);
        assert_eq!(
            steps,
            [
                Step::Record(TrackedFile {
                    pending: Some(Pending::Removed),
                    ..rows[0].clone()
                }),
                Step::Forget("mods/deleted.jar".into()),
            ]
        );
    }

    #[test]
    fn a_pack_file_the_owner_deleted_stays_deleted_until_repair() {
        let rows = [row("mods/m.jar", KIND, "m1")];
        assert_eq!(
            run(&rows, &[("mods/m.jar", "m1")], &[], false),
            [Step::Record(rows[0].clone())]
        );
        assert_eq!(
            run(&rows, &[("mods/m.jar", "m2")], &[], false),
            [Step::Record(TrackedFile {
                pending: Some(Pending::Changed),
                ..rows[0].clone()
            })]
        );
        assert_eq!(
            run(&rows, &[("mods/m.jar", "m1")], &[], true),
            [replaced("mods/m.jar", "m1", None, Some(&rows[0]))]
        );
    }

    #[test]
    fn a_file_nobody_recorded_is_the_owners() {
        let steps = run(
            &[],
            &[("config/a.toml", "a1")],
            &[("config/a.toml", "owner")],
            false,
        );
        assert_eq!(
            steps,
            [Step::Record(TrackedFile {
                pending: Some(Pending::Changed),
                size: 0,
                ..row("config/a.toml", ContentSourceKind::Local, "owner")
            })]
        );
        // The same file is simply the pack's.
        let steps = run(
            &[],
            &[("config/a.toml", "a1")],
            &[("config/a.toml", "a1")],
            false,
        );
        assert_eq!(steps, [Step::Record(row("config/a.toml", KIND, "a1"))]);
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
            false,
        );
        assert_eq!(
            steps,
            [Step::Place {
                path: "ops.json".into(),
                wanted: want("o1"),
                row: None,
                expected: None,
                before: None,
            }]
        );
    }

    #[test]
    fn a_pack_file_the_owner_disabled_stays_disabled_even_on_repair() {
        let rows = [row("mods/m.jar", KIND, "m1")];
        let disabled = [("mods/m.jar.disabled", "m1")];
        for repair in [false, true] {
            assert_eq!(
                run(&rows, &[("mods/m.jar", "m1")], &disabled, repair),
                [Step::Record(rows[0].clone())],
                "repair {repair}"
            );
        }
        // A new version waits for the owner, still disabled.
        assert_eq!(
            run(&rows, &[("mods/m.jar", "m2")], &disabled, true),
            [Step::Record(TrackedFile {
                pending: Some(Pending::Changed),
                ..rows[0].clone()
            })]
        );
    }

    #[test]
    fn a_dropped_file_that_is_now_inside_the_world_is_forgotten_not_deleted() {
        // The pack placed `world/data.dat`; then level-name was set to `world`.
        let rows = [row("world/data.dat", KIND, "d1")];
        assert_eq!(
            run(&rows, &[], &[("world/data.dat", "d1")], false),
            [Step::Forget("world/data.dat".into())]
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
                false,
                true,
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
                false,
                false,
            ),
            [Step::Delete(rows[0].clone()), placed("config/a.toml", "a1"),]
        );
    }

    #[test]
    fn two_spellings_of_one_file_refuse_the_pack_where_case_does_not_count() {
        use std::io::Write;

        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("modrinth.index.json", options).unwrap();
        writer
            .write_all(
                br#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"p"}"#,
            )
            .unwrap();
        for name in ["overrides/config/A.toml", "overrides/config/a.toml"] {
            writer.start_file(name, options).unwrap();
            writer.write_all(b"x").unwrap();
        }
        let mut archive =
            zip::ZipArchive::new(writer.finish().unwrap()).unwrap();
        let index = read_index(&mut archive).unwrap();
        let candidates =
            candidates(&index, &mut archive, None, u64::MAX).unwrap();
        assert!(matches!(
            split(
                candidates.clone(),
                &Known::new(),
                &ServerOn::default(),
                true
            ),
            Err(PackError::Collision(..))
        ));
        let both =
            split(candidates, &Known::new(), &ServerOn::default(), false)
                .unwrap()
                .wanted;
        assert_eq!(both.len(), 2);
        assert_eq!(both["config/a.toml"].size, 1);
    }

    /// A jar whose only metadata is `fabric.mod.json` with `environment`.
    fn fabric_jar(environment: &str) -> Vec<u8> {
        use std::io::Write;

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

    #[test]
    fn a_server_leaves_out_what_the_index_modrinth_or_the_jar_says_is_client_only()
     {
        use std::io::Write;

        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("modrinth.index.json", options).unwrap();
        writer
            .write_all(
                br#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"p",
                "dependencies":{"minecraft":"1.21.1","fabric-loader":"0.16.0"},
                "files":[
                {"path":"mods/client.jar","hashes":{"sha1":"c1"},"env":{"client":"required","server":"unsupported"},"downloads":["https://cdn.modrinth.com/data/P/versions/V/client.jar"],"fileSize":1},
                {"path":"mods/asked.jar","hashes":{"sha1":"a1"},"downloads":["https://cdn.modrinth.com/data/Q/versions/W/asked.jar"],"fileSize":1}]}"#,
            )
            .unwrap();
        for (name, bytes) in [
            ("overrides/mods/menu.jar", fabric_jar("client")),
            ("overrides/mods/both.jar", fabric_jar("*")),
            ("overrides/mods/forge.jar", b"not a jar".to_vec()),
            ("server-overrides/mods/menu.jar", fabric_jar("client")),
            ("overrides/resourcepacks/look.zip", b"x".to_vec()),
        ] {
            writer.start_file(name, options).unwrap();
            writer.write_all(&bytes).unwrap();
        }
        let mut archive =
            zip::ZipArchive::new(writer.finish().unwrap()).unwrap();
        let index = read_index(&mut archive).unwrap();
        let candidates = candidates(
            &index,
            &mut archive,
            Some(JarPlatform::Fabric),
            u64::MAX,
        )
        .unwrap();
        let asked = candidates
            .iter()
            .find(|candidate| candidate.path == "mods/asked.jar")
            .unwrap();
        let known = Known::from([(
            asked.wanted.sha1.clone(),
            KnownFile {
                project_id: "Q".into(),
                version_id: "W".into(),
                environment: Some(VersionEnvironment::ClientOnly),
            },
        )]);
        let by_path = split(
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
        let by_project = split(
            candidates.clone(),
            &known,
            &ServerOn {
                paths: HashSet::from(["mods/client-0.9.jar".to_owned()]),
                projects: HashSet::from(["P".to_owned()]),
            },
            false,
        )
        .unwrap();
        assert!(by_project.wanted.contains_key("mods/client.jar"));
        assert!(by_project.turned_on.contains("mods/client.jar"));
        let Split {
            wanted,
            skipped,
            turned_on,
        } = split(candidates, &known, &ServerOn::default(), false).unwrap();
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
            expected: None,
            before: None,
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
    fn edited_files_are_those_whose_hash_changed_or_that_are_gone() {
        let rows = [
            row("config/a.toml", KIND, "a1"),
            row("config/b.toml", KIND, "b1"),
            row("config/c.toml", KIND, "c1"),
            row("mods/mine.jar", ContentSourceKind::Local, "x1"),
            // The owner's file the pack also wants: pending, whatever its hash.
            TrackedFile {
                pending: Some(Pending::Changed),
                ..row("mods/theirs.jar", ContentSourceKind::Local, "t1")
            },
        ];
        let on_disk = HashMap::from([
            ("config/a.toml", "a1"),
            ("config/b.toml", "owner"),
            ("mods/mine.jar", "changed"),
            ("mods/theirs.jar", "t1"),
        ]);
        let edited = edited(&rows, &|path| {
            on_disk.get(path).map(|sha1| (*sha1).to_owned())
        });
        let paths: Vec<_> = edited
            .iter()
            .map(|(row, sha1)| (row.relative_path.as_str(), sha1.as_deref()))
            .collect();
        assert_eq!(
            paths,
            [
                ("config/b.toml", Some("owner")),
                ("config/c.toml", None),
                ("mods/theirs.jar", Some("t1")),
            ]
        );
    }
}
