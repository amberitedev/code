//! The App's side of the engine's instance changes. Synced options, packs and servers keep up
//! through modpack updates, removals, content changes and server-list edits.

use std::sync::{Arc, OnceLock, Weak};

use engine::{
    Engine, InstanceHooks, PackOverrides, PackServers, SavedSecrets,
    access::BoxFuture,
    data::{ContentItem, InstanceMetadata},
    server_list::{DesyncServerMode, ServerRecord},
};
use quartz_nbt::NbtCompound;

use crate::state::{Desktop, DesktopState};
use crate::synced_options as synced;

pub struct DesktopHooks {
    engine: OnceLock<Weak<Engine>>,
    state: Arc<DesktopState>,
}

impl DesktopHooks {
    pub fn new(state: Arc<DesktopState>) -> Self {
        Self {
            engine: OnceLock::new(),
            state,
        }
    }

    /// The engine these hooks serve, which is alive while it calls them.
    fn desktop(&self) -> Desktop {
        let engine = self
            .engine
            .get()
            .and_then(Weak::upgrade)
            .expect("the engine binds its hooks and outlives their calls");
        Desktop::new(engine, Arc::clone(&self.state))
    }
}

impl InstanceHooks for DesktopHooks {
    fn bind(&self, engine: Weak<Engine>) {
        let _ = self.engine.set(engine);
    }

    fn secrets<'a>(
        &'a self,
        engine: &'a Engine,
    ) -> BoxFuture<'a, engine::Result<SavedSecrets>> {
        Box::pin(crate::accounts::saved_secrets(
            &self.state.db,
            &engine.sessions,
        ))
    }

    fn before_pack_update<'a>(
        &'a self,
        _: &'a Engine,
        instance_id: &'a str,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(async move {
            synced::prepare_instance_update(&self.desktop(), instance_id).await
        })
    }

    fn after_pack_update<'a>(
        &'a self,
        _: &'a Engine,
        instance_id: &'a str,
        overrides: PackOverrides<'a>,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(async move {
            let desktop = self.desktop();
            match overrides.servers {
                PackServers::Untouched => {}
                PackServers::Ignored => {
                    synced::discard_modpack_servers(instance_id, &desktop)
                        .await?;
                }
                PackServers::Pack(path) => {
                    synced::capture_modpack_server_override(
                        &desktop,
                        instance_id,
                        path,
                    )
                    .await?;
                }
            }
            if let Err(error) = synced::capture_game_options_pack_base(
                &desktop,
                instance_id,
                overrides.game_options,
            )
            .await
            {
                tracing::warn!(
                    "The pack was applied, but its options.txt could not be captured for game-settings sync: {error}"
                );
            }
            synced::reconcile_instance_after_pack_update(&desktop, instance_id)
                .await
        })
    }

    fn removing<'a>(
        &'a self,
        _: &'a Engine,
    ) -> BoxFuture<'a, Box<dyn Send + 'a>> {
        Box::pin(async {
            Box::new(self.state.synced_options.lock().await) as Box<dyn Send>
        })
    }

    fn decorate_content<'a>(
        &'a self,
        _: &'a Engine,
        instance_id: &'a str,
        items: &'a mut [ContentItem],
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(async move {
            synced::decorate_content(instance_id, items, &self.desktop()).await
        })
    }

    fn content_installed<'a>(
        &'a self,
        _: &'a Engine,
        instance_id: &'a str,
        path: &'a str,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            synced::sync_new_pack(&self.desktop(), instance_id, path).await
        })
    }

    fn content_changed<'a>(
        &'a self,
        _: &'a Engine,
        instance_id: &'a str,
        paths: &'a [&'a str],
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if paths.iter().any(|path| synced::is_pack_path(path)) {
                synced::reconcile_synced_packs_after_content_change(
                    &self.desktop(),
                    instance_id,
                )
                .await;
            }
        })
    }

    fn list_servers<'a>(
        &'a self,
        _: &'a Engine,
        metadata: &'a InstanceMetadata,
    ) -> BoxFuture<'a, engine::Result<Vec<ServerRecord>>> {
        Box::pin(async move {
            let desktop = self.desktop();
            let metadata =
                synced::state::with_synced_options(&desktop, metadata).await?;
            synced::list_server_records(&metadata, &desktop).await
        })
    }

    fn add_server<'a>(
        &'a self,
        _: &'a Engine,
        metadata: &'a InstanceMetadata,
        data: NbtCompound,
    ) -> BoxFuture<'a, engine::Result<String>> {
        Box::pin(async move {
            let desktop = self.desktop();
            let metadata =
                synced::state::with_synced_options(&desktop, metadata).await?;
            synced::add_user_server(&metadata, data, &desktop).await
        })
    }

    fn ensure_managed_server<'a>(
        &'a self,
        _: &'a Engine,
        metadata: &'a InstanceMetadata,
        data: NbtCompound,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(async move {
            let desktop = self.desktop();
            let metadata =
                synced::state::with_synced_options(&desktop, metadata).await?;
            synced::ensure_managed_server(&metadata, data, &desktop).await
        })
    }

    fn update_server<'a>(
        &'a self,
        _: &'a Engine,
        metadata: &'a InstanceMetadata,
        index: usize,
        name: String,
        address: String,
        accept_textures: Option<bool>,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(async move {
            let desktop = self.desktop();
            let metadata =
                synced::state::with_synced_options(&desktop, metadata).await?;
            synced::update_server_by_index(
                &metadata,
                index,
                name,
                address,
                accept_textures,
                &desktop,
            )
            .await
        })
    }

    fn remove_server<'a>(
        &'a self,
        _: &'a Engine,
        metadata: &'a InstanceMetadata,
        index: usize,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(async move {
            let desktop = self.desktop();
            let metadata =
                synced::state::with_synced_options(&desktop, metadata).await?;
            synced::remove_server_by_index(&metadata, index, &desktop).await
        })
    }

    fn remove_managed_servers<'a>(
        &'a self,
        _: &'a Engine,
        instance_id: &'a str,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(async move {
            synced::remove_managed_servers(instance_id, &self.desktop()).await
        })
    }

    fn discard_modpack_servers<'a>(
        &'a self,
        _: &'a Engine,
        instance_id: &'a str,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(async move {
            synced::discard_modpack_servers(instance_id, &self.desktop()).await
        })
    }

    fn desync_server<'a>(
        &'a self,
        _: &'a Engine,
        instance_id: &'a str,
        server_id: &'a str,
        mode: DesyncServerMode,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(async move {
            synced::desync_server(&self.desktop(), instance_id, server_id, mode)
                .await
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use engine::data::{InstanceLink, InstanceSyncedOption, ModLoader};
    use engine::instance;
    use std::path::Path;

    /// The App's engine, with the synced options that need Mojang's version manifest turned off
    /// so nothing reaches the network.
    pub(crate) async fn offline_engine(dir: &Path) -> Desktop {
        let engine = crate::data_dir::tests::open(dir, None).await.unwrap();
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

    pub(crate) async fn create(engine: &Desktop) -> InstanceMetadata {
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

    #[tokio::test]
    async fn the_removing_hook_waits_for_a_synced_options_pass_in_flight() {
        let dir = tempfile::tempdir().unwrap();
        let engine = offline_engine(dir.path()).await;

        let pass = engine.synced_options().lock().await;
        // What an instance's removal holds before it deletes the folder.
        let mut removing = std::pin::pin!(engine.hooks().removing(&engine));
        assert!(futures::poll!(removing.as_mut()).is_pending());
        drop(pass);
        removing.await;
    }
}
