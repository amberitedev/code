//! The Java runtimes Core manages: Azul Zulu JREs, checked against the SHA-256 Azul publishes, in
//! `java/<major>/<release>/`, shared by every server on that major. `java/<major>/current.json`
//! names the release new starts use. A newer patch is installed beside the old one, so a running
//! server never loses the Java it runs; the old release goes once nothing runs it.
//!
//! Core never runs these itself: servers and installers run them as `copal-servers`.

use std::path::{Path, PathBuf};

use daedalus::minecraft::VersionInfo;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::{
    daedalus::{SharedDirs, meta, version_info},
    download::{self, FileDownload, Hash},
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
        && let Ok(info) = serde_json::from_slice::<VersionInfo>(&bytes)
        && let Some(java) = info.java_version
    {
        return java.major_version;
    }
    let fetched = async {
        let manifest = meta::fetch_minecraft_manifest(http).await.ok()?;
        let version = meta::find_game_version(&manifest, game_version).ok()?;
        version_info::download_version_info(http, &dirs, &version, None, false)
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

/// The Java binary for `major`, downloaded first when missing.
pub async fn ensure(
    paths: &Paths,
    http: &reqwest::Client,
    major: u32,
) -> Result<PathBuf, JavaError> {
    if let Some(binary) = installed(paths, major) {
        return Ok(binary);
    }
    let package = latest(http, major).await?;
    install(paths, http, major, &package).await
}

/// One Azul package, as their metadata API lists it.
#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    download_url: String,
    sha256_hash: String,
}

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
    let url = format!(
        "https://api.azul.com/metadata/v1/zulu/packages?arch={arch}&java_version={major}&os={os}&archive_type=zip&javafx_bundled=false&java_package_type=jre&availability_types=CA&release_status=ga&latest=true&include_fields=sha256_hash&page_size=1"
    );
    let packages: Vec<Package> = download::json(http, &url).await?;
    packages
        .into_iter()
        .next()
        .ok_or(JavaError::Unavailable(major))
}

/// Download and unpack `package` beside any older release, then point `current.json` at it.
async fn install(
    paths: &Paths,
    http: &reqwest::Client,
    major: u32,
    package: &Package,
) -> Result<PathBuf, JavaError> {
    info!(major, release = %package.name, "installing Java");
    let folder = paths.java().join(major.to_string());
    tokio::fs::create_dir_all(&folder).await?;
    let archive = tempfile::Builder::new()
        .suffix(".zip")
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
    let release = package.name.trim_end_matches(".zip").to_owned();
    let sha256 = package.sha256_hash.clone();
    let paths = paths.clone();
    tokio::task::spawn_blocking(move || {
        let unusable = |reason: String| JavaError::Archive { major, reason };
        let staged = tempfile::tempdir_in(&folder)?;
        zip::ZipArchive::new(std::fs::File::open(&archive)?)
            .and_then(|mut zip| zip.extract(staged.path()))
            .map_err(|error| unusable(error.to_string()))?;
        let binary_name = if cfg!(windows) { "java.exe" } else { "java" };
        let binary = walkdir::WalkDir::new(staged.path())
            .into_iter()
            .filter_map(Result::ok)
            .find(|entry| {
                entry.file_type().is_file()
                    && entry.file_name() == binary_name
                    && entry.path().parent().is_some_and(|p| p.ends_with("bin"))
            })
            .ok_or_else(|| unusable("it holds no Java binary".into()))?
            .into_path();
        let binary = binary
            .strip_prefix(staged.path())
            .expect("found inside")
            .to_path_buf();
        executable(staged.path())?;
        let destination = folder.join(&release);
        if destination.exists() {
            std::fs::remove_dir_all(&destination)?;
        }
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

/// Remove every release of `major` but the current one and those a running server started with.
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
        if !path.is_dir()
            || name.to_str() == Some(current.release.as_str())
            || in_use.iter().any(|java| java.starts_with(&path))
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
}
