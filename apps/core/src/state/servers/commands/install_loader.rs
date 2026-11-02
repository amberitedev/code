//! Installing a bare loader on a held server, or checking it again (repair).

use tracing::info;

use crate::{
    api::events::{ContentPlatform, InstallProgressKey},
    installs::{self, ModLoader},
    servers::Hold,
    state::servers::{Content, Error, Progress, adapters::sqlite::server_rows},
};

pub fn platform_name(loader: ModLoader) -> ContentPlatform {
    match loader {
        ModLoader::Vanilla => ContentPlatform::Vanilla,
        ModLoader::Paper => ContentPlatform::Paper,
        ModLoader::Fabric => ContentPlatform::Fabric,
        ModLoader::Forge => ContentPlatform::Forge,
        ModLoader::NeoForge => ContentPlatform::Neoforge,
        ModLoader::Quilt => ContentPlatform::Quilt,
    }
}

/// Install `loader` on the held server. A server with something installed is copied first,
/// unless this is a repair (`verify`), which changes no versions.
pub async fn install_loader(
    content: &Content,
    hold: &Hold,
    progress: &Progress,
    loader: ModLoader,
    game_version: &str,
    requested: Option<&str>,
    verify: bool,
) -> Result<(), Error> {
    let id = hold.id();
    let server = server_rows::get(&content.db, id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("There is no server {id}.")))?;
    if server.game_version.is_some() && !verify {
        content.backups.pre_update(id).await?;
    }
    let key = InstallProgressKey::Platform {
        platform: platform_name(loader),
        platform_version: requested.unwrap_or_default().to_owned(),
        game_version: game_version.to_owned(),
    };
    let installed = installs::install(
        &content.servers,
        hold,
        &content.http,
        &server,
        loader,
        game_version,
        requested,
        verify,
        &|share| progress.set("platform", &key, share),
    )
    .await?;
    server_rows::set_installed(
        &content.db,
        id,
        loader.as_str(),
        game_version,
        installed.as_deref(),
    )
    .await?;
    info!(server = %id, %loader, game_version, version = ?installed, "installed");
    Ok(())
}
