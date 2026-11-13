//! The App's side of the engine's instance changes. Synced options, packs and servers keep up
//! through modpack updates, removals, content changes and server-list edits, and the state they
//! share for one engine lives here.

use std::any::Any;
use std::sync::Arc;

use dashmap::DashMap;
use engine::{
    Engine, InstanceHooks, PackOverrides, PackServers,
    access::BoxFuture,
    data::{ContentItem, InstanceMetadata},
    server_list::{DesyncServerMode, ServerRecord},
};
use quartz_nbt::NbtCompound;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::synced_options::{self as synced, state::SyncedOptions};

#[derive(Default)]
pub struct DesktopHooks {
    synced_options: SyncedOptions,
    screenshots: DashMap<String, Arc<Mutex<()>>>,
}

fn desktop(engine: &Engine) -> &DesktopHooks {
    (engine.hooks() as &dyn Any)
        .downcast_ref()
        .expect("the App's engine carries DesktopHooks")
}

pub fn synced_options(engine: &Engine) -> &SyncedOptions {
    &desktop(engine).synced_options
}

/// Serializes screenshot changes in one instance.
pub async fn lock_screenshots(
    engine: &Engine,
    instance_id: &str,
) -> OwnedMutexGuard<()> {
    let lock = desktop(engine)
        .screenshots
        .entry(instance_id.to_string())
        .or_default()
        .clone();
    lock.lock_owned().await
}

pub fn forget_instance(engine: &Engine, instance_id: &str) {
    desktop(engine).screenshots.remove(instance_id);
}

impl InstanceHooks for DesktopHooks {
    fn before_pack_update<'a>(
        &'a self,
        engine: &'a Engine,
        instance_id: &'a str,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(synced::prepare_instance_update(engine, instance_id))
    }

    fn after_pack_update<'a>(
        &'a self,
        engine: &'a Engine,
        instance_id: &'a str,
        overrides: PackOverrides<'a>,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(async move {
            match overrides.servers {
                PackServers::Untouched => {}
                PackServers::Ignored => {
                    synced::discard_modpack_servers(instance_id, engine)
                        .await?;
                }
                PackServers::Pack(path) => {
                    synced::capture_modpack_server_override(
                        engine,
                        instance_id,
                        path,
                    )
                    .await?;
                }
            }
            if let Err(error) = synced::capture_game_options_pack_base(
                engine,
                instance_id,
                overrides.game_options,
            )
            .await
            {
                tracing::warn!(
                    "The pack was applied, but its options.txt could not be captured for game-settings sync: {error}"
                );
            }
            synced::reconcile_instance_after_pack_update(engine, instance_id)
                .await
        })
    }

    fn removing<'a>(
        &'a self,
        _engine: &'a Engine,
    ) -> BoxFuture<'a, Box<dyn Send + 'a>> {
        Box::pin(async {
            Box::new(self.synced_options.lock().await) as Box<dyn Send>
        })
    }

    fn decorate_content<'a>(
        &'a self,
        engine: &'a Engine,
        instance_id: &'a str,
        items: &'a mut [ContentItem],
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(synced::decorate_content(instance_id, items, engine))
    }

    fn content_installed<'a>(
        &'a self,
        engine: &'a Engine,
        instance_id: &'a str,
        path: &'a str,
    ) -> BoxFuture<'a, ()> {
        Box::pin(synced::sync_new_pack(engine, instance_id, path))
    }

    fn content_changed<'a>(
        &'a self,
        engine: &'a Engine,
        instance_id: &'a str,
        paths: &'a [&'a str],
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if paths.iter().any(|path| synced::is_pack_path(path)) {
                synced::reconcile_synced_packs_after_content_change(
                    engine,
                    instance_id,
                )
                .await;
            }
        })
    }

    fn list_servers<'a>(
        &'a self,
        engine: &'a Engine,
        metadata: &'a InstanceMetadata,
    ) -> BoxFuture<'a, engine::Result<Vec<ServerRecord>>> {
        Box::pin(synced::list_server_records(metadata, engine))
    }

    fn add_server<'a>(
        &'a self,
        engine: &'a Engine,
        metadata: &'a InstanceMetadata,
        data: NbtCompound,
    ) -> BoxFuture<'a, engine::Result<String>> {
        Box::pin(synced::add_user_server(metadata, data, engine))
    }

    fn ensure_managed_server<'a>(
        &'a self,
        engine: &'a Engine,
        metadata: &'a InstanceMetadata,
        data: NbtCompound,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(synced::ensure_managed_server(metadata, data, engine))
    }

    fn update_server<'a>(
        &'a self,
        engine: &'a Engine,
        metadata: &'a InstanceMetadata,
        index: usize,
        name: String,
        address: String,
        accept_textures: Option<bool>,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(synced::update_server_by_index(
            metadata,
            index,
            name,
            address,
            accept_textures,
            engine,
        ))
    }

    fn remove_server<'a>(
        &'a self,
        engine: &'a Engine,
        metadata: &'a InstanceMetadata,
        index: usize,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(synced::remove_server_by_index(metadata, index, engine))
    }

    fn remove_managed_servers<'a>(
        &'a self,
        engine: &'a Engine,
        instance_id: &'a str,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(synced::remove_managed_servers(instance_id, engine))
    }

    fn discard_modpack_servers<'a>(
        &'a self,
        engine: &'a Engine,
        instance_id: &'a str,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(synced::discard_modpack_servers(instance_id, engine))
    }

    fn desync_server<'a>(
        &'a self,
        engine: &'a Engine,
        instance_id: &'a str,
        server_id: &'a str,
        mode: DesyncServerMode,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(synced::desync_server(engine, instance_id, server_id, mode))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use engine::data::{InstanceLink, InstanceSyncedOption, ModLoader};
    use engine::instance;
    use std::{path::Path, sync::Arc};

    /// The App's engine, with the synced options that need Mojang's version manifest turned off
    /// so nothing reaches the network.
    pub(crate) async fn offline_engine(dir: &Path) -> Arc<Engine> {
        engine::install_crypto_provider();
        let config = crate::data_dir::engine_config(
            dir.to_path_buf(),
            engine::EventSink::new(),
            None,
        )
        .await
        .unwrap();
        let engine = Engine::new(config).await.unwrap();
        for option in [
            InstanceSyncedOption::GameOptions,
            InstanceSyncedOption::CommandHistory,
            InstanceSyncedOption::MultiplayerServers,
            InstanceSyncedOption::CreativeHotbars,
        ] {
            synced::set_global_synced_option(&engine, option, false, None)
                .await
                .unwrap();
        }
        engine
    }

    pub(crate) async fn create(engine: &Engine) -> InstanceMetadata {
        instance::create(
            engine,
            "Followed".to_string(),
            "1.21.1".to_string(),
            ModLoader::Vanilla,
            None,
            None,
            None,
            InstanceLink::Unmanaged,
            None,
            None,
        )
        .await
        .unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_pack_update_keeps_a_changed_options_value() {
        let dir = tempfile::tempdir().unwrap();
        let engine = offline_engine(dir.path()).await;
        let id = create(&engine).await.instance.id;
        let options = instance::get_full_path(&engine, &id)
            .await
            .unwrap()
            .join("options.txt");
        std::fs::write(&options, "fov:0.75\n").unwrap();

        engine
            .hooks()
            .before_pack_update(&engine, &id)
            .await
            .unwrap();
        std::fs::write(&options, "fov:0.0\n").unwrap();
        let overrides = PackOverrides {
            servers: PackServers::Untouched,
            game_options: None,
        };
        engine
            .hooks()
            .after_pack_update(&engine, &id, overrides)
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(&options).unwrap(), "fov:0.75\n");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn removal_waits_for_a_synced_options_pass_in_flight() {
        let dir = tempfile::tempdir().unwrap();
        let engine = offline_engine(dir.path()).await;
        let id = create(&engine).await.instance.id;
        let folder = instance::get_full_path(&engine, &id).await.unwrap();
        std::fs::create_dir_all(&folder).unwrap();

        let pass = synced_options(&engine).lock().await;
        let removal = tokio::spawn({
            let engine = Arc::clone(&engine);
            async move { instance::remove(&engine, &id).await }
        });
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(folder.exists() && !removal.is_finished());
        drop(pass);
        removal.await.unwrap().unwrap();
        assert!(!folder.exists());
    }
}
