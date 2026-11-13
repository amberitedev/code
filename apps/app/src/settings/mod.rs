//! The desktop's settings. Reading or writing them keeps the fullscreen setting in step with
//! synced game options.

use engine::{Engine, data::Settings};

pub mod routes;

/// Gets entire settings
#[tracing::instrument(skip(state))]
pub async fn get(state: &Engine) -> engine::Result<Settings> {
    let _guard = crate::hooks::synced_options(state).lock().await;
    let mut settings = Settings::get(state.pool()).await?;
    if crate::synced_options::state::game_options_sync_is_enabled(state.pool())
        .await?
        && let Some(fullscreen) =
            crate::synced_options::shared_fullscreen_value(state).await?
        && settings.force_fullscreen != fullscreen
    {
        settings.force_fullscreen = fullscreen;
        settings.update(state.pool()).await?;
    }
    Ok(settings)
}

/// Sets entire settings
#[tracing::instrument(skip(state))]
pub async fn set(state: &Engine, settings: Settings) -> engine::Result<()> {
    let _guard = crate::hooks::synced_options(state).lock().await;
    let current_settings = Settings::get(state.pool()).await?;
    let game_options_sync_enabled =
        crate::synced_options::state::game_options_sync_is_enabled(
            state.pool(),
        )
        .await?;
    let current_fullscreen = if game_options_sync_enabled {
        crate::synced_options::shared_fullscreen_value(state)
            .await?
            .unwrap_or(current_settings.force_fullscreen)
    } else {
        current_settings.force_fullscreen
    };
    let fullscreen_changed = current_fullscreen != settings.force_fullscreen;
    let shared_fullscreen_changed =
        if game_options_sync_enabled && fullscreen_changed {
            crate::synced_options::update_shared_fullscreen_from_app(
                state,
                settings.force_fullscreen,
            )
            .await?
        } else {
            false
        };
    settings.update(state.pool()).await?;

    if shared_fullscreen_changed {
        let result =
            crate::synced_options::sync_all_participating_instances(state)
                .await;
        if result.failed > 0 {
            tracing::warn!(
                "Fullscreen was saved, but {} synced instance(s) could not be updated",
                result.failed
            );
        }
    }

    Ok(())
}
