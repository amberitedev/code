//! A share version as a pack. A server follows an App instance by applying versions of the
//! instance's share, like one more member. Core reads the share's latest version from the
//! backend with its own key, for the user who asked for the install, who must be able to read
//! the share and reach this Core: a Modrinth pack index for the files on Modrinth, and the pool
//! entries of the rest. It fetches those through the store API of the version's holders, as a
//! member's App does, decrypts and checks them, and writes an `.mrpack` with them as overrides,
//! which installs like any other pack.
//!
//! Core only ever asks a holder for a file through the store API, so where and how the pool
//! keeps files does not matter here.

use std::path::{Path, PathBuf};

use sharing_client::consume::{
    self, ConsumeError, ModpackFile, PoolAccess, PoolFile, ReadyVersion,
    ServerPackError,
};
use sharing_client::store::{FetchError, StoreClient};

use super::install_pack as install;
use crate::{
    installs::shared::SharedDirs,
    state::servers::{
        Content, Error,
        adapters::filesystem::archive::{self, NoRoom},
    },
};

/// Fetch the files of `version` (read with [`consume::ready_version`]) and write it as an
/// `.mrpack` named `name` in the shared store. Returns the pack's SHA-1 and the version.
pub async fn write_pack(
    content: &Content,
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
    let total: u64 = version.files.iter().map(|file| file.object_size).sum();
    let staging = content
        .paths
        .staging(server_id)
        .join(uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(&staging).await?;
    // The files, then the pack written from them.
    NoRoom::check(
        "the instance's files",
        total.saturating_mul(2),
        archive::room(&staging)?,
    )?;
    let result = async {
        let mut fetched = Vec::with_capacity(version.files.len());
        if !version.files.is_empty() {
            let pool = version.pool.as_ref().ok_or_else(|| {
                Error::Invalid(
                    "The version has files but nowhere to fetch them.".into(),
                )
            })?;
            let store =
                StoreClient::new(concat!("copal/", env!("CARGO_PKG_VERSION")));
            for (n, file) in version.files.iter().enumerate() {
                let path = staging.join(n.to_string());
                fetch(&store, pool, file, &path).await?;
                fetched.push((file.path.clone(), path));
            }
        }
        let dirs = SharedDirs::new(content.paths.root());
        let packed = staging.join("pack.mrpack");
        let index = serde_json::to_vec(&version.index)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let sha1 = {
            let packed = packed.clone();
            tokio::task::spawn_blocking(move || zip(&packed, &index, &fetched))
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

    async fn versions(
        &self,
        ids: &[String],
    ) -> Result<Vec<bedrock::content::SidedVersion>, Error> {
        Ok(self.0.modrinth.sided_versions(ids).await?)
    }

    async fn modpack_files(&self, id: &str) -> Result<Vec<ModpackFile>, Error> {
        let dirs = SharedDirs::new(self.0.paths.root());
        let sha1 =
            install::fetch_modrinth_pack(self.0, &dirs, id, &|_| {}).await?;
        let (_, split) = install::read_pack(
            self.0,
            &dirs.pack(&sha1),
            &Default::default(),
            false,
        )
        .await?;
        let sha1s = split
            .wanted
            .values()
            .map(|file| file.sha1.clone())
            .collect::<Vec<_>>();
        let known = self.0.modrinth.by_hashes(&sha1s).await?;
        Ok(split
            .wanted
            .into_iter()
            .map(|(path, file)| ModpackFile {
                version_id: known
                    .get(&file.sha1)
                    .map(|version| version.id.clone()),
                path,
            })
            .collect())
    }
}

/// Fetch one file from the first holder that serves it correctly, decrypted and checked against
/// both C and P, into `path`, as a member's App does.
async fn fetch(
    store: &StoreClient,
    pool: &PoolAccess,
    file: &PoolFile,
    path: &Path,
) -> Result<(), Error> {
    // The holders that served bad bytes are logged by the client; only members report them.
    consume::fetch(store, pool, file, |mut reader| async move {
        let mut out = tokio::fs::File::create(path).await?;
        tokio::io::copy(&mut reader, &mut out).await?;
        out.sync_all().await
    })
    .await
    .map_err(|error| {
        Error::Invalid(match error {
            ConsumeError::Fetch(FetchError::Sink(error)) => {
                format!("Could not write {}: {error}", file.path)
            }
            error => format!("Could not fetch {}: {error}", file.path),
        })
    })
}

/// Write `index` and the fetched files, under `overrides/`, as an `.mrpack` at `path`. Returns
/// its SHA-1.
fn zip(
    path: &PathBuf,
    index: &[u8],
    files: &[(String, PathBuf)],
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
    zip.finish().map_err(zip_error)?;
    Ok(crate::installs::download::sha1_of(std::fs::File::open(
        path,
    )?)?)
}
