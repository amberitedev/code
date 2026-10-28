//! The Java runtimes Core manages: Azul Zulu JREs, checked against the SHA-256 Azul publishes, in
//! `java/<major>/<release>/`, shared by every server on that major. `java/<major>/current.json`
//! names the release new starts use. A newer patch is installed beside the old one, so a running
//! server never loses the Java it runs; the old release goes once nothing runs it.
//!
//! Core never runs these itself: servers and installers run them as `copal-servers`.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex},
};

use minecraft_meta_manifest::minecraft::VersionManifest;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::{
    download::{self, FileDownload, Hash},
    shared::{SharedDirs, meta, version_info},
};
use crate::{daemon::paths::Paths, servers::shim};

#[derive(Debug, thiserror::Error)]
pub enum JavaError {
    #[error("Azul has no Java {0} for this machine")]
    Unavailable(u32),
    #[error(transparent)]
    Download(#[from] download::DownloadError),
    #[error("the Java {major} archive is unusable: {reason}")]
    Archive { major: u32, reason: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Determine the required Java major version for a given Minecraft version.
/// Fallback when Mojang's javaVersion metadata is unavailable.
pub fn required_java_version(game_version: &str) -> u32 {
    let release = game_version
        .split(['-', '+'])
        .next()
        .unwrap_or(game_version);
    let parts: Vec<u32> = release
        .split('.')
        .take(3)
        .filter_map(|s| s.parse().ok())
        .collect();
    let minor = parts.get(1).copied().unwrap_or(0);
    let major = parts.first().copied().unwrap_or(0);
    let patch = parts.get(2).copied().unwrap_or(0);
    if major >= 26 {
        25
    } else if minor > 20 || (minor == 20 && patch >= 5) {
        21
    } else if minor >= 17 {
        17
    } else {
        8
    }
}

/// The Java major a Minecraft version needs. Cached Mojang metadata first, so an installed
/// server starts offline.
pub async fn java_requirement(
    http: &reqwest::Client,
    data_dir: &Path,
    game_version: &str,
) -> u32 {
    let dirs = SharedDirs::new(data_dir);
    if let Ok(bytes) =
        tokio::fs::read(dirs.version_info_path(game_version)).await
        && let Ok(info) = serde_json::from_slice::<VersionManifest>(&bytes)
        && let Some(java) = info.java_version
    {
        return java.major_version;
    }
    let fetched = async {
        let catalog = meta::fetch_minecraft_manifest(http).await.ok()?;
        let version = meta::find_game_version(&catalog, game_version).ok()?;
        version_info::vanilla(http, &dirs, &version, false)
            .await
            .ok()?
            .java_version
            .map(|java| java.major_version)
    }
    .await;
    fetched.unwrap_or_else(|| {
        warn!(
            game_version,
            "Java metadata unavailable; using the Minecraft version fallback"
        );
        required_java_version(game_version)
    })
}

/// `java/<major>/current.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Current {
    /// The folder in `java/<major>/` new starts use.
    release: String,
    /// The binary, relative to the release folder.
    binary: PathBuf,
    /// Of the archive it came from, to tell a newer patch apart.
    sha256: String,
}

fn current_path(paths: &Paths, major: u32) -> PathBuf {
    paths.java().join(major.to_string()).join("current.json")
}

fn read_current(paths: &Paths, major: u32) -> Option<Current> {
    let text = std::fs::read_to_string(current_path(paths, major)).ok()?;
    serde_json::from_str(&text).ok()
}

/// The installed Java binary for `major`, if any.
pub fn installed(paths: &Paths, major: u32) -> Option<PathBuf> {
    let current = read_current(paths, major)?;
    let binary = paths
        .java()
        .join(major.to_string())
        .join(&current.release)
        .join(&current.binary);
    binary.is_file().then_some(binary)
}

/// One install at a time per major, so two servers starting on a missing major, or a start and
/// the patch check, never unpack the same release over each other.
fn installing(major: u32) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: LazyLock<Mutex<HashMap<u32, Arc<tokio::sync::Mutex<()>>>>> =
        LazyLock::new(Mutex::default);
    Arc::clone(
        LOCKS
            .lock()
            .expect("not poisoned")
            .entry(major)
            .or_default(),
    )
}

/// Java binaries a start or an installer resolved and has not handed to its shim yet, with how
/// many hold each. The patch check never removes a release one of them is in.
static LEASES: LazyLock<Mutex<HashMap<PathBuf, usize>>> =
    LazyLock::new(Mutex::default);

/// A Java binary kept from removal until dropped. Hold it until the shim that runs it is
/// running: from then on its launch spec keeps the release.
#[must_use]
pub struct Lease {
    binary: PathBuf,
}

impl Drop for Lease {
    fn drop(&mut self) {
        let mut leases = LEASES.lock().expect("not poisoned");
        if let Some(count) = leases.get_mut(&self.binary) {
            *count -= 1;
            if *count == 0 {
                leases.remove(&self.binary);
            }
        }
    }
}

/// Lease `binary`, unless it is gone. Checked under the lease lock, which removal also holds, so
/// a release is either removed before this or kept.
fn lease(binary: &Path) -> Option<Lease> {
    let mut leases = LEASES.lock().expect("not poisoned");
    if !binary.is_file() {
        return None;
    }
    *leases.entry(binary.to_owned()).or_default() += 1;
    Some(Lease {
        binary: binary.to_owned(),
    })
}

/// The Java binary for `major`, downloaded first when missing, leased (see [`Lease`]).
pub async fn ensure(
    paths: &Paths,
    http: &reqwest::Client,
    major: u32,
) -> Result<(PathBuf, Lease), JavaError> {
    let leased = |binary: PathBuf| lease(&binary).map(|lease| (binary, lease));
    if let Some(found) = installed(paths, major).and_then(leased) {
        return Ok(found);
    }
    let lock = installing(major);
    let _installing = lock.lock().await;
    // Another start may have installed it while this one waited.
    if let Some(found) = installed(paths, major).and_then(leased) {
        return Ok(found);
    }
    let package = latest(http, major).await?;
    let binary = install(paths, http, major, &package).await?;
    // Removal runs under the lock this holds, so the new release is there.
    leased(binary).ok_or_else(|| JavaError::Archive {
        major,
        reason: "its binary vanished once installed".into(),
    })
}

/// One Azul package, as their metadata API lists it.
#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    download_url: String,
    sha256_hash: String,
}

/// The package Azul is asked for, as its file name ends. Windows gets the ZIP; Linux and macOS get
/// the tar.gz, the only one Azul publishes for every architecture there (Linux arm64 has no ZIP)
/// and the one that keeps file modes and links.
const ARCHIVE: &str = if cfg!(windows) { ".zip" } else { ".tar.gz" };

async fn latest(
    http: &reqwest::Client,
    major: u32,
) -> Result<Package, JavaError> {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        "x86" => "x86",
        _ => return Err(JavaError::Unavailable(major)),
    };
    let os = match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        _ => "linux",
    };
    let archive_type = ARCHIVE.trim_start_matches('.');
    // Without `crac_supported=false` the tar.gz answer can be a CRaC build.
    let url = format!(
        "https://api.azul.com/metadata/v1/zulu/packages?arch={arch}&java_version={major}&os={os}&archive_type={archive_type}&javafx_bundled=false&crac_supported=false&java_package_type=jre&availability_types=CA&release_status=ga&latest=true&include_fields=sha256_hash&page_size=1"
    );
    let packages: Vec<Package> = download::json(http, &url).await?;
    packages
        .into_iter()
        .next()
        .ok_or(JavaError::Unavailable(major))
}

/// Point `current.json` at the published release `release` of `major`, if it is there. A
/// release folder appears only whole (it is renamed into place once unpacked) and is never
/// changed after, so one that is there is used as it is.
fn use_published(
    paths: &Paths,
    major: u32,
    release: &str,
    sha256: &str,
) -> std::io::Result<Option<PathBuf>> {
    let folder = paths.java().join(major.to_string()).join(release);
    if !folder.is_dir() {
        return Ok(None);
    }
    let Some(binary) = find_binary(&folder) else {
        return Ok(None);
    };
    shim::write_json(
        &current_path(paths, major),
        &Current {
            release: release.to_owned(),
            binary: binary.clone(),
            sha256: sha256.to_owned(),
        },
    )?;
    Ok(Some(folder.join(binary)))
}

/// The Java binary in an unpacked release, relative to it.
fn find_binary(release: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "java.exe" } else { "java" };
    let found = walkdir::WalkDir::new(release)
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| {
            entry.file_type().is_file()
                && entry.file_name() == name
                && entry.path().parent().is_some_and(|p| p.ends_with("bin"))
        })?;
    Some(found.path().strip_prefix(release).ok()?.to_path_buf())
}

/// Download and unpack `package` beside any older release, then point `current.json` at it. A
/// release already published is reused, never replaced: a server may run from it.
async fn install(
    paths: &Paths,
    http: &reqwest::Client,
    major: u32,
    package: &Package,
) -> Result<PathBuf, JavaError> {
    let release = package.name.trim_end_matches(ARCHIVE).to_owned();
    if let Some(binary) =
        use_published(paths, major, &release, &package.sha256_hash)?
    {
        return Ok(binary);
    }
    info!(major, release = %package.name, "installing Java");
    let folder = paths.java().join(major.to_string());
    tokio::fs::create_dir_all(&folder).await?;
    let archive = tempfile::Builder::new()
        .suffix(ARCHIVE)
        .tempfile_in(&folder)?
        .into_temp_path();
    download::file(
        http,
        &package.download_url,
        &archive,
        &FileDownload {
            hash: Hash::Sha256(&package.sha256_hash),
            limit: Some(1024 * 1024 * 1024),
            progress: &download::quiet,
        },
    )
    .await?;
    let sha256 = package.sha256_hash.clone();
    let paths = paths.clone();
    tokio::task::spawn_blocking(move || {
        let unusable = |reason: String| JavaError::Archive { major, reason };
        let staged = tempfile::tempdir_in(&folder)?;
        #[cfg(windows)]
        zip::ZipArchive::new(std::fs::File::open(&archive)?)
            .and_then(|mut zip| zip.extract(staged.path()))
            .map_err(|error| unusable(error.to_string()))?;
        // tar 0.4.45 or later: an older one could change modes outside `staged` (GHSA-j4xf-2g29-59ph).
        #[cfg(unix)]
        tar::Archive::new(flate2::read::GzDecoder::new(std::fs::File::open(
            &archive,
        )?))
        .unpack(staged.path())
        .map_err(|error| unusable(error.to_string()))?;
        let binary = find_binary(staged.path())
            .ok_or_else(|| unusable("it holds no Java binary".into()))?;
        executable(staged.path())?;
        let destination = folder.join(&release);
        // Published releases are never replaced; the lock means none appeared meanwhile.
        std::fs::rename(staged.keep(), &destination)?;
        #[cfg(unix)]
        {
            // The staging folder was private; servers run this as `copal-servers`, whose group
            // owns `java/`.
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                &destination,
                std::fs::Permissions::from_mode(0o2750),
            )?;
        }
        let current = Current {
            release,
            binary: binary.clone(),
            sha256,
        };
        shim::write_json(&current_path(&paths, major), &current)?;
        Ok(destination.join(binary))
    })
    .await
    .map_err(std::io::Error::other)?
}

/// Unix: what Azul's archive marks executable may have lost its mode on the way out.
#[cfg_attr(windows, allow(unused_variables))]
fn executable(root: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
    {
        use std::os::unix::fs::PermissionsExt;
        let in_bin = entry.path().parent().is_some_and(|p| p.ends_with("bin"));
        let helper = entry.file_name() == "jspawnhelper";
        if entry.file_type().is_file() && (in_bin || helper) {
            std::fs::set_permissions(
                entry.path(),
                std::fs::Permissions::from_mode(0o755),
            )?;
        }
    }
    Ok(())
}

/// Look for a newer patch of every installed Java, then remove releases nothing uses. Runs at
/// start; a failure is only logged.
pub async fn update_all(paths: &Paths, http: &reqwest::Client) {
    let Ok(entries) = std::fs::read_dir(paths.java()) else {
        return;
    };
    let majors: Vec<u32> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
        .collect();
    for major in majors {
        let lock = installing(major);
        let _installing = lock.lock().await;
        let Some(current) = read_current(paths, major) else {
            continue;
        };
        match latest(http, major).await {
            Ok(package)
                if !package
                    .sha256_hash
                    .eq_ignore_ascii_case(&current.sha256) =>
            {
                if let Err(error) = install(paths, http, major, &package).await
                {
                    warn!(major, %error, "could not update Java");
                }
            }
            Ok(_) => {}
            Err(error) => {
                warn!(major, %error, "could not check for a newer Java")
            }
        }
        remove_unused(paths, major);
    }
}

/// Remove every release of `major` but the current one, those a running server started with, and
/// those a start or installer holds a [`Lease`] on.
fn remove_unused(paths: &Paths, major: u32) {
    let Some(current) = read_current(paths, major) else {
        return;
    };
    let folder = paths.java().join(major.to_string());
    let in_use: Vec<PathBuf> = shim::running(paths)
        .iter()
        .filter_map(|id| {
            shim::read_versioned::<shim::Launch>(&paths.launch_spec(id)).ok()
        })
        .map(|launch| launch.java)
        .collect();
    let Ok(entries) = std::fs::read_dir(&folder) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name();
        // Held across the check and the removal, so no lease is taken in between.
        let leases = LEASES.lock().expect("not poisoned");
        if !path.is_dir()
            || name.to_str() == Some(current.release.as_str())
            || in_use.iter().any(|java| java.starts_with(&path))
            || leases.keys().any(|java| java.starts_with(&path))
        {
            continue;
        }
        info!(major, release = ?name, "removing an unused Java");
        if let Err(error) = std::fs::remove_dir_all(&path) {
            warn!(major, %error, "could not remove an unused Java");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_without_metadata_fall_back_by_number() {
        assert_eq!(required_java_version("1.12.2"), 8);
        assert_eq!(required_java_version("1.18.2"), 17);
        assert_eq!(required_java_version("1.20.4"), 17);
        assert_eq!(required_java_version("1.20.5"), 21);
        assert_eq!(required_java_version("1.21.1"), 21);
        assert_eq!(required_java_version("26.1"), 25);
    }

    #[test]
    fn a_published_release_is_reused_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let name = if cfg!(windows) { "java.exe" } else { "java" };
        let bin = paths.java().join("21").join("zulu21").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(name), "running").unwrap();
        let binary =
            use_published(&paths, 21, "zulu21", "abc").unwrap().unwrap();
        assert_eq!(binary, bin.join(name));
        assert_eq!(std::fs::read_to_string(&binary).unwrap(), "running");
        assert_eq!(installed(&paths, 21), Some(bin.join(name)));
        assert_eq!(use_published(&paths, 21, "zulu22", "abc").unwrap(), None);
    }

    #[test]
    fn only_releases_nothing_uses_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let folder = paths.java().join("21");
        for release in ["old", "new", "running"] {
            std::fs::create_dir_all(folder.join(release).join("bin")).unwrap();
        }
        shim::write_json(
            &current_path(&paths, 21),
            &Current {
                release: "new".into(),
                binary: "bin/java".into(),
                sha256: String::new(),
            },
        )
        .unwrap();
        // A server runs on the release before.
        let _lock = shim::try_lock(&paths, "a").unwrap().unwrap();
        shim::write_json(
            &paths.launch_spec("a"),
            &shim::Launch {
                v: shim::PROTOCOL,
                run_id: "r".into(),
                java: folder.join("running").join("bin").join("java"),
                args: Vec::new(),
                dir: dir.path().into(),
                memory_limit_mb: 0,
            },
        )
        .unwrap();

        remove_unused(&paths, 21);
        assert!(!folder.join("old").exists());
        assert!(folder.join("new").exists());
        assert!(folder.join("running").exists());
    }

    #[test]
    fn a_release_a_start_has_leased_is_kept_until_the_lease_ends() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let folder = paths.java().join("17");
        for release in ["starting", "new"] {
            std::fs::create_dir_all(folder.join(release).join("bin")).unwrap();
            std::fs::write(folder.join(release).join("bin/java"), "").unwrap();
        }
        shim::write_json(
            &current_path(&paths, 17),
            &Current {
                release: "new".into(),
                binary: "bin/java".into(),
                sha256: String::new(),
            },
        )
        .unwrap();
        // A start resolved the release before the patch check published "new".
        let leased = lease(&folder.join("starting").join("bin/java")).unwrap();
        remove_unused(&paths, 17);
        assert!(folder.join("starting").exists());
        drop(leased);
        remove_unused(&paths, 17);
        assert!(!folder.join("starting").exists());
        // A release already removed cannot be leased.
        assert!(lease(&folder.join("starting").join("bin/java")).is_none());
    }
}
