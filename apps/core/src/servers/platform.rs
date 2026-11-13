//! What a server runs: its engine instance's loader and Minecraft version. A server is an engine
//! instance with the server's id, made and removed with it (`api::servers`).

use std::sync::Arc;

use engine::{
    Engine,
    state::{AppliedContentSetPatch, EditInstance, InstanceLink, ModLoader},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub loader: ModLoader,
    pub game_version: String,
    pub loader_version: Option<String>,
}

/// A loader name in any case.
pub fn parse_loader(name: &str) -> Option<ModLoader> {
    [
        ModLoader::Vanilla,
        ModLoader::Paper,
        ModLoader::Fabric,
        ModLoader::Forge,
        ModLoader::NeoForge,
        ModLoader::Quilt,
    ]
    .into_iter()
    .find(|loader| loader.as_str().eq_ignore_ascii_case(name))
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
        loader: set.loader,
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
        ModLoader::Vanilla,
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

/// Install `platform` on a held server with the engine's install job for its instance, and wait
/// for the job: the server follows no pack from now on. A failed install leaves the server on
/// what it ran before.
pub async fn install(
    engine: &Arc<Engine>,
    id: &str,
    platform: &Platform,
) -> engine::Result<()> {
    let before = engine::instance::get(engine, id).await?.ok_or_else(|| {
        engine::ErrorKind::InputError(format!("unknown instance {id}"))
    })?;
    point_at(engine, id, platform, InstanceLink::Unmanaged).await?;
    let installed = async {
        let job = engine::install::install_existing_instance(
            id.to_owned(),
            false,
            engine,
        )
        .await?;
        engine::install::runner::wait_for_job(job.job_id.parse()?, engine).await
    }
    .await;
    if installed.is_err() {
        let set = before.applied_content_set;
        let restored = Platform {
            loader: set.loader,
            game_version: set.game_version,
            loader_version: set.loader_version,
        };
        if let Err(error) = point_at(engine, id, &restored, before.link).await {
            tracing::warn!(server = %id, %error, "could not restore what the server ran");
        }
    }
    installed
}

async fn point_at(
    engine: &Engine,
    id: &str,
    platform: &Platform,
    link: InstanceLink,
) -> engine::Result<()> {
    engine::instance::edit(
        engine,
        id,
        EditInstance {
            link: Some(link),
            content_set_patch: Some(AppliedContentSetPatch {
                game_version: Some(platform.game_version.clone()),
                loader: Some(platform.loader),
                loader_version: Some(platform.loader_version.clone()),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await
    .map(drop)
}

/// Record `platform` as installed without installing it.
#[cfg(test)]
pub async fn set(
    engine: &Engine,
    id: &str,
    platform: &Platform,
) -> engine::Result<()> {
    point_at(engine, id, platform, InstanceLink::Unmanaged).await?;
    engine::instance::edit(
        engine,
        id,
        EditInstance {
            install_stage: Some(engine::state::InstanceInstallStage::Installed),
            ..Default::default()
        },
    )
    .await
    .map(drop)
}
