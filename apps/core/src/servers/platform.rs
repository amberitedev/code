//! What a server runs: its engine instance's loader and Minecraft version. A server is an engine
//! instance with the server's id, made and removed with it (`api::servers`).

use engine::{
    Engine,
    state::{
        AppliedContentSetPatch, EditInstance, InstanceInstallStage,
        InstanceLink,
    },
};

use crate::installs::ModLoader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub loader: ModLoader,
    pub game_version: String,
    pub loader_version: Option<String>,
}

/// `None` until something is installed: a new server's instance has no Minecraft version.
pub async fn platform(
    engine: &Engine,
    id: &str,
) -> engine::Result<Option<Platform>> {
    let Some(instance) = engine::instance::get(engine, id).await? else {
        return Ok(None);
    };
    let set = instance.applied_content_set;
    Ok((!set.game_version.is_empty()).then(|| Platform {
        loader: set.loader.into(),
        game_version: set.game_version,
        loader_version: set.loader_version,
    }))
}

/// The engine instance of a new server, with nothing installed.
pub async fn create(
    engine: &Engine,
    id: &str,
    name: &str,
) -> engine::Result<()> {
    engine::instance::create(
        engine,
        name.to_owned(),
        String::new(),
        engine::state::ModLoader::Vanilla,
        None,
        None,
        None,
        InstanceLink::Unmanaged,
        Some(id.to_owned()),
        Some(id.to_owned()),
    )
    .await
    .map(drop)
}

/// Record a loader Core installed itself: the server follows no pack from now on.
pub async fn set(
    engine: &Engine,
    id: &str,
    platform: &Platform,
) -> engine::Result<()> {
    engine::instance::edit(
        engine,
        id,
        EditInstance {
            install_stage: Some(InstanceInstallStage::Installed),
            link: Some(InstanceLink::Unmanaged),
            content_set_patch: Some(AppliedContentSetPatch {
                game_version: Some(platform.game_version.clone()),
                loader: Some(platform.loader.into()),
                loader_version: Some(platform.loader_version.clone()),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await
    .map(drop)
}
