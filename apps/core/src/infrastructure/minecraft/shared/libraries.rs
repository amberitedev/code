//! Shared library download + classpath construction.
//!
//! Port of app-lib's `launcher::download::download_libraries` and
//! `launcher::args` classpath helpers, specialised for the server:
//!
//! * Each library is written by its maven path into the ONE shared
//!   `libraries/` tree and deduped — `if path.exists() && !force { return }` —
//!   so a library is downloaded at most once across every installation.
//! * Natives are skipped entirely: a dedicated server never loads LWJGL/GLFW
//!   natives, so we neither download nor classpath them.
//! * Classpaths are built from absolute, canonicalised paths so the JVM finds
//!   every jar regardless of the process working directory (which stays the
//!   per-instance data dir).

use std::path::Path;

use daedalus::get_path_from_artifact;
use daedalus::minecraft::{Library, LibraryDownloads};
use dunce::canonicalize;
use futures::stream::{self, TryStreamExt};
use sha1::Digest;

use super::dirs::SharedDirs;
use super::rules::{classpath_separator, parse_rules};

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("maven path: {0}")]
    Maven(String),
    #[error("sha1 mismatch for {name}: expected {expected}, got {actual}")]
    HashMismatch {
        name: String,
        expected: String,
        actual: String,
    },
}

/// Number of libraries downloaded concurrently.
const CONCURRENCY: usize = 8;

/// Download every applicable library into the shared `libraries/` store,
/// skipping natives and anything excluded by host OS rules. Existing files are
/// left untouched unless `force` is set.
pub async fn download_libraries(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    libraries: &[Library],
    java_arch: &str,
    force: bool,
) -> Result<(), LibraryError> {
    tokio::fs::create_dir_all(dirs.libraries_dir()).await?;

    stream::iter(libraries.iter().map(Ok::<&Library, LibraryError>))
        .try_for_each_concurrent(CONCURRENCY, |library| async move {
            download_one(http, dirs, library, java_arch, force).await
        })
        .await
}

async fn download_one(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    library: &Library,
    java_arch: &str,
    force: bool,
) -> Result<(), LibraryError> {
    if let Some(rules) = &library.rules {
        if !parse_rules(rules, java_arch) {
            return Ok(());
        }
    }

    if !library.downloadable {
        return Ok(());
    }

    // Servers don't use natives — skip any natives-only library outright.
    if library.natives_os_key_and_classifiers(java_arch).is_some() {
        return Ok(());
    }

    let artifact_path = get_path_from_artifact(&library.name)
        .map_err(|e| LibraryError::Maven(format!("{}: {e}", library.name)))?;
    let path = dirs.libraries_dir().join(&artifact_path);

    if path.exists() && !force {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Prefer the explicit artifact download (with sha1), else fall back to a
    // constructed maven URL (best-effort, matching app-lib/PrismLauncher).
    if let Some(LibraryDownloads {
        artifact: Some(artifact),
        ..
    }) = &library.downloads
    {
        if !artifact.url.is_empty() {
            let bytes = fetch_verified(
                http,
                &artifact.url,
                Some(&artifact.sha1),
                &library.name,
            )
            .await?;
            tokio::fs::write(&path, &bytes).await?;
            return Ok(());
        }
    }

    let base = library
        .url
        .as_deref()
        .unwrap_or("https://libraries.minecraft.net/");
    let url = format!("{base}{artifact_path}");
    match fetch_verified(http, &url, None, &library.name).await {
        Ok(bytes) => {
            tokio::fs::write(&path, &bytes).await?;
        }
        Err(e) => {
            tracing::debug!(
                "optional library {} could not be fetched from {url}: {e}",
                library.name
            );
        }
    }
    Ok(())
}

async fn fetch_verified(
    http: &reqwest::Client,
    url: &str,
    sha1: Option<&str>,
    name: &str,
) -> Result<bytes::Bytes, LibraryError> {
    let bytes = http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if let Some(expected) = sha1 {
        let actual = hex::encode(sha1::Sha1::digest(&bytes));
        if !expected.is_empty() && actual != expected {
            return Err(LibraryError::HashMismatch {
                name: name.to_string(),
                expected: expected.to_string(),
                actual,
            });
        }
    }
    Ok(bytes)
}

/// Absolute path of a single library jar inside the shared store. When
/// `allow_not_exist` is set, a missing file yields the (non-canonical) joined
/// path rather than an error — used for processor classpaths that reference a
/// jar produced by an earlier processor step.
pub fn lib_path(
    libraries_dir: &Path,
    name: &str,
    allow_not_exist: bool,
) -> Result<String, LibraryError> {
    let joined = libraries_dir.join(
        get_path_from_artifact(name)
            .map_err(|e| LibraryError::Maven(format!("{name}: {e}")))?,
    );
    let resolved = match canonicalize(&joined) {
        Ok(p) => p,
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound && allow_not_exist =>
        {
            joined
        }
        Err(e) => {
            return Err(LibraryError::Io(e));
        }
    };
    Ok(resolved.to_string_lossy().to_string())
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
        if let Some(rules) = &library.rules {
            if !parse_rules(rules, java_arch) {
                continue;
            }
        }
        if !library.include_in_classpath {
            continue;
        }
        if library.natives_os_key_and_classifiers(java_arch).is_some() {
            continue;
        }
        entries.push(lib_path(libraries_dir, &library.name, false)?);
    }

    let sep = classpath_separator(java_arch);
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<String> = entries
        .into_iter()
        .filter(|e| seen.insert(e.clone()))
        .collect();
    Ok(unique.join(sep))
}

/// Classpath built from a list of maven coordinates (used for processor
/// invocations). Missing jars are allowed since some are produced mid-run.
pub fn classpath_from_artifacts(
    libraries_dir: &Path,
    artifacts: &[String],
    java_arch: &str,
) -> Result<String, LibraryError> {
    let cps = artifacts
        .iter()
        .map(|a| lib_path(libraries_dir, a, true))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(cps.join(classpath_separator(java_arch)))
}
