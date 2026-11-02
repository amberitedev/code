//! A share version as a pack. A server follows an App instance by applying versions of the
//! instance's share, like one more member. Core reads the share's latest version from the
//! backend with its own key, for the user who asked for the install, who must be able to read
//! the share and reach this Core: a Modrinth pack index for the files on Modrinth, and the pool
//! entries of the rest. It fetches those through the store API of the version's holders, as a
//! member's App does, decrypts and checks them, and writes an `.mrpack` with them as overrides,
//! which installs like any other pack. A version based on a modpack takes every file of the
//! modpack it keeps (`sharing_client::install::version_files`): the modpack's index entries go
//! into the index as they are, and its overrides are copied from the modpack's own `.mrpack`.
//!
//! Core only ever asks a holder for a file through the store API, so where and how the pool
//! keeps files does not matter here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sharing_client::backend::Backend;
use sharing_client::consume::{self, ReadyVersion, ServerPackError};
use sharing_client::install::ModpackFile;
use sharing_client::pool::{self, FetchError, PoolFile};

use super::install_pack as install;
use crate::{
    installs::{ModLoader, shared::SharedDirs},
    state::servers::{
        Content, Error,
        adapters::filesystem::archive::{self, NoRoom},
        domain::pack::{self, IndexFile, Source},
    },
};

/// Where a file of a share's modpack comes from.
#[derive(Debug, Clone)]
pub enum PackEntry {
    /// An entry of the modpack's index, which the pack install downloads.
    Index(IndexFile),
    /// The entry named `entry` of the modpack's `.mrpack`, stored as `pack`, of `size` bytes.
    Archive {
        pack: String,
        entry: String,
        size: u64,
    },
}

/// Fetch the files of `version` (read with [`consume::ready_version`] through `backend`) and
/// write it as an `.mrpack` named `name` in the shared store. Holders that serve bad bytes are
/// reported through `backend`, as a member's App reports them. Returns the pack's SHA-1 and the
/// version.
pub async fn write_pack(
    content: &Content,
    backend: &Backend,
    server_id: &str,
    version: ReadyVersion,
    name: &str,
) -> Result<(String, i32), Error> {
    let version = consume::server_pack(version, name, &Lookup(content))
        .await
        .map_err(|error| match error {
            ServerPackError::Modrinth(error) => error,
            error => Error::Invalid(error.to_string()),
        })?;
    let copied: u64 = version
        .modpack_files
        .iter()
        .map(|file| match &file.source {
            PackEntry::Archive { size, .. } => *size,
            PackEntry::Index(_) => 0,
        })
        .sum();
    // The pool's files, then the pack written from them and the modpack's overrides.
    let total = version
        .files
        .iter()
        .map(|(_, file)| file.object_size)
        .sum::<u64>()
        .saturating_mul(2)
        .saturating_add(copied);
    let staging = content
        .paths
        .staging(server_id)
        .join(uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(&staging).await?;
    NoRoom::check("the instance's files", total, archive::room(&staging)?)?;
    let result = async {
        let mut fetched = Vec::with_capacity(version.files.len());
        for (n, (name, file)) in version.files.iter().enumerate() {
            let path = staging.join(n.to_string());
            fetch(content, backend, name, file, &path).await?;
            fetched.push((name.clone(), path));
        }
        let dirs = SharedDirs::new(content.paths.root());
        let packed = staging.join("pack.mrpack");
        let mut index = version.index.clone();
        let mut copies = Vec::new();
        for file in &version.modpack_files {
            match &file.source {
                PackEntry::Index(entry) => index.files.push(entry.clone()),
                PackEntry::Archive { pack, entry, .. } => {
                    copies.push((dirs.pack(pack), entry.clone()));
                }
            }
        }
        let index = serde_json::to_vec(&index)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let sha1 = {
            let packed = packed.clone();
            tokio::task::spawn_blocking(move || {
                zip(&packed, &index, &fetched, &copies)
            })
            .await
            .map_err(std::io::Error::other)??
        };
        let stored = dirs.pack(&sha1);
        if let Some(parent) = stored.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::rename(&packed, &stored).await?;
        Ok((sha1, version.version))
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&staging).await;
    result
}

/// Modrinth as a share's server pack needs it, through Core's own client.
struct Lookup<'a>(&'a Content);

impl consume::Modrinth for Lookup<'_> {
    type Error = Error;
    type Source = PackEntry;

    async fn versions(
        &self,
        ids: &[String],
    ) -> Result<Vec<bedrock::content::SidedVersion>, Error> {
        Ok(self.0.modrinth.sided_versions(ids).await?)
    }

    /// Every file of the modpack, client-only ones too: the server's pack install splits the
    /// written pack as it splits any pack, so they show there as client only.
    async fn modpack_files(
        &self,
        id: &str,
    ) -> Result<Vec<ModpackFile<PackEntry>>, Error> {
        let dirs = SharedDirs::new(self.0.paths.root());
        let sha1 =
            install::fetch_modrinth_pack(self.0, &dirs, id, &|_| {}).await?;
        let path = dirs.pack(&sha1);
        let room = archive::room(self.0.paths.root())?;
        let files = tokio::task::spawn_blocking(move || {
            let mut archive = zip::ZipArchive::new(std::fs::File::open(&path)?)
                .map_err(archive::ArchiveError::from)?;
            let index = pack::read_index(&mut archive)?;
            let loader = pack::platform(&index)?.loader;
            let candidates = pack::candidates(
                &index,
                &mut archive,
                ModLoader::from(loader).jar_platform(),
                room,
            )?;
            let files = candidates
                .into_iter()
                .map(|candidate| {
                    let source = match candidate.wanted.source {
                        Source::Download(n) => {
                            PackEntry::Index(index.files[n].clone())
                        }
                        Source::Override(n) => PackEntry::Archive {
                            pack: sha1.clone(),
                            entry: archive
                                .by_index_raw(n)
                                .ok()
                                .and_then(|entry| {
                                    Some(entry.name().ok()?.into_owned())
                                })
                                .unwrap_or_default(),
                            size: candidate.wanted.size,
                        },
                    };
                    (candidate.path, candidate.wanted.sha1, source)
                })
                .collect::<Vec<_>>();
            Ok::<_, Error>(files)
        })
        .await
        .map_err(std::io::Error::other)??;
        let sha1s = files
            .iter()
            .map(|(_, sha1, _)| sha1.clone())
            .collect::<Vec<_>>();
        let known = self.0.modrinth.by_hashes(&sha1s).await?;
        Ok(files
            .into_iter()
            .map(|(path, sha1, source)| ModpackFile {
                version_id: known.get(&sha1).map(|version| version.id.clone()),
                sha1: Some(sha1),
                path,
                alias: None,
                source,
            })
            .collect())
    }
}

/// Fetch the file `name` from the first holder that serves it correctly, decrypted and checked
/// against both C and P, into `path`, as a member's App does.
async fn fetch(
    content: &Content,
    backend: &Backend,
    name: &str,
    file: &PoolFile,
    path: &Path,
) -> Result<(), Error> {
    pool::fetch(
        &content.store,
        backend,
        file,
        &|_| {},
        |mut reader| async move {
            let mut out = tokio::fs::File::create(path).await?;
            tokio::io::copy(&mut reader, &mut out).await?;
            out.sync_all().await
        },
    )
    .await
    .map_err(|error| {
        Error::Invalid(match error {
            FetchError::Sink(error) => {
                format!("Could not write {name}: {error}")
            }
            error => format!("Could not fetch {name}: {error}"),
        })
    })
}

/// Write `index`, the fetched files under `overrides/`, and the `copies` (an entry of the
/// `.mrpack` at a path, under the same name) as an `.mrpack` at `path`. Returns its SHA-1.
fn zip(
    path: &PathBuf,
    index: &[u8],
    files: &[(String, PathBuf)],
    copies: &[(PathBuf, String)],
) -> Result<String, Error> {
    use std::io::Write;

    let zip_error =
        |error: zip::result::ZipError| Error::Io(std::io::Error::other(error));
    // The files are mostly jars, already compressed.
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(true);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path)?);
    zip.start_file("modrinth.index.json", options)
        .map_err(zip_error)?;
    zip.write_all(index)?;
    for (name, fetched) in files {
        zip.start_file(format!("overrides/{name}"), options)
            .map_err(zip_error)?;
        std::io::copy(&mut std::fs::File::open(fetched)?, &mut zip)?;
    }
    let mut packs = HashMap::new();
    for (pack, entry) in copies {
        if !packs.contains_key(pack) {
            let opened = zip::ZipArchive::new(std::fs::File::open(pack)?)
                .map_err(zip_error)?;
            packs.insert(pack.clone(), opened);
        }
        let source = packs.get_mut(pack).expect("opened above");
        let file = source.by_name(entry).map_err(zip_error)?;
        zip.raw_copy_file(file).map_err(zip_error)?;
    }
    zip.finish().map_err(zip_error)?;
    Ok(crate::installs::download::sha1_of(std::fs::File::open(
        path,
    )?)?)
}
