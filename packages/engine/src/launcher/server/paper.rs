//! Paper: one jar per build, from PaperMC's Fill API, kept in the metadata folder and run in
//! place. A server records its build as its loader version, so it stays on that build until it
//! is updated to a newer one.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::bail;
use reqwest::Method;
use serde::Deserialize;
use sha2::Digest;

use super::checked;
use crate::state::DirectoryInfo;
use crate::util::fetch::{fetch_file, fetch_json};
use crate::{Engine, ErrorKind};

#[derive(Deserialize)]
struct Build {
    id: u64,
    channel: String,
    downloads: Downloads,
}

#[derive(Deserialize)]
struct Downloads {
    #[serde(rename = "server:default")]
    server: Download,
}

#[derive(Deserialize)]
struct Download {
    url: String,
    checksums: Checksums,
}

#[derive(Deserialize)]
struct Checksums {
    sha256: String,
}

/// Download Paper `build` for `game_version`, or the newest stable build when `build` is `None`,
/// `latest` or `stable`. Returns the build installed. A build already there is kept unless
/// `force` (repair) asks for it again.
pub async fn install(
    engine: &Engine,
    game_version: &str,
    build: Option<&str>,
    force: bool,
) -> crate::Result<String> {
    let game_version = checked(game_version)?;
    let builds: Vec<Build> = fetch_json(
        Method::GET,
        &format!(
            "https://fill.papermc.io/v3/projects/paper/versions/{game_version}/builds"
        ),
        None,
        None,
        None,
        &engine.api_semaphore,
        &engine.sessions,
    )
    .await?;
    let chosen = match build
        .filter(|build| !matches!(*build, "latest" | "stable"))
    {
        Some(wanted) => builds
            .into_iter()
            .find(|candidate| candidate.id.to_string() == wanted)
            .ok_or_else(|| {
                ErrorKind::InputError(format!(
                    "Paper has no build {wanted} for Minecraft {game_version}"
                ))
            })?,
        None => builds
            .into_iter()
            .filter(|candidate| candidate.channel == "STABLE")
            .max_by_key(|candidate| candidate.id)
            .ok_or_else(|| {
                ErrorKind::InputError(format!(
                    "Paper has no stable build for Minecraft {game_version}"
                ))
            })?,
    };
    let build = chosen.id.to_string();
    let path = jar_path(&engine.directories, game_version, &build);
    if path.exists() && !force {
        return Ok(build);
    }
    let file = fetch_file(
        &chosen.downloads.server.url,
        None,
        None,
        None,
        &engine.fetch_semaphore,
        &engine.sessions,
        None,
        &engine.directories.store_staging_dir(),
    )
    .await?;
    let sha256 = sha256(file.path()).await?;
    if !sha256.eq_ignore_ascii_case(&chosen.downloads.server.checksums.sha256) {
        bail!(ErrorKind::LauncherError(format!(
            "Paper {build} for Minecraft {game_version} downloaded damaged"
        )));
    }
    file.copy_to(&path, &engine.io_semaphore).await?;
    Ok(build)
}

/// The arguments a server on Paper `build` starts with.
pub fn launch_args(
    engine: &Engine,
    game_version: &str,
    build: &str,
) -> crate::Result<Vec<String>> {
    let jar =
        jar_path(&engine.directories, checked(game_version)?, checked(build)?);
    if !jar.is_file() {
        bail!(ErrorKind::LauncherError(format!(
            "Paper {build} for Minecraft {game_version} is not installed"
        )));
    }
    Ok(vec![
        "-jar".into(),
        jar.to_string_lossy().into_owned(),
        "--nogui".into(),
    ])
}

fn jar_path(
    directories: &DirectoryInfo,
    game_version: &str,
    build: &str,
) -> PathBuf {
    directories
        .metadata_dir()
        .join("paper")
        .join(format!("paper-{game_version}-{build}.jar"))
}

async fn sha256(path: &Path) -> crate::Result<String> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || -> crate::Result<String> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = sha2::Sha256::new();
        let mut buffer = vec![0; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(hex::encode(hasher.finalize()))
    })
    .await?
}
