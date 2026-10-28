//! Shared library download + classpath construction.
//!
//! Port of app-lib's `launcher::download::download_libraries` and
//! `launcher::args` classpath helpers, specialised for the server:
//!
//! * Each library is written by its maven path into the ONE shared
//!   `libraries/` tree and deduped, so a library is downloaded at most once
//!   across every installation. Repair re-hashes what is there and replaces
//!   only what does not match, whole, never deleting a file a server may use.
//! * Natives are skipped entirely: a dedicated server never loads LWJGL/GLFW
//!   natives, so we neither download nor classpath them.
//! * Classpaths are built from absolute, canonicalised paths so the JVM finds
//!   every jar regardless of the process working directory (which stays the
//!   per-instance data dir).

use std::{
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

use dunce::canonicalize;
use futures::stream::{self, TryStreamExt};
use minecraft_meta_manifest::{
    minecraft::{Library, LibraryDownloads},
    util::get_path_from_artifact,
};

use super::dirs::SharedDirs;
use super::rules::{classpath_separator, native_arch, parse_rules};
use crate::installs::download::{self, FileDownload, Hash};

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Download(#[from] download::DownloadError),
    #[error("maven path: {0}")]
    Maven(String),
}

/// Number of libraries downloaded concurrently.
const CONCURRENCY: usize = 8;

/// Download every applicable library into the shared `libraries/` store,
/// skipping natives and anything excluded by host OS rules. Existing files are
/// kept; with `verify` (repair) they are re-hashed and replaced when they do
/// not match. `progress` is told the share of libraries done.
pub async fn download_libraries(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    libraries: &[Library],
    java_arch: &str,
    verify: bool,
    progress: &(dyn Fn(f64) + Sync),
) -> Result<(), LibraryError> {
    tokio::fs::create_dir_all(dirs.libraries_dir()).await?;
    let done = AtomicUsize::new(0);
    let total = libraries.len().max(1);

    stream::iter(libraries.iter().map(Ok::<&Library, LibraryError>))
        .try_for_each_concurrent(CONCURRENCY, |library| {
            let done = &done;
            async move {
                download_one(http, dirs, library, java_arch, verify).await?;
                let done = done.fetch_add(1, Ordering::Relaxed) + 1;
                progress(done as f64 / total as f64);
                Ok(())
            }
        })
        .await
}

async fn download_one(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    library: &Library,
    java_arch: &str,
    verify: bool,
) -> Result<(), LibraryError> {
    if let Some(rules) = &library.rules
        && !parse_rules(rules, java_arch)
    {
        return Ok(());
    }

    if !library.downloadable {
        return Ok(());
    }

    // Servers don't use natives — skip any natives-only library outright.
    if natives_only(library, java_arch) {
        return Ok(());
    }

    let artifact_path = maven_path(&library.name)?;
    let path = dirs.libraries_dir().join(&artifact_path);

    // Prefer the explicit artifact download (with sha1), else fall back to a
    // constructed maven URL (best-effort, matching app-lib/PrismLauncher).
    if let Some(LibraryDownloads {
        artifact: Some(artifact),
        ..
    }) = &library.downloads
        && let Some(url) = &artifact.url
    {
        let sha1 = artifact.sha1.to_string();
        if path.exists()
            && (!verify || download::has_hash(&path, Hash::Sha1(&sha1)).await)
        {
            return Ok(());
        }
        download::file(
            http,
            url.as_str(),
            &path,
            &FileDownload {
                hash: Hash::Sha1(&sha1),
                limit: None,
                progress: &download::quiet,
            },
        )
        .await?;
        return Ok(());
    }

    if path.exists() {
        return Ok(());
    }
    let base = library
        .url
        .as_deref()
        .unwrap_or("https://libraries.minecraft.net/");
    let url = format!("{base}{artifact_path}");
    let fetched = download::file(
        http,
        &url,
        &path,
        &FileDownload {
            hash: Hash::Unknown,
            limit: None,
            progress: &download::quiet,
        },
    )
    .await;
    if let Err(e) = fetched {
        tracing::debug!(
            "optional library {} could not be fetched from {url}: {e}",
            library.name
        );
    }
    Ok(())
}

/// Whether a library only carries natives for this machine, which a dedicated server never
/// loads. app-lib's `LibraryExt::native_classifier`.
fn natives_only(library: &Library, java_arch: &str) -> bool {
    library
        .natives
        .as_ref()
        .is_some_and(|natives| natives.contains_key(&native_arch(java_arch)))
}

/// A library's path under the `libraries/` tree, from its maven coordinates.
pub fn maven_path(name: &str) -> Result<String, LibraryError> {
    get_path_from_artifact(name)
        .map_err(|error| LibraryError::Maven(format!("{name}: {error:#}")))
}

/// Absolute path of a single library jar inside the shared store.
fn lib_path(libraries_dir: &Path, name: &str) -> Result<String, LibraryError> {
    let joined = libraries_dir.join(maven_path(name)?);
    Ok(canonicalize(&joined)?.to_string_lossy().to_string())
}

/// Build a classpath string from the merged libraries plus any extra entries
/// (e.g. the shared server jar). All paths are absolute. Natives-only and
/// rule-excluded or non-classpath libraries are dropped.
pub fn build_classpath(
    libraries_dir: &Path,
    libraries: &[Library],
    extra: &[&Path],
    java_arch: &str,
) -> Result<String, LibraryError> {
    let mut entries: Vec<String> = Vec::new();

    for path in extra {
        let abs = canonicalize(path).map_err(LibraryError::Io)?;
        entries.push(abs.to_string_lossy().to_string());
    }

    for library in libraries {
        if let Some(rules) = &library.rules
            && !parse_rules(rules, java_arch)
        {
            continue;
        }
        if !library.include_in_classpath {
            continue;
        }
        if natives_only(library, java_arch) {
            continue;
        }
        entries.push(lib_path(libraries_dir, &library.name)?);
    }

    let sep = classpath_separator(java_arch);
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<String> = entries
        .into_iter()
        .filter(|e| seen.insert(e.clone()))
        .collect();
    Ok(unique.join(sep))
}
