use serde::Deserialize;

use crate::domain::instance::ModLoader;

#[derive(Debug, thiserror::Error)]
pub enum FlavourError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unsupported loader/version: {0}/{1}")]
    Unsupported(String, String),
    #[error("sha1 mismatch: expected {expected}, got {actual}")]
    Sha1Mismatch { expected: String, actual: String },
}

/// Resolved download info for a server JAR.
#[derive(Debug, Clone)]
pub struct JarInfo {
    pub url: String,
    pub filename: String,
    pub sha1: Option<String>,
}

/// Resolve the download URL for URL-based loaders (Vanilla, Paper, Fabric).
/// Quilt/Forge/NeoForge are installer-based — call `installer::install_with_installer` instead.
pub async fn resolve_jar(
    http: &reqwest::Client,
    loader: &ModLoader,
    game_version: &str,
    loader_version: Option<&str>,
) -> Result<JarInfo, FlavourError> {
    match loader {
        ModLoader::Vanilla => resolve_vanilla(http, game_version).await,
        ModLoader::Paper => resolve_paper(http, game_version).await,
        ModLoader::Fabric => resolve_fabric(http, game_version, loader_version).await,
        ModLoader::Quilt | ModLoader::Forge | ModLoader::NeoForge => {
            // These loaders use the installer path in server_jar.rs.
            Err(FlavourError::Unsupported(loader.to_string(), game_version.to_string()))
        }
    }
}

// ---- Vanilla ----

#[derive(Deserialize)]
struct VersionManifest {
    versions: Vec<VersionEntry>,
}

#[derive(Deserialize)]
struct VersionEntry {
    id: String,
    url: String,
}

#[derive(Deserialize)]
struct VersionMeta {
    downloads: Downloads,
}

#[derive(Deserialize)]
struct Downloads {
    server: DownloadInfo,
}

#[derive(Deserialize)]
struct DownloadInfo {
    url: String,
    sha1: String,
}

async fn resolve_vanilla(http: &reqwest::Client, game_version: &str) -> Result<JarInfo, FlavourError> {
    let manifest: VersionManifest = http
        .get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
        .send().await?.json().await?;
    let entry = manifest.versions.iter()
        .find(|v| v.id == game_version)
        .ok_or_else(|| FlavourError::Unsupported("vanilla".into(), game_version.into()))?;
    let meta: VersionMeta = http.get(&entry.url).send().await?.json().await?;
    Ok(JarInfo {
        url: meta.downloads.server.url,
        filename: "server.jar".into(),
        sha1: Some(meta.downloads.server.sha1),
    })
}

// ---- Paper ----

#[derive(Deserialize)]
struct PaperBuildsResponse {
    builds: Vec<PaperBuildEntry>,
}

#[derive(Deserialize)]
struct PaperBuildEntry {
    build: i64,
    channel: String,
    downloads: PaperDownloads,
}

#[derive(Deserialize)]
struct PaperDownloads {
    application: PaperDownload,
}

#[derive(Deserialize)]
struct PaperDownload {
    name: String,
}

async fn resolve_paper(http: &reqwest::Client, game_version: &str) -> Result<JarInfo, FlavourError> {
    let builds_url = format!(
        "https://api.papermc.io/v2/projects/paper/versions/{game_version}/builds"
    );
    let resp: PaperBuildsResponse = http.get(&builds_url).send().await?.error_for_status()?.json().await?;
    // Pick last build with channel = "default" (stable releases only)
    let entry = resp.builds.iter()
        .filter(|b| b.channel == "default")
        .last()
        .ok_or_else(|| FlavourError::Unsupported("paper".into(), game_version.into()))?;
    let build = entry.build;
    let filename = entry.downloads.application.name.clone();
    let url = format!(
        "https://api.papermc.io/v2/projects/paper/versions/{game_version}/builds/{build}/downloads/{filename}"
    );
    Ok(JarInfo { url, filename, sha1: None })
}

// ---- Fabric ----

#[derive(Deserialize)]
struct FabricLoaderVersion {
    version: String,
}

async fn resolve_fabric(
    http: &reqwest::Client,
    game_version: &str,
    loader_version: Option<&str>,
) -> Result<JarInfo, FlavourError> {
    let lv = if let Some(v) = loader_version {
        v.to_string()
    } else {
        let versions: Vec<FabricLoaderVersion> = http
            .get("https://meta.fabricmc.net/v2/versions/loader")
            .send().await?.json().await?;
        versions.first()
            .ok_or_else(|| FlavourError::Unsupported("fabric".into(), game_version.into()))?
            .version.clone()
    };
    // Fabric provides a pre-built server-launch JAR
    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{game_version}/{lv}/1.0.1/server/jar"
    );
    Ok(JarInfo {
        url,
        filename: "fabric-server-launch.jar".into(),
        sha1: None,
    })
}
