//! Builds the absolute, instance-independent launch token list for a build.
//!
//! These tokens are the portion of the JVM command between the memory flags and
//! the user's extra server args. Because every path is absolute (into the shared
//! store), the same tokens launch correctly from any instance's working
//! directory, so they are computed once at install time and persisted in the
//! installation's `launch.json`.
//!
//! Server main classes differ from the client ones launcher-meta records: Fabric and
//! Quilt launch their `KnotServer` entrypoint with the loader libraries and the
//! vanilla server jar on the classpath (exactly what their `*-server-launch`
//! jars do), while vanilla is launched with a plain `-jar`.

use std::path::Path;

use minecraft_meta_manifest::minecraft::VersionManifest;

use super::dirs::SharedDirs;
use super::libraries::{LibraryError, build_classpath};
use crate::installs::ModLoader;

/// Fabric's server entrypoint (the client uses `...knot.KnotClient`).
const FABRIC_SERVER_MAIN: &str =
    "net.fabricmc.loader.impl.launch.knot.KnotServer";
/// Quilt's server entrypoint.
const QUILT_SERVER_MAIN: &str =
    "org.quiltmc.loader.impl.launch.knot.KnotServer";

/// Build the launch tokens for a loader whose server launch launcher-meta models
/// directly (Vanilla, Fabric, Quilt). Returns `None` for the others, which start
/// from their own files (Forge/NeoForge/Paper).
pub fn build_launch_tokens(
    dirs: &SharedDirs,
    version_info: &VersionManifest,
    game_version: &str,
    loader: ModLoader,
    java_arch: &str,
) -> Result<Option<Vec<String>>, LibraryError> {
    let server_jar = dirs.server_jar_path(game_version);

    match loader {
        ModLoader::Vanilla => Ok(Some(vec![
            "-jar".to_string(),
            canonical(&server_jar)?,
            "--nogui".to_string(),
        ])),
        ModLoader::Fabric | ModLoader::Quilt => {
            let main = if loader == ModLoader::Fabric {
                FABRIC_SERVER_MAIN
            } else {
                QUILT_SERVER_MAIN
            };
            let classpath = build_classpath(
                &dirs.libraries_dir(),
                &version_info.libraries,
                &[&server_jar],
                java_arch,
            )?;
            Ok(Some(vec![
                "-cp".to_string(),
                classpath,
                main.to_string(),
                "--nogui".to_string(),
            ]))
        }
        ModLoader::Forge | ModLoader::NeoForge | ModLoader::Paper => Ok(None),
    }
}

fn canonical(path: &Path) -> Result<String, LibraryError> {
    Ok(dunce::canonicalize(path)
        .map_err(LibraryError::Io)?
        .to_string_lossy()
        .to_string())
}
