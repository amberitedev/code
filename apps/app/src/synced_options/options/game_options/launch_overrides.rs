//! Writes synced settings and launcher overrides before Minecraft starts.

use super::options_file::{
    GameOptionsDocument, options_path, read_document, sha1_bytes,
};
use super::write_shared_settings::{
    apply_shared_settings_to_instance, remember_processed_sha,
    sync_is_active_for_instance,
};
use crate::synced_options::state::SyncedInstance;
use crate::synced_options::state::{
    game_options_sync_is_enabled, load_game_option_preferences,
};
use engine::ErrorKind;
use engine::state::{Engine, InstanceMetadata};
use engine::util::io;
use std::collections::HashSet;

pub(super) fn currently_launcher_owned_keys(
    metadata: &InstanceMetadata,
) -> HashSet<String> {
    let mut keys = HashSet::new();
    if metadata.launch_overrides.force_fullscreen.is_some() {
        keys.insert("fullscreen".to_string());
    }
    keys
}

/// Writes canonical settings to `options.txt` before launch.
pub async fn sync_before_launch(
    state: &Engine,
    instance_id: &str,
) -> engine::Result<()> {
    let _guard = crate::hooks::synced_options(state).lock().await;
    let metadata =
        crate::synced_options::state::get_instance(state, instance_id)
            .await?
            .ok_or(ErrorKind::InputError("unknown instance".to_string()))?;
    if sync_is_active_for_instance(&metadata, state).await? {
        let _ =
            apply_shared_settings_to_instance(&metadata, state, false).await?;
    }
    Ok(())
}

/// Applies the instance's fullscreen override after shared settings, or the App's
/// `force_fullscreen` when fullscreen is not synced for the instance.
///
/// Stores the written file's hash so the watcher ignores the app's write.
pub async fn apply_launcher_overrides(
    state: &Engine,
    instance_id: &str,
    force_fullscreen: bool,
) -> engine::Result<()> {
    let _guard = crate::hooks::synced_options(state).lock().await;
    let metadata =
        crate::synced_options::state::get_instance(state, instance_id)
            .await?
            .ok_or(ErrorKind::InputError("unknown instance".to_string()))?;
    let fullscreen = match metadata.launch_overrides.force_fullscreen {
        Some(fullscreen) => fullscreen,
        None if force_fullscreen
            && !fullscreen_is_synced(&metadata, state).await? =>
        {
            true
        }
        None => return Ok(()),
    };
    let path = options_path(&metadata, state);
    let (mut document, input_bytes) = if path.exists() {
        read_document(&path).await?
    } else {
        (
            GameOptionsDocument::for_instance(&metadata, state)
                .await?
                .unwrap_or_else(GameOptionsDocument::empty),
            Vec::new(),
        )
    };
    document.set("fullscreen", &fullscreen.to_string(), true)?;
    let output_bytes = document.serialize()?;
    let output_sha1 = sha1_bytes(&output_bytes);
    if output_bytes != input_bytes {
        io::write(&path, output_bytes).await?;
    }
    remember_processed_sha(instance_id, &output_sha1, state).await?;
    Ok(())
}

async fn fullscreen_is_synced(
    metadata: &SyncedInstance,
    state: &Engine,
) -> engine::Result<bool> {
    Ok(metadata.synced.game_options
        && game_options_sync_is_enabled(crate::db::pool(state)).await?
        && load_game_option_preferences(crate::db::pool(state))
            .await?
            .get("fullscreen")
            .is_some_and(|preference| preference.enabled))
}
