//! Paper: one jar per build, downloaded from PaperMC.

use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum PaperError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Paper has no stable build for Minecraft {0}")]
    NoBuild(String),
}

/// Where to download a Paper jar, and the file name PaperMC gives it.
#[derive(Debug, Clone)]
pub struct PaperJar {
    pub url: String,
    pub filename: String,
}

#[derive(Deserialize)]
struct BuildsResponse {
    builds: Vec<Build>,
}

#[derive(Deserialize)]
struct Build {
    build: i64,
    channel: String,
    downloads: Downloads,
}

#[derive(Deserialize)]
struct Downloads {
    application: Download,
}

#[derive(Deserialize)]
struct Download {
    name: String,
}

/// The latest stable Paper build for `game_version`.
pub async fn latest_build(
    http: &reqwest::Client,
    game_version: &str,
) -> Result<PaperJar, PaperError> {
    let builds_url = format!(
        "https://api.papermc.io/v2/projects/paper/versions/{game_version}/builds"
    );
    let response: BuildsResponse = http
        .get(&builds_url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    // "default" is PaperMC's stable channel.
    let entry = response
        .builds
        .iter()
        .rfind(|build| build.channel == "default")
        .ok_or_else(|| PaperError::NoBuild(game_version.into()))?;
    let build = entry.build;
    let filename = entry.downloads.application.name.clone();
    Ok(PaperJar {
        url: format!("{builds_url}/{build}/downloads/{filename}"),
        filename,
    })
}
