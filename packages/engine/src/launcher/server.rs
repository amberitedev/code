//! A server-side engine's installs: the server jar, each loader's server install, and the Java
//! command a host runs. The engine only builds the command; the host starts the process.

pub(super) mod forge;
pub mod paper;

use std::path::{Path, PathBuf};

use anyhow::bail;
use contracts::Side;
use minecraft_meta_manifest::profile::Processor;

use super::args::get_class_paths;
use super::profile::ResolvedVersion;
use crate::data::ModLoader;
use crate::state::{DirectoryInfo, InstanceInstallStage};
use crate::{Engine, ErrorKind};

const FABRIC_SERVER_MAIN: &str =
    "net.fabricmc.loader.impl.launch.knot.KnotServer";
const QUILT_SERVER_MAIN: &str =
    "org.quiltmc.loader.impl.launch.knot.KnotServer";

/// How a host starts an installed server: `java`, the host's own JVM flags (memory, temporary
/// folder), then `args`, in `cwd`.
#[derive(Debug, Clone)]
pub struct ServerCommand {
    pub java: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

/// The command that starts instance `id`, an installed server. The Java is the instance's own,
/// or the managed one its Minecraft version needs, installed when missing.
pub async fn server_command(
    engine: &Engine,
    id: &str,
) -> crate::Result<ServerCommand> {
    let context =
        crate::state::instances::commands::get_instance_launch_context(
            id,
            &engine.pool,
        )
        .await?
        .ok_or_else(|| {
            ErrorKind::InputError(format!("unknown instance {id}"))
        })?;
    if context.instance.install_stage != InstanceInstallStage::Installed {
        bail!(ErrorKind::LauncherError(
            "instance is not installed; start an install job first".to_string()
        ));
    }
    let content_set = &context.applied_content_set;
    let java = super::resolve_java_for_launch(&context, engine).await?;
    let cwd = super::get_instance_full_path(
        &context.instance.path,
        &engine.directories,
    )?;
    let (minecraft, index) =
        super::resolve_minecraft_manifest(&content_set.game_version, engine)
            .await?;
    let version = &minecraft.versions[index];
    let loader_version = super::get_loader_version_from_profile(
        engine,
        &content_set.game_version,
        content_set.loader,
        content_set.loader_version.as_deref(),
        &engine.directories,
    )
    .await?;
    let args = match (content_set.loader, &loader_version) {
        (ModLoader::Vanilla, _)
        | (ModLoader::Fabric | ModLoader::Quilt, Some(_)) => {
            let info = super::download::download_version_info(
                engine,
                version,
                loader_version.as_ref(),
                None,
                None,
                None,
            )
            .await?;
            launch_args(
                &engine.directories.libraries_dir(),
                &server_jar_path(&engine.directories, &version.id),
                &info,
                content_set.loader,
                &java.architecture,
            )?
        }
        (ModLoader::Forge | ModLoader::NeoForge, Some(loader_version)) => {
            forge::launch_args(
                &engine.directories,
                &cwd,
                content_set.loader,
                &version.id,
                &loader_version.id,
            )?
        }
        (loader, None) => bail!(ErrorKind::NoLoaderVersion { loader }),
    };
    Ok(ServerCommand {
        java: PathBuf::from(java.path),
        args,
        cwd,
    })
}

/// The vanilla server jar of `game_version`, which every loader on that version shares.
pub(crate) fn server_jar_path(
    directories: &DirectoryInfo,
    game_version: &str,
) -> PathBuf {
    directories.version_dir(game_version).join("server.jar")
}

/// Whether a loader install step runs on `side`. A step that names no side runs on both.
pub(super) fn runs_on(processor: &Processor, side: Side) -> bool {
    let name = match side {
        Side::Client => "client",
        Side::Server => "server",
    };
    processor
        .sides
        .as_ref()
        .is_none_or(|sides| sides.iter().any(|side| side == name))
}

/// The arguments a Vanilla, Fabric or Quilt server starts with. Fabric and Quilt start their
/// `KnotServer` with the loader libraries and the vanilla server jar on the classpath.
fn launch_args(
    libraries_dir: &Path,
    server_jar: &Path,
    info: &ResolvedVersion,
    loader: ModLoader,
    java_arch: &str,
) -> crate::Result<Vec<String>> {
    let main = match loader {
        ModLoader::Vanilla => {
            return Ok(vec![
                "-jar".to_string(),
                canonical(server_jar)?,
                "--nogui".to_string(),
            ]);
        }
        ModLoader::Fabric => FABRIC_SERVER_MAIN,
        ModLoader::Quilt => QUILT_SERVER_MAIN,
        ModLoader::Forge | ModLoader::NeoForge => {
            bail!(ErrorKind::LauncherError(format!(
                "{loader:?} starts from its install's arguments"
            )))
        }
    };
    Ok(vec![
        "-cp".to_string(),
        get_class_paths(
            libraries_dir,
            &info.libraries,
            &[server_jar],
            java_arch,
            true,
        )?,
        main.to_string(),
        "--nogui".to_string(),
    ])
}

fn canonical(path: &Path) -> crate::Result<String> {
    Ok(crate::util::io::canonicalize(path)?
        .to_string_lossy()
        .into_owned())
}

/// Whether `version` is plainly a version: letters, digits, dots and dashes, starting with a
/// letter or digit, with no `..`. Versions go into vendor URLs and store paths.
fn plain_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 64
        && version.starts_with(|c: char| c.is_ascii_alphanumeric())
        && !version.contains("..")
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
}

fn checked(version: &str) -> crate::Result<&str> {
    if plain_version(version) {
        Ok(version)
    } else {
        bail!(ErrorKind::InputError(format!(
            "{version:?} is not a version"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minecraft_meta_manifest::minecraft::VersionManifest;

    fn processor(sides: Option<&[&str]>) -> Processor {
        Processor {
            jar: "a:b:1".to_string(),
            classpath: Vec::new(),
            args: Vec::new(),
            outputs: None,
            sides: sides.map(|sides| {
                sides.iter().map(|side| side.to_string()).collect()
            }),
        }
    }

    #[test]
    fn install_steps_run_on_the_side_they_name() {
        let client = processor(Some(&["client"]));
        let server = processor(Some(&["server"]));
        let both = processor(None);
        assert!(runs_on(&client, Side::Client));
        assert!(!runs_on(&client, Side::Server));
        assert!(runs_on(&server, Side::Server));
        assert!(!runs_on(&server, Side::Client));
        assert!(runs_on(&both, Side::Client));
        assert!(runs_on(&both, Side::Server));
    }

    fn library(name: &str, rules: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "downloads": {"artifact": {
                "sha1": "0000000000000000000000000000000000000000",
                "size": 1,
                "url": "https://maven.fabricmc.net/a.jar"
            }},
            "rules": rules
        })
    }

    /// A Fabric server's version info, and its server jar and libraries in a store, with one
    /// library for another OS and one listed twice.
    fn fabric_store(data: &Path) -> (ResolvedVersion, Vec<PathBuf>) {
        let other_os = if cfg!(windows) { "osx" } else { "windows" };
        let info: VersionManifest = serde_json::from_value(serde_json::json!({
            "assetIndex": {
                "id": "17",
                "sha1": "0000000000000000000000000000000000000000",
                "size": 1,
                "totalSize": 1,
                "url": "https://piston-meta.mojang.com/17.json"
            },
            "assets": "17",
            "downloads": {},
            "id": "1.21.1-0.16.14",
            "libraries": [
                library("org.ow2.asm:asm:9.8", serde_json::json!([])),
                library(
                    "net.fabricmc:intermediary:1.21.1",
                    serde_json::json!([{"action": "allow"}])
                ),
                library(
                    "org.lwjgl:lwjgl:3.3.3",
                    serde_json::json!([{"action": "allow", "os": {"name": other_os}}])
                ),
                library("org.ow2.asm:asm:9.8", serde_json::json!([])),
            ],
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "minimumLauncherVersion": 21,
            "releaseTime": "2024-08-08T12:24:45Z",
            "time": "2024-08-08T12:24:45Z",
            "type": "release"
        }))
        .unwrap();
        let files: Vec<PathBuf> = [
            "versions/1.21.1/server.jar",
            "libraries/org/ow2/asm/asm/9.8/asm-9.8.jar",
            "libraries/net/fabricmc/intermediary/1.21.1/intermediary-1.21.1.jar",
        ]
        .iter()
        .map(|path| data.join(path))
        .collect();
        for file in &files {
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, "").unwrap();
        }
        let files = files
            .iter()
            .map(|file| dunce::canonicalize(file).unwrap())
            .collect();
        (info.into(), files)
    }

    // The output of Core's `installs::shared::launch::build_launch_tokens` for the same store.
    #[test]
    fn a_vanilla_server_starts_from_its_jar() {
        let data = tempfile::tempdir().unwrap();
        let (info, files) = fabric_store(data.path());
        assert_eq!(
            launch_args(
                &data.path().join("libraries"),
                &data.path().join("versions/1.21.1/server.jar"),
                &info,
                ModLoader::Vanilla,
                std::env::consts::ARCH
            )
            .unwrap(),
            ["-jar", &*files[0].to_string_lossy(), "--nogui"]
        );
    }

    #[test]
    fn a_fabric_server_starts_knot_with_the_server_jar_first() {
        let data = tempfile::tempdir().unwrap();
        let (info, files) = fabric_store(data.path());
        let separator = if cfg!(windows) { ";" } else { ":" };
        let classpath = files
            .iter()
            .map(|file| file.to_string_lossy())
            .collect::<Vec<_>>()
            .join(separator);
        assert_eq!(
            launch_args(
                &data.path().join("libraries"),
                &data.path().join("versions/1.21.1/server.jar"),
                &info,
                ModLoader::Fabric,
                std::env::consts::ARCH
            )
            .unwrap(),
            [
                "-cp",
                &classpath,
                "net.fabricmc.loader.impl.launch.knot.KnotServer",
                "--nogui"
            ]
        );
    }

    #[test]
    fn a_version_that_is_not_plainly_one_never_reaches_a_url_or_path() {
        for good in [
            "47.2.0",
            "1.20.1-47.2.0",
            "21.1.77",
            "20.2.3-beta",
            "24w14a",
        ] {
            assert!(plain_version(good), "{good}");
        }
        for bad in [
            "47.2.0?x=/../../../../pwn",
            "..",
            "../47.2.0",
            "47.2.0/../x",
            r"47.2.0\x",
            ".hidden",
            "",
            "47 2",
        ] {
            assert!(!plain_version(bad), "{bad}");
        }
    }
}
