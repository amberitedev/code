//! Which options each instance shares.

use engine::Engine;
use engine::data::{
    InstanceMetadata, InstanceSyncedOption, InstanceSyncedOptions,
};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::ops::Deref;

/// An instance with the options it shares.
#[derive(Clone, Debug)]
pub(crate) struct SyncedInstance {
    pub metadata: InstanceMetadata,
    pub synced: InstanceSyncedOptions,
}

impl Deref for SyncedInstance {
    type Target = InstanceMetadata;

    fn deref(&self) -> &InstanceMetadata {
        &self.metadata
    }
}

/// `metadata` with the options it shares.
pub(crate) async fn with_synced_options(
    state: &Engine,
    metadata: &InstanceMetadata,
) -> engine::Result<SyncedInstance> {
    Ok(SyncedInstance {
        synced: instance_synced_options(
            &metadata.instance.id,
            crate::db::pool(state),
        )
        .await?,
        metadata: metadata.clone(),
    })
}

pub(crate) async fn get_instance(
    state: &Engine,
    instance_id: &str,
) -> engine::Result<Option<SyncedInstance>> {
    let Some(metadata) =
        engine::state::get_instance(instance_id, state.pool()).await?
    else {
        return Ok(None);
    };
    let synced =
        instance_synced_options(instance_id, crate::db::pool(state)).await?;
    Ok(Some(SyncedInstance { metadata, synced }))
}

pub(crate) async fn list_instances(
    state: &Engine,
) -> engine::Result<Vec<SyncedInstance>> {
    let mut synced =
        all_instance_synced_options(crate::db::pool(state)).await?;
    Ok(engine::state::list_instances(state.pool())
        .await?
        .into_iter()
        .map(|metadata| SyncedInstance {
            synced: synced.remove(&metadata.instance.id).unwrap_or_default(),
            metadata,
        })
        .collect())
}

async fn instance_synced_options(
    instance_id: &str,
    pool: &SqlitePool,
) -> engine::Result<InstanceSyncedOptions> {
    let mut synced = InstanceSyncedOptions::default();
    for feature in sqlx::query_scalar!(
        "
		SELECT feature
		FROM instance_sync_preferences
		WHERE instance_id = ? AND enabled = 1
		",
        instance_id,
    )
    .fetch_all(pool)
    .await?
    {
        enable(&mut synced, &feature);
    }
    Ok(synced)
}

/// Each instance's shared options, by instance id.
pub(crate) async fn all_instance_synced_options(
    pool: &SqlitePool,
) -> engine::Result<HashMap<String, InstanceSyncedOptions>> {
    let rows = sqlx::query!(
        "
		SELECT instance_id, feature
		FROM instance_sync_preferences
		WHERE enabled = 1
		",
    )
    .fetch_all(pool)
    .await?;

    let mut options = HashMap::<String, InstanceSyncedOptions>::new();
    for row in rows {
        enable(options.entry(row.instance_id).or_default(), &row.feature);
    }
    Ok(options)
}

fn enable(synced: &mut InstanceSyncedOptions, feature: &str) {
    match feature {
        "game_options" => synced.game_options = true,
        "command_history" => synced.command_history = true,
        "multiplayer_servers" => synced.multiplayer_servers = true,
        "creative_hotbars" => synced.creative_hotbars = true,
        "screenshots" => synced.screenshots = true,
        "resource_packs" => synced.resource_packs = true,
        "data_packs" => synced.data_packs = true,
        _ => {}
    }
}

pub(crate) async fn set_instance_sync_preference(
    instance_id: &str,
    option: InstanceSyncedOption,
    enabled: bool,
    pool: &SqlitePool,
) -> engine::Result<()> {
    let option_name = option.as_str();
    sqlx::query!(
        "
		INSERT INTO instance_sync_preferences (instance_id, feature, enabled)
		VALUES (?, ?, ?)
		ON CONFLICT (instance_id, feature) DO UPDATE SET
			enabled = excluded.enabled
		",
        instance_id,
        option_name,
        enabled,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Gives a new instance each option's default for new instances.
pub(crate) async fn insert_default_instance_sync_preferences(
    instance_id: &str,
    pool: &SqlitePool,
) -> engine::Result<()> {
    sqlx::query!(
        "
		INSERT INTO instance_sync_preferences (instance_id, feature, enabled)
		SELECT ?, feature, new_instance_default
		FROM sync_feature_settings
		WHERE true
		ON CONFLICT (instance_id, feature) DO NOTHING
		",
        instance_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}
