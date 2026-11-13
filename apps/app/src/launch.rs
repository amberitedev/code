use std::sync::Arc;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty,
    server::{Req, route},
};
use engine::Engine;
use engine::data::ProcessMetadata;
use engine::instance::{
    LaunchCredentials, LaunchDefaults, LaunchRequest, QuickPlayType,
};
use engine::server_address::ServerAddress;

use crate::server::{
    AppState,
    endpoints::launch::{JoinServer, JoinSingleplayerWorld},
};

pub fn routes() -> Router<AppState> {
    [
        route::<JoinServer, _, _, _>(join_server),
        route::<JoinSingleplayerWorld, _, _, _>(join_singleplayer_world),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
}

async fn join_server(
    State(engine): State<Arc<Engine>>,
    request: Req<JoinServer>,
) -> Result<Empty, ApiError> {
    let join = request.body;
    let address = ServerAddress::Unresolved(join.address);
    launch(&engine, &join.instance_id, QuickPlayType::Server(address))
        .await
        .map(|_| Empty)
        .map_err(engine::routes::api_error)
}

async fn join_singleplayer_world(
    State(engine): State<Arc<Engine>>,
    request: Req<JoinSingleplayerWorld>,
) -> Result<Empty, ApiError> {
    let join = request.body;
    let world = QuickPlayType::Singleplayer(join.world);
    launch(&engine, &join.instance_id, world)
        .await
        .map(|_| Empty)
        .map_err(engine::routes::api_error)
}

/// Launches an instance as the active Minecraft account, after applying this desktop's pending
/// skin, synced packs, game options and launch settings.
pub async fn launch(
    engine: &Arc<Engine>,
    instance_id: &str,
    quick_play: QuickPlayType,
) -> engine::Result<ProcessMetadata> {
    let account = engine::minecraft_auth::default_credential(engine)
        .await?
        .ok_or(engine::ErrorKind::NotLoggedIn)?;

    // A repair install reconciles synced options itself, so it runs before the pre-launch sync.
    engine::instance::ensure_installed(engine, instance_id).await?;
    engine::minecraft_skins::flush_pending_skin_change(engine).await?;

    if let Err(error) =
        engine::instance::reconcile_synced_packs(engine, instance_id).await
    {
        tracing::warn!(
            "Failed to reconcile synced packs before launching {instance_id}: {error}"
        );
    }
    if let Err(error) =
        engine::instance::sync_game_options_before_launch(engine, instance_id)
            .await
    {
        tracing::warn!(
            "Failed to reconcile game options before launching {instance_id}: {error}"
        );
    }

    let settings = engine::settings::get(engine).await?;
    engine::instance::apply_game_options_launcher_overrides(
        engine,
        instance_id,
        settings.force_fullscreen,
    )
    .await?;

    let profile = account.maybe_online_profile().await;
    let request = LaunchRequest {
        credentials: LaunchCredentials {
            username: profile.name.clone(),
            uuid: profile.id,
            access_token: account.access_token.clone(),
        },
        defaults: LaunchDefaults {
            extra_launch_args: settings.extra_launch_args,
            custom_env_vars: settings.custom_env_vars,
            memory: settings.memory,
            game_resolution: settings.game_resolution,
            hooks: settings.hooks,
        },
        quick_play,
    };
    engine::instance::launch(engine, instance_id, request).await
}
