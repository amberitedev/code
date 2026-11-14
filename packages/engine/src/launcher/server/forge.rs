//! Forge and NeoForge servers. Every NeoForge build, and Forge from 1.17, runs launcher-meta's
//! server-side install steps and starts from the arguments its installer ships. Older Forge has
//! no start launcher-meta can describe: its own installer runs in the server folder, and the
//! server starts from the jar it leaves there.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::bail;
use reqwest::Method;
use tokio::process::Command;

use super::{capped, checked};
use crate::data::ModLoader;
use crate::state::DirectoryInfo;
use crate::util::fetch::{fetch_advanced, fetch_file, write_atomic};
use crate::util::io::IOError;
use crate::util::platform::classpath_separator;
use crate::util::sandbox::Folder;
use crate::{Engine, ErrorKind};

pub(in crate::launcher) enum ForgeServer {
    /// The install steps run; the server starts from the installer's `args`.
    Steps { installer: PathBuf, args: String },
    /// The installer already ran in the server folder.
    Installed,
}

/// The first part of a Forge or NeoForge server install: the installer, and for older Forge the
/// whole install. `None` for other loaders.
pub(in crate::launcher) async fn prepare(
    state: &Engine,
    loader: ModLoader,
    game_version: &str,
    version: &str,
    java: &str,
    dir: &Path,
) -> crate::Result<Option<ForgeServer>> {
    if !matches!(loader, ModLoader::Forge | ModLoader::NeoForge) {
        return Ok(None);
    }
    let installer = installer(state, loader, game_version, version).await?;
    Ok(Some(match server_args(&installer).await? {
        Some(args) => ForgeServer::Steps { installer, args },
        None => {
            run_vendor_installer(java, &installer, dir).await?;
            ForgeServer::Installed
        }
    }))
}

/// Fetch the installer for `version` into the metadata folder, unless it is there. Checked
/// against the SHA-1 the maven publishes beside it, when it does.
async fn installer(
    state: &Engine,
    loader: ModLoader,
    game_version: &str,
    version: &str,
) -> crate::Result<PathBuf> {
    let (game_version, version) = (checked(game_version)?, checked(version)?);
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
    let path = state
        .directories
        .metadata_dir()
        .join("installers")
        .join(name);
    if path.exists() {
        return Ok(path);
    }
    let sha1 = fetch_advanced(
        Method::GET,
        &format!("{url}.sha1"),
        None,
        None,
        None,
        None,
        None,
        None,
        &state.fetch_semaphore,
        &state.sessions,
    )
    .await
    .ok()
    .and_then(|text| {
        let text = String::from_utf8_lossy(&text);
        let hash = text.split_whitespace().next()?.to_owned();
        (hash.len() == 40).then_some(hash)
    });
    fetch_file(
        &url,
        sha1.as_deref(),
        None,
        None,
        &state.fetch_semaphore,
        &state.sessions,
        Some(&mut capped(256 * 1024 * 1024)),
        &state.directories.store_staging_dir(),
    )
    .await?
    .copy_to(&path, &state.io_semaphore)
    .await?;
    Ok(path)
}

/// The Java arguments this platform's server starts with, as the installer ships them, or
/// `None` for Forge before 1.17, which has none. Read from the jar; nothing in it runs.
async fn server_args(installer: &Path) -> crate::Result<Option<String>> {
    let installer = installer.to_owned();
    let name = if cfg!(windows) {
        "data/win_args.txt"
    } else {
        "data/unix_args.txt"
    };
    tokio::task::spawn_blocking(move || -> crate::Result<Option<String>> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&installer)?)?;
        let Ok(entry) = zip.by_name(name) else {
            return Ok(None);
        };
        let mut text = String::new();
        entry.take(1024 * 1024).read_to_string(&mut text)?;
        Ok(Some(text))
    })
    .await?
}

/// Record how a build installed from launcher-meta starts: the installer's `args` with every
/// `libraries/` path made absolute. Written last, so it is there only once the build is
/// complete.
pub(in crate::launcher) async fn record_launch_args(
    state: &Engine,
    version_jar: &str,
    args: &str,
    java_arch: &str,
) -> crate::Result<()> {
    let libraries =
        crate::util::io::canonicalize(state.directories.libraries_dir())?;
    let (tokens, referenced) = absolute_args(
        args,
        &libraries.to_string_lossy(),
        classpath_separator(java_arch),
    );
    if let Some(missing) = referenced
        .iter()
        .find(|path| !libraries.join(path).is_file())
    {
        bail!(ErrorKind::LauncherError(format!(
            "the install did not make {missing}, which the server starts with"
        )));
    }
    write_atomic(
        &server_args_path(&state.directories, version_jar),
        tokens.join("\n").as_bytes(),
        &state.io_semaphore,
    )
    .await
}

fn server_args_path(directories: &DirectoryInfo, version_jar: &str) -> PathBuf {
    directories.version_dir(version_jar).join("server-args.txt")
}

/// The installer's server arguments, one token each, with every `libraries/` path made absolute
/// into `libraries`; and those paths, relative to it. `sep` separates the entries of a module
/// path or classpath.
fn absolute_args(
    args: &str,
    libraries: &str,
    sep: &str,
) -> (Vec<String>, Vec<String>) {
    let mut referenced = Vec::new();
    let tokens = args
        .split_whitespace()
        .map(|token| {
            let (prefix, value) = match token.split_once('=') {
                Some((key, value)) if key.starts_with("-D") => {
                    (&token[..=key.len()], value)
                }
                _ => ("", token),
            };
            let value = value
                .split(sep)
                .map(|part| {
                    if part == "libraries" {
                        libraries.to_owned()
                    } else if let Some(path) = part.strip_prefix("libraries/") {
                        referenced.push(path.to_owned());
                        format!("{libraries}/{path}")
                    } else {
                        part.to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join(sep);
            format!("{prefix}{value}")
        })
        .collect();
    (tokens, referenced)
}

/// Run an older Forge installer in the server folder `dir`. It writes its log beside itself, so
/// it runs from a copy there, removed after; the log stays when it fails, since it says why.
async fn run_vendor_installer(
    java: &str,
    installer: &Path,
    dir: &Path,
) -> crate::Result<()> {
    let name = installer
        .file_name()
        .ok_or_else(|| {
            ErrorKind::LauncherError("installer has no name".into())
        })?
        .to_string_lossy()
        .into_owned();
    {
        let (installer, dir, name) =
            (installer.to_owned(), dir.to_owned(), name.clone());
        tokio::task::spawn_blocking(move || {
            Folder::open(&dir)?
                .write(&name, &mut std::fs::File::open(&installer)?)
        })
        .await??;
    }
    let output = Command::new(java)
        .args(["-jar", &name, "--installServer"])
        .arg(dir)
        .current_dir(dir)
        .output()
        .await;
    let _ = std::fs::remove_file(dir.join(&name));
    let output = output.map_err(|error| IOError::with_path(error, java))?;
    if !output.status.success() {
        bail!(ErrorKind::LauncherError(format!(
            "the Forge installer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    remove_installer_leftovers(dir, &name).await;
    Ok(())
}

/// What the installer leaves in the server folder: its log, and its temporary folders in
/// `tmp/`. Removed through the folder, so a link a mod planted is never followed. Best effort.
async fn remove_installer_leftovers(dir: &Path, installer: &str) {
    let dir = dir.to_owned();
    let log = format!("{installer}.log");
    let removed =
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            let folder = Folder::open(&dir)?;
            let mut leftovers = vec![log];
            if let Ok(tmp) = folder.open_dir(Some("tmp")) {
                for entry in tmp.entries()?.filter_map(Result::ok) {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.starts_with("neoforge_installer")
                        || name.starts_with("forge_installer")
                    {
                        leftovers.push(format!("tmp/{name}"));
                    }
                }
            }
            for path in leftovers {
                if folder.exists(&path) {
                    folder.remove(&path)?;
                }
            }
            Ok(())
        })
        .await;
    if let Err(error) = removed
        .map_err(std::io::Error::other)
        .and_then(|result| result)
    {
        tracing::warn!(%error, "could not remove what the Forge installer left behind");
    }
}

/// The arguments that start an installed Forge or NeoForge server in `dir`. A build installed
/// from launcher-meta starts from the arguments it recorded; older Forge from the jar its
/// installer wrote in the server folder.
pub(super) fn launch_args(
    directories: &DirectoryInfo,
    dir: &Path,
    loader: ModLoader,
    game_version: &str,
    version: &str,
) -> crate::Result<Vec<String>> {
    let version_jar = format!("{game_version}-{version}");
    if let Ok(args) =
        std::fs::read_to_string(server_args_path(directories, &version_jar))
    {
        return Ok(args
            .lines()
            .map(str::to_owned)
            .chain(["--nogui".to_owned()])
            .collect());
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
        _ => bail!(ErrorKind::LauncherError(format!(
            "the {} installer left nothing to start the server with",
            loader.as_str()
        ))),
    }
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
    fn the_installer_arguments_point_into_the_shared_store() {
        let args = "-p libraries/cpw/a.jar:libraries/org/b.jar\n--add-modules ALL-MODULE-PATH\n-DignoreList=a.jar,b.jar\n-Dfml.gameLayerLibraries=\n-DlibraryDirectory=libraries\n-DlegacyClassPath=libraries/net/c.jar:libraries/net/d.jar\ncpw.mods.bootstraplauncher.BootstrapLauncher\n--launchTarget forgeserver\n";
        let (tokens, referenced) =
            absolute_args(args, "/data/meta/libraries", ":");
        assert_eq!(
            tokens,
            [
                "-p",
                "/data/meta/libraries/cpw/a.jar:/data/meta/libraries/org/b.jar",
                "--add-modules",
                "ALL-MODULE-PATH",
                "-DignoreList=a.jar,b.jar",
                "-Dfml.gameLayerLibraries=",
                "-DlibraryDirectory=/data/meta/libraries",
                "-DlegacyClassPath=/data/meta/libraries/net/c.jar:/data/meta/libraries/net/d.jar",
                "cpw.mods.bootstraplauncher.BootstrapLauncher",
                "--launchTarget",
                "forgeserver",
            ]
        );
        assert_eq!(
            referenced,
            ["cpw/a.jar", "org/b.jar", "net/c.jar", "net/d.jar"]
        );
    }

    async fn directories(data: &Path) -> DirectoryInfo {
        DirectoryInfo::init(data.to_owned(), data.join("servers"), None, None)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_build_installed_from_launcher_meta_starts_from_the_store() {
        let data = tempfile::tempdir().unwrap();
        let directories = directories(data.path()).await;
        let path = server_args_path(&directories, "1.21.1-21.1.265");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "-p\n/m/libraries/a.jar\nMain").unwrap();
        let server = tempfile::tempdir().unwrap();
        assert_eq!(
            launch_args(
                &directories,
                server.path(),
                ModLoader::NeoForge,
                "1.21.1",
                "21.1.265"
            )
            .unwrap(),
            ["-p", "/m/libraries/a.jar", "Main", "--nogui"]
        );
    }

    #[tokio::test]
    async fn an_old_forge_folder_starts_from_its_jar() {
        let data = tempfile::tempdir().unwrap();
        let directories = directories(data.path()).await;
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "forge-1.12.2-14.23.5.2860.jar",
            "forge-1.12.2-14.23.5.2860-installer.jar",
        ] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        assert_eq!(
            launch_args(
                &directories,
                dir.path(),
                ModLoader::Forge,
                "1.12.2",
                "14.23.5.2860"
            )
            .unwrap(),
            ["-jar", "forge-1.12.2-14.23.5.2860.jar", "--nogui"]
        );
        assert!(
            launch_args(
                &directories,
                dir.path(),
                ModLoader::NeoForge,
                "1.21.1",
                "21.1.77"
            )
            .is_err()
        );
    }
}
