use std::sync::Arc;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, Json,
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
    endpoints::launch::{JoinServer, JoinSingleplayerWorld, PlayInstance},
};

pub fn routes() -> Router<AppState> {
    [
        route::<PlayInstance, _, _, _>(play_instance),
        route::<JoinServer, _, _, _>(join_server),
        route::<JoinSingleplayerWorld, _, _, _>(join_singleplayer_world),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
}

async fn play_instance(
    State(engine): State<Arc<Engine>>,
    request: Req<PlayInstance>,
) -> Result<Json<ProcessMetadata>, ApiError> {
    let play = request.body;
    let quick_play = match play.server_address {
        Some(address) => {
            QuickPlayType::Server(ServerAddress::Unresolved(address))
        }
        None => QuickPlayType::None,
    };
    launch(&engine, &play.instance_id, quick_play)
        .await
        .map(Json)
        .map_err(crate::accounts::api_error)
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
        .map_err(crate::accounts::api_error)
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
        .map_err(crate::accounts::api_error)
}

/// Launches an instance as the active Minecraft account, after applying this desktop's pending
/// skin, synced packs, game options and launch settings.
pub async fn launch(
    engine: &Arc<Engine>,
    instance_id: &str,
    quick_play: QuickPlayType,
) -> engine::Result<ProcessMetadata> {
    let account = crate::accounts::minecraft_auth::default_credential(engine)
        .await?
        .ok_or(engine::ErrorKind::NotLoggedIn)?;

    let settings = crate::settings::get(engine).await?;
    let force_fullscreen = settings.force_fullscreen;
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
    engine::instance::launch(engine, instance_id, request, async || {
        crate::skins::minecraft_skins::flush_pending_skin_change(engine).await?;
        if let Err(error) =
            crate::synced_options::reconcile_synced_packs(engine, instance_id).await
        {
            tracing::warn!(
                "Failed to reconcile synced packs before launching {instance_id}: {error}"
            );
        }
        if let Err(error) = crate::synced_options::sync_game_options_before_launch(
            engine,
            instance_id,
        )
        .await
        {
            tracing::warn!(
                "Failed to reconcile game options before launching {instance_id}: {error}"
            );
        }
        crate::synced_options::apply_game_options_launcher_overrides(
            engine,
            instance_id,
            force_fullscreen,
        )
        .await
    })
    .await
}
