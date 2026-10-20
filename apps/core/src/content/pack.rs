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
}

/// Every file a server running `index` should have, by path. Client-only files are left out, and
/// `server-overrides/` wins over `overrides/`. Downloads must come from an allowed host.
pub fn wanted<R: Read + Seek>(
    index: &Index,
    archive: &mut zip::ZipArchive<R>,
) -> Result<BTreeMap<String, Wanted>, PackError> {
    let mut wanted = BTreeMap::new();
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
        wanted.insert(
            path,
            Wanted {
                sha1: file.hashes.sha1.to_ascii_lowercase(),
                source: Source::Download(n),
            },
        );
    }
    let entries = archive::entries(archive)?;
    for prefix in ["overrides/", "server-overrides/"] {
        for (n, (path, is_dir)) in entries.iter().enumerate() {
            let Some(path) = path.strip_prefix(prefix).filter(|_| !is_dir)
            else {
                continue;
            };
            wanted.insert(
                path.to_owned(),
                Wanted {
                    sha1: archive::entry_sha1(archive, n)?,
                    source: Source::Override(n),
                },
            );
        }
    }
    Ok(wanted)
}

/// One thing an install does to a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Write the pack's file, then record `row`. `row` is `None` for a server-owned file, which
    /// is placed only where none exists and then belongs to the server.
    Place {
        path: String,
        wanted: Wanted,
        row: Option<ContentFile>,
    },
    /// The file is right as it is; record `row`.
    Record(ContentFile),
    /// Delete the file and its row: the pack dropped it and nobody edited it.
    Delete(String),
    /// Forget the row: the pack dropped a file that is already gone.
    Forget(String),
}

/// How the files on disk compare to what the pack wants.
pub struct Disk<'a> {
    /// The SHA-1 of the file at a path, `None` when there is none.
    pub sha1: &'a dyn Fn(&str) -> Option<String>,
    /// The world folder, for the deny-list.
    pub level_name: &'a str,
}

/// What installing `wanted` over the files on disk does, given the rows recorded for them.
///
/// A pack file the owner edited (its hash on disk is not the one recorded, or it is gone) is
/// never touched: when the new pack changes or drops it, the row is marked pending instead. A
/// file nobody recorded that is in the way is the owner's too. Repair (`repair`) also restores
/// pack files that are missing. Server-owned files are placed only where none exists.
pub fn plan(
    rows: &[ContentFile],
    wanted: &BTreeMap<String, Wanted>,
    disk: &Disk,
    repair: bool,
) -> Vec<Step> {
    let rows: HashMap<&str, &ContentFile> =
        rows.iter().map(|row| (row.path.as_str(), row)).collect();
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
        let row = rows.get(path.as_str()).copied();
        if folder::is_server_owned(path, disk.level_name) {
            if on_disk.is_none() {
                steps.push(Step::Place {
                    path: path.clone(),
                    wanted: want.clone(),
                    row: None,
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
        };
        let keep = |row: &ContentFile, sha1: &str, origin, pending| {
            Step::Record(ContentFile {
                sha1: sha1.to_owned(),
                origin,
                pending,
                ..row.clone()
            })
        };
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
        if row.origin != Origin::Pack || wanted.contains_key(&row.path) {
            continue;
        }
        steps.push(match (disk.sha1)(&row.path) {
            None => Step::Forget(row.path.clone()),
            Some(sha1) if sha1 == row.sha1 => Step::Delete(row.path.clone()),
            Some(_) => Step::Record(ContentFile {
                pending: Some(Pending::Removed),
                ..(*row).clone()
            }),
        });
    }
    steps.sort_by(|a, b| step_path(a).cmp(step_path(b)));
    steps
}

fn step_path(step: &Step) -> &str {
    match step {
        Step::Place { path, .. } | Step::Delete(path) | Step::Forget(path) => {
            path
        }
        Step::Record(row) => &row.path,
    }
}

/// Pack files the owner edited since the pack placed them, or deleted: their hash on disk is not
/// the one recorded.
pub fn edited<'a>(
    rows: &'a [ContentFile],
    sha1: &dyn Fn(&str) -> Option<String>,
) -> Vec<(&'a ContentFile, Option<String>)> {
    rows.iter()
        .filter(|row| row.origin == Origin::Pack)
        .filter_map(|row| {
            let on_disk = sha1(&row.path);
            (on_disk.as_deref() != Some(row.sha1.as_str()))
                .then_some((row, on_disk))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }

    fn run(
        rows: &[ContentFile],
        wanted: &[(&str, &str)],
        on_disk: &[(&str, &str)],
        repair: bool,
    ) -> Vec<Step> {
        let wanted = wanted
            .iter()
            .map(|(path, sha1)| ((*path).to_owned(), want(sha1)))
            .collect();
        let on_disk: HashMap<String, String> = on_disk
            .iter()
            .map(|(path, sha1)| ((*path).to_owned(), (*sha1).to_owned()))
            .collect();
        plan(
            rows,
            &wanted,
            &Disk {
                sha1: &|path| on_disk.get(path).cloned(),
                level_name: "world",
            },
            repair,
        )
    }

    fn placed(path: &str, sha1: &str) -> Step {
        Step::Place {
            path: path.into(),
            wanted: want(sha1),
            row: Some(row(path, Origin::Pack, sha1)),
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
                placed("config/same.toml", "s2"),
                Step::Record(rows[3].clone()),
                placed("mods/new.jar", "n1"),
                Step::Delete("mods/old.jar".into()),
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
            [placed("mods/m.jar", "m1")]
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
            }]
        );
    }

    #[test]
    fn edited_files_are_those_whose_hash_changed_or_that_are_gone() {
        let rows = [
            row("config/a.toml", Origin::Pack, "a1"),
            row("config/b.toml", Origin::Pack, "b1"),
            row("config/c.toml", Origin::Pack, "c1"),
            row("mods/mine.jar", Origin::Added, "x1"),
        ];
        let on_disk = HashMap::from([
            ("config/a.toml", "a1"),
            ("config/b.toml", "owner"),
            ("mods/mine.jar", "changed"),
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
            [("config/b.toml", Some("owner")), ("config/c.toml", None)]
        );
    }
}
