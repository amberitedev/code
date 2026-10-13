use std::path::{Path, PathBuf};

use daedalus::minecraft::VersionInfo;
use serde::Deserialize;
use tracing::{info, warn};

use super::shared::{meta, version_info, SharedDirs};
pub use crate::domain::java::JavaInstall;

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

/// Probe the binary itself rather than assuming its name identifies its version.
pub fn detect_java_installations() -> Vec<JavaInstall> {
    let mut installs = Vec::new();
    for name in ["java25", "java21", "java17", "java8", "java"] {
        if let Ok(path) = which::which(name) {
            if let Some(version) = installed_java_version(&path) {
                installs.push(JavaInstall { version, path });
            }
        }
    }
    installs
}

pub fn installed_java_version(path: &Path) -> Option<u32> {
    let output = std::process::Command::new(path)
        .arg("-version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let version = text.split('"').nth(1)?;
    let mut parts = version.split(['.', '-', '_']);
    let major = parts.next()?.parse().ok()?;
    if major == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(major)
    }
}

/// Prefer cached Mojang metadata so an installed server can start offline.
pub async fn java_requirement(
    http: &reqwest::Client,
    data_dir: &Path,
    game_version: &str,
    loader_version: Option<&str>,
) -> (u32, &'static str) {
    let dirs = SharedDirs::new(data_dir);
    let mut ids = vec![game_version.to_string()];
    if let Some(loader) = loader_version {
        ids.push(format!("{game_version}-{loader}"));
    }
    for id in ids {
        if let Ok(bytes) = tokio::fs::read(dirs.version_info_path(&id)).await {
            if let Ok(info) = serde_json::from_slice::<VersionInfo>(&bytes) {
                if let Some(java) = info.java_version {
                    return (
                        java.major_version,
                        "cached Mojang javaVersion.majorVersion",
                    );
                }
            }
        }
    }
    let fetched =
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            let manifest = meta::fetch_minecraft_manifest(http).await.ok()?;
            let version =
                meta::find_game_version(&manifest, game_version).ok()?;
            version_info::download_version_info(
                http, &dirs, &version, None, false,
            )
            .await
            .ok()?
            .java_version
            .map(|java| java.major_version)
        })
        .await;
    if let Ok(Some(major)) = fetched {
        return (major, "Mojang javaVersion.majorVersion");
    }
    warn!(
        game_version,
        "Java metadata unavailable; using Minecraft version fallback"
    );
    (
        required_java_version(game_version),
        "Minecraft version fallback",
    )
}

/// Uses the same Azul Zulu ZIP packages as the launcher, owned by Core.
pub async fn download_java(
    http: &reqwest::Client,
    data_dir: &Path,
    version: u32,
) -> Result<PathBuf, String> {
    #[derive(Deserialize)]
    struct Package {
        download_url: String,
        sha256_hash: String,
    }

    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        "x86" => "x86",
        other => {
            return Err(format!("No Java download configured for {other}"))
        }
    };
    let url = format!("https://api.azul.com/metadata/v1/zulu/packages?arch={arch}&java_version={version}&os={}&archive_type=zip&javafx_bundled=false&java_package_type=jre&availability_types=CA&release_status=ga&latest=true&include_fields=sha256_hash&page_size=1", std::env::consts::OS);
    info!(version, "Downloading Java runtime from Azul");
    let packages: Vec<Package> = http
        .get(url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let package = packages.first().ok_or_else(|| {
        format!("No Java {version} runtime available for this platform")
    })?;
    let bytes = http
        .get(&package.download_url)
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    use sha2::Digest;
    if !hex::encode(sha2::Sha256::digest(&bytes))
        .eq_ignore_ascii_case(&package.sha256_hash)
    {
        return Err(format!("Java {version} download checksum mismatch"));
    }
    let parent = data_dir.join("java");
    tokio::fs::create_dir_all(&parent)
        .await
        .map_err(|e| e.to_string())?;
    let destination = parent.join(version.to_string());
    tokio::task::spawn_blocking(move || {
        let temp = tempfile::tempdir_in(&parent).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|e| e.to_string())?;
        archive.extract(temp.path()).map_err(|e| e.to_string())?;
        let binary = if cfg!(windows) { "java.exe" } else { "java" };
        let path = walkdir::WalkDir::new(temp.path())
            .into_iter()
            .filter_map(Result::ok)
            .find(|entry| {
                entry.file_type().is_file()
                    && entry.file_name() == binary
                    && entry.path().parent().is_some_and(|p| p.ends_with("bin"))
            })
            .ok_or_else(|| "Java archive contains no runtime".to_string())?
            .into_path();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                &path,
                std::fs::Permissions::from_mode(0o755),
            )
            .map_err(|e| e.to_string())?;
        }
        if installed_java_version(&path) != Some(version) {
            return Err(format!("Downloaded runtime is not Java {version}"));
        }
        let relative = path
            .strip_prefix(temp.path())
            .map_err(|e| e.to_string())?
            .to_path_buf();
        if destination.exists() {
            std::fs::remove_dir_all(&destination).map_err(|e| e.to_string())?;
        }
        std::fs::rename(temp.path(), &destination)
            .map_err(|e| e.to_string())?;
        Ok(destination.join(relative))
    })
    .await
    .map_err(|e| e.to_string())?
}
