//! A server's content as a Modrinth `.mrpack`, as upstream's App exports an instance (app-lib
//! `api/instance/export_mrpack.rs`): files Modrinth knows by their hash go in the index with their
//! download URL, and the rest of the content and `config/` go under `overrides/`. Where the server
//! knows a file is for one side only, its override goes under `server-overrides/` or
//! `client-overrides/` instead, and its index entry says so in `env`. Upstream's export picks its
//! folders in a dialog; a server exports what upstream selects by default, which is its content
//! and `config/`.

use std::{
    collections::{BTreeMap, HashMap},
    io::Write,
    path::Path,
};

use bedrock::content::{
    Hashes, Index, IndexFile, SideType, Version, mrpack_env,
};
use modrinth_content_management::ContentType;

use crate::{
    installs::ModLoader,
    state::servers::{
        Content, EntrySides, Error,
        adapters::{
            filesystem::{
                archive::{self, NoRoom},
                content_files::{self, DISABLED},
                folder::Folder,
            },
            sqlite::{content_rows, server_rows::ServerRow},
        },
        domain::sides::Sides,
    },
};

/// The folder upstream's export selects by default besides the content folders.
const CONFIG: &str = "config";

/// Write `server`'s content as an `.mrpack` at `path`, which must be on a disk with room for it.
pub async fn export_mrpack(
    content: &Content,
    server: &ServerRow,
    path: &Path,
) -> Result<(), Error> {
    let loader = server.loader.as_deref().and_then(ModLoader::parse);
    let mut dependencies = BTreeMap::from([(
        "minecraft".to_owned(),
        server.game_version.clone().ok_or_else(|| {
            Error::Invalid("The server has no Minecraft version yet.".into())
        })?,
    )]);
    let key = match loader {
        Some(ModLoader::Fabric) => Some("fabric-loader"),
        Some(ModLoader::Quilt) => Some("quilt-loader"),
        Some(ModLoader::Forge) => Some("forge"),
        Some(ModLoader::NeoForge) => Some("neoforge"),
        Some(ModLoader::Vanilla) | None => None,
        Some(ModLoader::Paper) => {
            return Err(Error::Invalid(
                "A Paper server cannot be exported: Modrinth packs have no Paper loader."
                    .into(),
            ));
        }
    };
    if let Some(key) = key {
        let version = server.loader_version.clone().ok_or_else(|| {
            Error::Invalid("The server's loader has no version yet.".into())
        })?;
        dependencies.insert(key.to_owned(), version);
    }

    let sides: HashMap<String, EntrySides> =
        content_rows::get_entry_sides(&content.db, &server.id).await?;
    let skipped =
        content_rows::get_skipped_files(&content.db, &server.id).await?;
    let root = content.servers.paths().server(&server.id);
    let (found, configs) = tokio::task::spawn_blocking(move || {
        let folder = Folder::open(&root)?;
        let found = content_files::scan(&folder, loader)
            .into_iter()
            .map(|listed| {
                let on_disk = if listed.disabled {
                    format!("{}{DISABLED}", listed.path)
                } else {
                    listed.path.clone()
                };
                let sha1 = folder.sha1(&on_disk)?.unwrap_or_default();
                Ok(Found {
                    sides: Sides::of(&listed, sides.get(&listed.path)),
                    path: listed.path,
                    on_disk,
                    kind: listed.kind,
                    sha1,
                    size: listed.size,
                })
            })
            .collect::<std::io::Result<Vec<_>>>()?;
        Ok::<_, std::io::Error>((found, config_files(&folder)))
    })
    .await
    .map_err(std::io::Error::other)??;

    let sha1s: Vec<String> = found
        .iter()
        .map(|found| found.sha1.clone())
        .chain(skipped.iter().map(|file| file.sha1.clone()))
        .collect();
    let known = content.modrinth.sided_by_hashes(&sha1s).await?;
    let mut files = Vec::new();
    let mut overrides = Vec::new();
    for found in found {
        match known.get(&found.sha1) {
            Some((version, environment)) => {
                let version = Version {
                    environment: *environment,
                    ..version.clone()
                };
                if let Some(file) = index_file(&found, &version) {
                    files.push(file);
                    continue;
                }
                overrides.push(found.into_override());
            }
            None => overrides.push(found.into_override()),
        }
    }
    // The pack's client-only files are not on the server's disk, but players take them.
    for file in skipped {
        let Some((version, environment)) = known.get(&file.sha1) else {
            continue;
        };
        let Some(download) = version
            .files
            .iter()
            .find(|candidate| candidate.hashes.sha1 == file.sha1)
        else {
            continue;
        };
        let mut env = mrpack_env(*environment);
        env.server = SideType::Unsupported;
        files.push(IndexFile {
            path: file.relative_path,
            hashes: Hashes {
                sha1: download.hashes.sha1.clone(),
                sha512: download.hashes.sha512.clone(),
            },
            env: Some(env),
            downloads: vec![download.url.clone()],
            file_size: download.size,
        });
    }
    overrides.extend(configs.into_iter().map(|(path, size)| Override {
        entry: format!("overrides/{path}"),
        on_disk: path,
        size,
    }));
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let index = serde_json::to_vec_pretty(&Index {
        format_version: Index::FORMAT_VERSION,
        game: Index::GAME.to_owned(),
        version_id: "1.0.0".to_owned(),
        name: server.name.clone(),
        summary: None,
        files,
        dependencies,
    })
    .map_err(|error| Error::Invalid(error.to_string()))?;
    let needed = overrides.iter().fold(index.len() as u64, |total, file| {
        total.saturating_add(file.size)
    });
    let parent = path.parent().unwrap_or(path);
    tokio::fs::create_dir_all(parent).await?;
    NoRoom::check("the export", needed, archive::room(parent)?)?;

    let root = content.servers.paths().server(&server.id);
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || write(&root, &path, &index, &overrides))
        .await
        .map_err(std::io::Error::other)?
}

/// An addon file on the server's disk.
#[derive(Debug)]
struct Found {
    /// Its enabled path.
    path: String,
    /// The path of the file there now, with `.disabled` when it is off.
    on_disk: String,
    kind: ContentType,
    sides: Sides,
    sha1: String,
    size: u64,
}

impl Found {
    /// Whether players take it: never a plugin or a world's datapack, which only a server reads.
    fn for_players(&self) -> bool {
        !self.sides.disabled_player
            && !matches!(self.kind, ContentType::Plugin | ContentType::DataPack)
    }

    fn for_server(&self) -> bool {
        !self.sides.disabled_server
    }

    /// Its path in the pack: off for both sides it keeps `.disabled`, as upstream's export does.
    fn pack_path(&self) -> &str {
        if self.sides.disabled {
            &self.on_disk
        } else {
            &self.path
        }
    }

    /// Where it goes as an override: `overrides/` for both sides, else the one side's folder.
    fn into_override(self) -> Override {
        let folder = match (
            self.sides.disabled,
            self.for_players(),
            self.for_server(),
        ) {
            (true, _, _) | (false, true, true) => "overrides",
            (false, true, false) => "client-overrides",
            (false, false, _) => "server-overrides",
        };
        Override {
            entry: format!("{folder}/{}", self.pack_path()),
            on_disk: self.on_disk,
            size: self.size,
        }
    }
}

/// A file copied into the pack.
#[derive(Debug)]
struct Override {
    /// Its name in the archive.
    entry: String,
    on_disk: String,
    size: u64,
}

/// `found`'s index entry, when `version` has a file with its hash.
fn index_file(found: &Found, version: &Version) -> Option<IndexFile> {
    let file = version
        .files
        .iter()
        .find(|file| file.hashes.sha1 == found.sha1)?;
    let mut env = mrpack_env(version.environment);
    if !found.for_players() {
        env.client = SideType::Unsupported;
    }
    if !found.for_server() {
        env.server = SideType::Unsupported;
    }
    Some(IndexFile {
        path: found.pack_path().to_owned(),
        hashes: Hashes {
            sha1: file.hashes.sha1.clone(),
            sha512: file.hashes.sha512.clone(),
        },
        env: Some(env),
        downloads: vec![file.url.clone()],
        file_size: file.size,
    })
}

/// Every plain file under `config/`, by path, with its size. Links and folders that cannot be
/// opened are skipped.
fn config_files(folder: &Folder) -> Vec<(String, u64)> {
    let mut files = Vec::new();
    let mut folders = vec![CONFIG.to_owned()];
    while let Some(path) = folders.pop() {
        let Ok(entries) =
            folder.open_dir(Some(&path)).and_then(|dir| dir.entries())
        else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let (Ok(name), Ok(kind)) =
                (entry.file_name().into_string(), entry.file_type())
            else {
                continue;
            };
            let child = format!("{path}/{name}");
            if kind.is_dir() {
                folders.push(child);
            } else if kind.is_file() {
                let size = entry.metadata().map_or(0, |meta| meta.len());
                files.push((child, size));
            }
        }
    }
    files.sort();
    files
}

/// The archive: the overrides, read without following links, then the index.
fn write(
    root: &Path,
    path: &Path,
    index: &[u8],
    overrides: &[Override],
) -> Result<(), Error> {
    let zip_error =
        |error: zip::result::ZipError| Error::Io(std::io::Error::other(error));
    let folder = Folder::open(root)?;
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(
        std::fs::File::create(path)?,
    ));
    for file in overrides {
        // Gone since it was listed: the server changed it meanwhile.
        let Some(source) = folder.open_file(&file.on_disk)? else {
            continue;
        };
        zip.start_file(
            &file.entry,
            options.large_file(file.size >= u64::from(u32::MAX)),
        )
        .map_err(zip_error)?;
        std::io::copy(&mut source.into_std(), &mut zip)?;
    }
    zip.start_file("modrinth.index.json", options)
        .map_err(zip_error)?;
    zip.write_all(index)?;
    zip.finish().map_err(zip_error)?.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use bedrock::content::{VersionEnvironment, VersionFile, VersionHashes};

    use super::*;

    fn found(path: &str, kind: ContentType, sides: Sides) -> Found {
        Found {
            path: path.into(),
            on_disk: if sides.disabled || sides.disabled_server {
                format!("{path}{DISABLED}")
            } else {
                path.into()
            },
            kind,
            sides,
            sha1: "aa".into(),
            size: 3,
        }
    }

    fn on(disabled_server: bool, disabled_player: bool) -> Sides {
        Sides {
            disabled: false,
            disabled_server,
            disabled_player,
        }
    }

    fn version(environment: VersionEnvironment) -> Version {
        Version {
            id: "v".into(),
            project_id: "p".into(),
            version_number: "1".into(),
            version_type: Default::default(),
            loaders: Vec::new(),
            game_versions: Vec::new(),
            files: vec![VersionFile {
                url: "https://cdn.modrinth.com/data/p/versions/v/a.jar".into(),
                filename: "a.jar".into(),
                primary: true,
                hashes: VersionHashes {
                    sha1: "aa".into(),
                    sha512: Some("bb".into()),
                },
                size: 3,
            }],
            dependencies: Vec::new(),
            date_published: None,
            environment: Some(environment),
        }
    }

    #[test]
    fn a_modrinth_file_keeps_its_sides_narrowed_by_the_server_s_own() {
        let both = version(VersionEnvironment::ClientAndServer);
        let file = index_file(
            &found("mods/a.jar", ContentType::Mod, on(false, true)),
            &both,
        )
        .unwrap();
        assert_eq!(file.path, "mods/a.jar");
        assert_eq!(
            file.downloads,
            ["https://cdn.modrinth.com/data/p/versions/v/a.jar"]
        );
        let env = file.env.unwrap();
        assert_eq!(
            (env.client, env.server),
            (SideType::Unsupported, SideType::Required)
        );

        let client = version(VersionEnvironment::ClientOnly);
        let env = index_file(
            &found("mods/a.jar", ContentType::Mod, on(true, false)),
            &client,
        )
        .unwrap()
        .env
        .unwrap();
        assert_eq!(
            (env.client, env.server),
            (SideType::Required, SideType::Unsupported)
        );
    }

    #[test]
    fn a_file_modrinth_does_not_know_goes_to_the_overrides_of_its_sides() {
        let entry = |found: Found| found.into_override().entry;
        assert_eq!(
            entry(found("mods/a.jar", ContentType::Mod, on(false, false))),
            "overrides/mods/a.jar"
        );
        assert_eq!(
            entry(found("mods/a.jar", ContentType::Mod, on(true, false))),
            "client-overrides/mods/a.jar"
        );
        assert_eq!(
            entry(found(
                "plugins/a.jar",
                ContentType::Plugin,
                on(false, false)
            )),
            "server-overrides/plugins/a.jar"
        );
        let off = Sides {
            disabled: true,
            disabled_server: false,
            disabled_player: false,
        };
        assert_eq!(
            entry(found("mods/a.jar", ContentType::Mod, off)),
            "overrides/mods/a.jar.disabled"
        );
    }
}
