//! Paper: one jar per build, from PaperMC's Fill API, kept in the shared store and run in place.
//! A server records its build as its loader version, so it stays on that build until it is
//! updated to a newer one.

use serde::Deserialize;

use super::{
    daedalus::SharedDirs,
    download::{self, FileDownload, Hash},
};

#[derive(Debug, thiserror::Error)]
pub enum PaperError {
    #[error(transparent)]
    Download(#[from] download::DownloadError),
    #[error("Paper has no stable build for Minecraft {0}")]
    NoBuild(String),
    #[error("Paper has no build {build} for Minecraft {game_version}")]
    UnknownBuild { game_version: String, build: String },
}

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

/// Download Paper `build` for `game_version` into the shared store, or the newest stable build
/// when `build` is `None` or `latest`. Returns the build installed. A build already there is
/// kept unless `verify` (repair) asks for it again.
pub async fn install(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    game_version: &str,
    build: Option<&str>,
    verify: bool,
    progress: &(dyn Fn(download::Progress) + Sync),
) -> Result<String, PaperError> {
    let url = format!(
        "https://fill.papermc.io/v3/projects/paper/versions/{game_version}/builds"
    );
    let builds: Vec<Build> = download::json(http, &url).await?;
    let chosen =
        match build.filter(|build| !matches!(*build, "latest" | "stable")) {
            Some(wanted) => builds
                .into_iter()
                .find(|candidate| candidate.id.to_string() == wanted)
                .ok_or_else(|| PaperError::UnknownBuild {
                    game_version: game_version.into(),
                    build: wanted.into(),
                })?,
            None => builds
                .into_iter()
                .filter(|candidate| candidate.channel == "STABLE")
                .max_by_key(|candidate| candidate.id)
                .ok_or_else(|| PaperError::NoBuild(game_version.into()))?,
        };
    let build = chosen.id.to_string();
    let path = dirs.paper_jar(game_version, &build);
    let hash = Hash::Sha256(&chosen.downloads.server.checksums.sha256);
    if !path.exists() || (verify && !download::has_hash(&path, hash).await) {
        download::file(
            http,
            &chosen.downloads.server.url,
            &path,
            &FileDownload {
                hash,
                limit: Some(512 * 1024 * 1024),
                progress,
            },
        )
        .await?;
    }
    Ok(build)
}
