//! Forge and NeoForge through the vendor installer. Core fetches the installer into the shared
//! store; the server's shim runs it as `copal-servers` inside the server folder, because it runs
//! code (see `Servers::run_installer`). What it leaves behind says how to start the server.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{
    ModLoader,
    daedalus::{SharedDirs, meta},
    download::{self, FileDownload, Hash},
};
use crate::servers::launch::read_server_file;

#[derive(Debug, thiserror::Error)]
pub enum InstallerError {
    #[error(transparent)]
    Download(#[from] download::DownloadError),
    #[error(transparent)]
    Metadata(#[from] meta::MetaError),
    #[error("Forge has no build for Minecraft {0}")]
    NoBuild(String),
    #[error(
        "the {0} installer finished but left nothing to start the server with"
    )]
    NoLaunch(ModLoader),
}

/// The loader version to install: `requested`, or the recommended (then latest) Forge build, or
/// the newest NeoForge build for `game_version`.
pub async fn resolve(
    http: &reqwest::Client,
    loader: ModLoader,
    game_version: &str,
    requested: Option<&str>,
) -> Result<String, InstallerError> {
    let requested =
        requested.filter(|version| !matches!(*version, "latest" | "stable"));
    if let Some(version) = requested {
        return Ok(version.to_owned());
    }
    if loader == ModLoader::NeoForge {
        let manifest = meta::fetch_loader_manifest(http, "neo").await?;
        return Ok(meta::resolve_loader_version(
            &manifest,
            game_version,
            None,
        )?
        .id);
    }
    #[derive(Deserialize)]
    struct Promotions {
        promos: std::collections::HashMap<String, String>,
    }
    let promotions: Promotions = download::json(
        http,
        "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
    )
    .await?;
    let promoted = |kind| {
        promotions
            .promos
            .get(&format!("{game_version}-{kind}"))
            .cloned()
    };
    promoted("recommended")
        .or_else(|| promoted("latest"))
        .ok_or_else(|| InstallerError::NoBuild(game_version.into()))
}

/// Fetch the installer for `version` into the shared store, unless it is there. Checked against
/// the SHA-1 the maven publishes beside it, when it does.
pub async fn installer(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    loader: ModLoader,
    game_version: &str,
    version: &str,
) -> Result<PathBuf, InstallerError> {
    let (name, url) = match loader {
        ModLoader::NeoForge => {
            let name = format!("neoforge-{version}-installer.jar");
            let url = format!(
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/{name}"
            );
            (name, url)
        }
        _ => {
            let name = format!("forge-{game_version}-{version}-installer.jar");
            let url = format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{game_version}-{version}/{name}"
            );
            (name, url)
        }
    };
    let path = dirs.installer(&name);
    let sha1 = download::bytes(http, &format!("{url}.sha1"))
        .await
        .ok()
        .and_then(|text| {
            let text = String::from_utf8_lossy(&text);
            let hash = text.split_whitespace().next()?.to_owned();
            (hash.len() == 40).then_some(hash)
        });
    let hash = sha1.as_deref().map_or(Hash::Unknown, Hash::Sha1);
    if !download::has_hash(&path, hash).await {
        download::file(
            http,
            &url,
            &path,
            &FileDownload {
                hash,
                limit: Some(256 * 1024 * 1024),
                progress: &download::quiet,
            },
        )
        .await?;
    }
    Ok(path)
}

/// The arguments that run `installer` into `dir`.
pub fn installer_args(installer: &Path, dir: &Path) -> Vec<String> {
    vec![
        "-jar".into(),
        installer.to_string_lossy().into_owned(),
        "--installServer".into(),
        dir.to_string_lossy().into_owned(),
    ]
}

/// The arguments that start an installed Forge or NeoForge server in `dir`, after the memory
/// flags. Forge 1.17 and later and every NeoForge start from the args file their start script
/// names: `run.bat` on Windows, `run.sh` elsewhere. Older Forge has no script and starts from
/// the jar its installer wrote.
pub fn launch_args(
    dir: &Path,
    loader: ModLoader,
    game_version: &str,
    version: &str,
) -> Result<Vec<String>, InstallerError> {
    let script = if cfg!(windows) { "run.bat" } else { "run.sh" };
    if let Ok(text) = read_server_file(&dir.join(script))
        && let Some(args) = args_file(&text)
    {
        return Ok(vec![args.to_owned(), "--nogui".into()]);
    }
    let names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry.file_type().is_ok_and(|kind| kind.is_file())
                })
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    match old_forge_jar(&names, game_version, version) {
        Some(jar) if loader == ModLoader::Forge => {
            Ok(vec!["-jar".into(), jar, "--nogui".into()])
        }
        _ => Err(InstallerError::NoLaunch(loader)),
    }
}

/// The `@libraries/.../*_args.txt` a start script passes to Java.
fn args_file(script: &str) -> Option<&str> {
    script.split_whitespace().find(|token| {
        token.starts_with("@libraries/") && token.ends_with("_args.txt")
    })
}

/// The jar an installer before Forge 1.17 wrote, among the file names in the server folder. Its
/// name changed over the years; an exact name wins over a guess.
fn old_forge_jar(
    names: &[String],
    game_version: &str,
    version: &str,
) -> Option<String> {
    let exact = [
        // 1.12.2 to 1.16.5.
        format!("forge-{game_version}-{version}.jar"),
        // 1.8 to 1.11.2.
        format!("forge-{game_version}-{version}-universal.jar"),
        // 1.7.10.
        format!("forge-{game_version}-{version}-{game_version}-universal.jar"),
    ];
    if let Some(found) = exact.iter().find(|name| names.contains(name)) {
        return Some(found.clone());
    }
    names
        .iter()
        .filter(|name| {
            name.starts_with("forge-")
                && name.ends_with(".jar")
                && !name.ends_with("-installer.jar")
                && name.contains(game_version)
        })
        .min_by_key(|name| name.len())
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn old_forge_starts_from_the_jar_its_installer_wrote() {
        let installed = names(&[
            "forge-1.12.2-14.23.5.2860-installer.jar",
            "forge-1.12.2-14.23.5.2860.jar",
            "minecraft_server.1.12.2.jar",
        ]);
        assert_eq!(
            old_forge_jar(&installed, "1.12.2", "14.23.5.2860").as_deref(),
            Some("forge-1.12.2-14.23.5.2860.jar")
        );
        let universal = names(&[
            "forge-1.7.10-10.13.4.1614-1.7.10-universal.jar",
            "minecraft_server.1.7.10.jar",
        ]);
        assert_eq!(
            old_forge_jar(&universal, "1.7.10", "10.13.4.1614-1.7.10")
                .as_deref(),
            Some("forge-1.7.10-10.13.4.1614-1.7.10-universal.jar")
        );
        assert_eq!(
            old_forge_jar(&universal, "1.7.10", "10.13.4.1614").as_deref(),
            Some("forge-1.7.10-10.13.4.1614-1.7.10-universal.jar")
        );
        // The installer alone is not a server.
        let only_installer = names(&["forge-1.16.5-36.2.39-installer.jar"]);
        assert_eq!(old_forge_jar(&only_installer, "1.16.5", "36.2.39"), None);
    }

    #[test]
    fn the_start_script_of_this_platform_names_the_args_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("run.sh"),
            "#!/usr/bin/env sh\njava @user_jvm_args.txt @libraries/net/minecraftforge/forge/1.20.1-47.2.0/unix_args.txt \"$@\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("run.bat"),
            "java @user_jvm_args.txt @libraries/net/minecraftforge/forge/1.20.1-47.2.0/win_args.txt %*\r\npause\r\n",
        )
        .unwrap();
        let args =
            launch_args(dir.path(), ModLoader::Forge, "1.20.1", "47.2.0")
                .unwrap();
        let expected = if cfg!(windows) {
            "@libraries/net/minecraftforge/forge/1.20.1-47.2.0/win_args.txt"
        } else {
            "@libraries/net/minecraftforge/forge/1.20.1-47.2.0/unix_args.txt"
        };
        assert_eq!(args, [expected, "--nogui"]);
    }

    #[test]
    fn an_old_forge_folder_starts_from_its_jar() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "forge-1.12.2-14.23.5.2860.jar",
            "forge-1.12.2-14.23.5.2860-installer.jar",
        ] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        assert_eq!(
            launch_args(dir.path(), ModLoader::Forge, "1.12.2", "14.23.5.2860")
                .unwrap(),
            ["-jar", "forge-1.12.2-14.23.5.2860.jar", "--nogui"]
        );
        assert!(matches!(
            launch_args(dir.path(), ModLoader::NeoForge, "1.21.1", "21.1.77"),
            Err(InstallerError::NoLaunch(_))
        ));
    }
}
