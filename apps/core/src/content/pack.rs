//! Packs: Modrinth's `.mrpack`. A pack is an index of files to download plus `overrides/` and
//! `server-overrides/` to unpack. Installing or updating one is planned as a whole first
//! ([`plan`]): every file the pack wants is placed, kept, or, when the owner edited it since the
//! pack put it there, left alone and marked pending until the App says keep or replace.

use std::{
    collections::{BTreeMap, HashMap},
    io::{Read, Seek},
};

use serde::Deserialize;

use super::{
    archive::{self, ArchiveError},
    folder::{self, PathError},
    records::{ContentFile, Origin, Pending},
};
use crate::installs::ModLoader;

/// The index is small; a pack claiming a larger one is refused.
const MAX_INDEX: u64 = 32 * 1024 * 1024;

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
    #[error(
        "the pack would unpack more than {} GB", archive::MAX_UNPACKED / 1024 / 1024 / 1024
    )]
    TooLarge,
}

/// `modrinth.index.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub format_version: u32,
    pub game: String,
    pub version_id: String,
    pub name: String,
    pub summary: Option<String>,
    #[serde(default)]
    pub files: Vec<IndexFile>,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexFile {
    pub path: String,
    pub hashes: IndexHashes,
    pub env: Option<Env>,
    #[serde(default)]
    pub downloads: Vec<String>,
    pub file_size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexHashes {
    pub sha1: String,
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Env {
    pub server: String,
}

/// Read and check a pack's index.
pub fn read_index<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Index, PackError> {
    let not_a_pack = |reason: &str| PackError::NotAPack(reason.to_owned());
    let mut text = String::new();
    archive
        .by_name("modrinth.index.json")
        .map_err(|_| not_a_pack("it has no modrinth.index.json"))?
        .take(MAX_INDEX)
        .read_to_string(&mut text)
        .map_err(|_| not_a_pack("its index is unreadable"))?;
    let index: Index = serde_json::from_str(&text)
        .map_err(|error| PackError::NotAPack(error.to_string()))?;
    if index.format_version != 1 || index.game != "minecraft" {
        return Err(not_a_pack("it is not a Minecraft pack of format 1"));
    }
    Ok(index)
}

/// The loader and versions a pack runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub loader: ModLoader,
    pub game_version: String,
    pub loader_version: Option<String>,
}

pub fn platform(index: &Index) -> Result<Platform, PackError> {
    let game_version = index
        .dependencies
        .get("minecraft")
        .ok_or_else(|| {
            PackError::NotAPack("it names no Minecraft version".into())
        })?
        .clone();
    let loaders = [
        ("fabric-loader", ModLoader::Fabric),
        ("quilt-loader", ModLoader::Quilt),
        ("forge", ModLoader::Forge),
        ("neoforge", ModLoader::NeoForge),
    ];
    let (loader, loader_version) = loaders
        .iter()
        .find_map(|(key, loader)| {
            index
                .dependencies
                .get(*key)
                .map(|version| (*loader, Some(version.clone())))
        })
        .unwrap_or((ModLoader::Vanilla, None));
    Ok(Platform {
        loader,
        game_version,
        loader_version,
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
        folder::fold_case(path)
    } else {
        path.to_owned()
    }
}

/// Every file a server running `index` should have, by path. Client-only files are left out, and
/// `server-overrides/` wins over `overrides/`. Downloads must come from an allowed host.
pub fn wanted<R: Read + Seek>(
    index: &Index,
    archive: &mut zip::ZipArchive<R>,
    case_insensitive: bool,
) -> Result<BTreeMap<String, Wanted>, PackError> {
    let mut wanted = BTreeMap::new();
    // Two spellings of one file are refused; the same spelling twice is an override.
    let mut spellings: HashMap<String, String> = HashMap::new();
    let mut add = |path: String, want: Wanted| match spellings
        .insert(fold(&path, case_insensitive), path.clone())
    {
        Some(other) if other != path => Err(PackError::Collision(other, path)),
        _ => {
            wanted.insert(path, want);
            Ok(())
        }
    };
    for (n, file) in index.files.iter().enumerate() {
        if file
            .env
            .as_ref()
            .is_some_and(|env| env.server == "unsupported")
        {
            continue;
        }
        let path = folder::clean(&file.path)?;
        let usable = file.downloads.iter().any(|url| {
            reqwest::Url::parse(url)
                .is_ok_and(|url| super::hosts::allowed(&url, false))
        });
        if !usable {
            return Err(PackError::Host(path));
        }
        add(
            path,
            Wanted {
                sha1: file.hashes.sha1.to_ascii_lowercase(),
                source: Source::Download(n),
                size: file.file_size,
            },
        )?;
    }
    let entries = archive::entries(archive)?;
    // One budget for every override: a small pack must not inflate past it entry by entry.
    let mut budget = archive::MAX_UNPACKED;
    for prefix in ["overrides/", "server-overrides/"] {
        for (n, (path, is_dir)) in entries.iter().enumerate() {
            let Some(path) = path.strip_prefix(prefix).filter(|_| !is_dir)
            else {
                continue;
            };
            let (sha1, size) = archive::entry_sha1(archive, n, &mut budget)?;
            add(
                path.to_owned(),
                Wanted {
                    sha1,
                    source: Source::Override(n),
                    size,
                },
            )?;
        }
    }
    Ok(wanted)
}

/// One thing an install does to a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Write the pack's file, then record `row`. `row` is `None` for a server-owned file, which
    /// is placed only where none exists and then belongs to the server. Done only if the file
    /// on disk still has the hash `expected` (`None`: no file) when its turn comes; otherwise
    /// someone changed it meanwhile and it is left as theirs, `before` marked pending.
    Place {
        path: String,
        wanted: Wanted,
        row: Option<ContentFile>,
        expected: Option<String>,
        before: Option<ContentFile>,
    },
    /// The file is right as it is; record `row`.
    Record(ContentFile),
    /// Delete the file and its row: the pack dropped it and nobody edited it. Done only if the
    /// file still has the row's hash when its turn comes.
    Delete(ContentFile),
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

/// The most a set of steps writes, refused past [`archive::MAX_UNPACKED`] before anything is
/// written: one budget for the whole install. Download sizes are the index's word, so for them
/// this is an estimate; each download is only capped at [`super::install::MAX_FILE`].
pub fn check_budget(steps: &[Step]) -> Result<u64, PackError> {
    let total = steps
        .iter()
        .map(|step| match step {
            Step::Place { wanted, .. } => wanted.size,
            _ => 0,
        })
        .fold(0u64, u64::saturating_add);
    if total > archive::MAX_UNPACKED {
        return Err(PackError::TooLarge);
    }
    Ok(total)
}

/// What installing `wanted` over the files on disk does, given the rows recorded for them.
///
/// A pack file the owner edited (its hash on disk is not the one recorded, or it is gone) is
/// never touched: when the new pack changes or drops it, the row is marked pending instead. A
/// file nobody recorded that is in the way is the owner's too. Repair (`repair`) also restores
/// pack files that are missing. Server-owned files are placed only where none exists. Rows and
/// wanted paths are matched the way the file system compares paths.
pub fn plan(
    rows: &[ContentFile],
    wanted: &BTreeMap<String, Wanted>,
    disk: &Disk,
    repair: bool,
) -> Vec<Step> {
    let key = |path: &str| fold(path, disk.case_insensitive);
    let rows: HashMap<String, &ContentFile> =
        rows.iter().map(|row| (key(&row.path), row)).collect();
    let wanted_keys: std::collections::HashSet<String> =
        wanted.keys().map(|path| key(path)).collect();
    let pack_row = |path: &str, wanted: &Wanted, pending| ContentFile {
        path: path.to_owned(),
        origin: Origin::Pack,
        sha1: wanted.sha1.clone(),
        project_id: None,
        version_id: None,
        pending,
    };
    let mut steps = Vec::new();
    for (path, want) in wanted {
        let on_disk = (disk.sha1)(path);
        let row = rows.get(&key(path)).copied();
        // The pack spells a file it already placed another way: the row follows the new
        // spelling, and the old one is only forgotten, never deleted, since it is this file.
        if let Some(row) = row.filter(|row| row.path != *path) {
            steps.push(Step::Forget(row.path.clone()));
        }
        let row = row.map(|row| ContentFile {
            path: path.clone(),
            ..row.clone()
        });
        let row = row.as_ref();
        if folder::is_server_owned(path, disk.level_name) {
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
            row: Some(ContentFile {
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
        let keep = |row: &ContentFile, sha1: &str, origin, pending| {
            Step::Record(ContentFile {
                sha1: sha1.to_owned(),
                origin,
                pending,
                ..row.clone()
            })
        };
        // The owner disabled it: left alone, like a file they removed, even by repair.
        if on_disk.is_none() && (disk.disabled)(path) {
            if let Some(row) = row {
                let pending =
                    (row.sha1 != want.sha1).then_some(Pending::Changed);
                steps.push(keep(row, &row.sha1, row.origin, pending));
            }
            continue;
        }
        let step = match (row, on_disk.as_deref()) {
            // Already the pack's file.
            (Some(row), Some(sha1)) if sha1 == want.sha1 => {
                keep(row, &want.sha1, Origin::Pack, None)
            }
            (None, Some(sha1)) if sha1 == want.sha1 => {
                Step::Record(pack_row(path, want, None))
            }
            // Nothing there: the pack places it, unless the owner deleted a pack file.
            (None, None) => place(),
            (Some(row), None) if repair || row.origin == Origin::Added => {
                place()
            }
            (Some(row), None) if row.sha1 == want.sha1 => {
                keep(row, &row.sha1, Origin::Pack, None)
            }
            (Some(row), None) => {
                keep(row, &row.sha1, Origin::Pack, Some(Pending::Changed))
            }
            // A pack file as Core placed it: replace it with the new one.
            (Some(row), Some(sha1))
                if row.origin == Origin::Pack && sha1 == row.sha1 =>
            {
                place()
            }
            // Edited, but the pack did not change it: the edit stays, nothing to decide.
            (Some(row), Some(_))
                if row.origin == Origin::Pack && row.sha1 == want.sha1 =>
            {
                keep(row, &row.sha1, Origin::Pack, None)
            }
            // Edited, and the pack has a new version: the owner decides.
            (Some(row), Some(_)) => {
                keep(row, &row.sha1, row.origin, Some(Pending::Changed))
            }
            // A file nobody recorded is in the way: it is the owner's.
            (None, Some(sha1)) => Step::Record(ContentFile {
                path: path.clone(),
                origin: Origin::Added,
                sha1: sha1.to_owned(),
                project_id: None,
                version_id: None,
                pending: Some(Pending::Changed),
            }),
        };
        steps.push(step);
    }
    for row in rows.values() {
        if row.origin != Origin::Pack || wanted_keys.contains(&key(&row.path)) {
            continue;
        }
        // The path became the server's (its world was renamed to it): never deleted.
        if folder::is_server_owned(&row.path, disk.level_name) {
            steps.push(Step::Forget(row.path.clone()));
            continue;
        }
        steps.push(match (disk.sha1)(&row.path) {
            None => Step::Forget(row.path.clone()),
            Some(sha1) if sha1 == row.sha1 => Step::Delete((*row).clone()),
            Some(_) => Step::Record(ContentFile {
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
        Step::Delete(row) | Step::Record(row) => &row.path,
    }
}

/// Files the owner must know about: pack files they edited since the pack placed them, or
/// deleted (the hash on disk is not the one recorded), and every file waiting on a decision,
/// whatever its origin or hash.
pub fn edited<'a>(
    rows: &'a [ContentFile],
    sha1: &dyn Fn(&str) -> Option<String>,
) -> Vec<(&'a ContentFile, Option<String>)> {
    rows.iter()
        .filter_map(|row| {
            let on_disk = sha1(&row.path);
            let changed = row.origin == Origin::Pack
                && on_disk.as_deref() != Some(row.sha1.as_str());
            (changed || row.pending.is_some()).then_some((row, on_disk))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn row(path: &str, origin: Origin, sha1: &str) -> ContentFile {
        ContentFile {
            path: path.into(),
            origin,
            sha1: sha1.into(),
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
        rows: &[ContentFile],
        wanted: &[(&str, &str)],
        on_disk: &[(&str, &str)],
        repair: bool,
    ) -> Vec<Step> {
        run_on(rows, wanted, on_disk, repair, false)
    }

    /// As [`run`], on a file system that compares paths without case when `case_insensitive`.
    fn run_on(
        rows: &[ContentFile],
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
        before: Option<&ContentFile>,
    ) -> Step {
        Step::Place {
            path: path.into(),
            wanted: want(sha1),
            row: Some(row(path, Origin::Pack, sha1)),
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
            row("config/edited.toml", Origin::Pack, "e1"),
            row("config/same.toml", Origin::Pack, "s1"),
            row("mods/old.jar", Origin::Pack, "o1"),
            row("mods/kept.jar", Origin::Pack, "k1"),
            row("mods/mine.jar", Origin::Added, "x1"),
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
                Step::Record(ContentFile {
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
        let rows = [row("config/a.toml", Origin::Pack, "a1")];
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
            row("config/gone.toml", Origin::Pack, "g1"),
            row("mods/deleted.jar", Origin::Pack, "d1"),
        ];
        let steps = run(&rows, &[], &[("config/gone.toml", "owner")], false);
        assert_eq!(
            steps,
            [
                Step::Record(ContentFile {
                    pending: Some(Pending::Removed),
                    ..rows[0].clone()
                }),
                Step::Forget("mods/deleted.jar".into()),
            ]
        );
    }

    #[test]
    fn a_pack_file_the_owner_deleted_stays_deleted_until_repair() {
        let rows = [row("mods/m.jar", Origin::Pack, "m1")];
        assert_eq!(
            run(&rows, &[("mods/m.jar", "m1")], &[], false),
            [Step::Record(rows[0].clone())]
        );
        assert_eq!(
            run(&rows, &[("mods/m.jar", "m2")], &[], false),
            [Step::Record(ContentFile {
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
            [Step::Record(ContentFile {
                pending: Some(Pending::Changed),
                ..row("config/a.toml", Origin::Added, "owner")
            })]
        );
        // The same file is simply the pack's.
        let steps = run(
            &[],
            &[("config/a.toml", "a1")],
            &[("config/a.toml", "a1")],
            false,
        );
        assert_eq!(
            steps,
            [Step::Record(row("config/a.toml", Origin::Pack, "a1"))]
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
        let rows = [row("mods/m.jar", Origin::Pack, "m1")];
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
            [Step::Record(ContentFile {
                pending: Some(Pending::Changed),
                ..rows[0].clone()
            })]
        );
    }

    #[test]
    fn a_dropped_file_that_is_now_inside_the_world_is_forgotten_not_deleted() {
        // The pack placed `world/data.dat`; then level-name was set to `world`.
        let rows = [row("world/data.dat", Origin::Pack, "d1")];
        assert_eq!(
            run(&rows, &[], &[("world/data.dat", "d1")], false),
            [Step::Forget("world/data.dat".into())]
        );
    }

    #[test]
    fn a_pack_that_respells_a_file_moves_its_row_without_deleting_it() {
        let rows = [row("Config/A.toml", Origin::Pack, "a1")];
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
                Step::Record(row("config/a.toml", Origin::Pack, "a1")),
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
        assert!(matches!(
            wanted(&index, &mut archive, true),
            Err(PackError::Collision(..))
        ));
        let both = wanted(&index, &mut archive, false).unwrap();
        assert_eq!(both.len(), 2);
        assert_eq!(both["config/a.toml"].size, 1);
    }

    #[test]
    fn one_budget_covers_the_whole_install() {
        let place = |size| Step::Place {
            path: "mods/m.jar".into(),
            wanted: Wanted { size, ..want("m1") },
            row: None,
            expected: None,
            before: None,
        };
        let half = archive::MAX_UNPACKED / 2;
        assert_eq!(
            check_budget(&[place(half), place(half)]).unwrap(),
            archive::MAX_UNPACKED
        );
        // Each file fits on its own; together they do not.
        assert!(matches!(
            check_budget(&[place(half), place(half), place(1)]),
            Err(PackError::TooLarge)
        ));
    }

    #[test]
    fn edited_files_are_those_whose_hash_changed_or_that_are_gone() {
        let rows = [
            row("config/a.toml", Origin::Pack, "a1"),
            row("config/b.toml", Origin::Pack, "b1"),
            row("config/c.toml", Origin::Pack, "c1"),
            row("mods/mine.jar", Origin::Added, "x1"),
            // The owner's file the pack also wants: pending, whatever its hash.
            ContentFile {
                pending: Some(Pending::Changed),
                ..row("mods/theirs.jar", Origin::Added, "t1")
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
            .map(|(row, sha1)| (row.path.as_str(), sha1.as_deref()))
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
