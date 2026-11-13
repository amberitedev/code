use std::sync::Arc;

use engine::Engine;
use engine::data::ProcessMetadata;
use engine::instance::{
    LaunchCredentials, LaunchDefaults, LaunchRequest, QuickPlayType,
};

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
