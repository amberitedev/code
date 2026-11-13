//! Keeps the desktop's synced options, packs and servers through a modpack update: the user's
//! files are captured before the pack overwrites them and reapplied after.

use engine::{
    Engine, PackOverrides, PackServers, PackUpdateHooks, access::BoxFuture,
    instance,
};

pub struct PackHooks;

impl PackUpdateHooks for PackHooks {
    fn before_pack_update<'a>(
        &'a self,
        engine: &'a Engine,
        instance_id: &'a str,
    ) -> BoxFuture<'a, engine::Result<()>> {
        Box::pin(instance::prepare_instance_update(engine, instance_id))
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
                    instance::discard_modpack_servers(instance_id, engine)
                        .await?;
                }
                PackServers::Pack(path) => {
                    instance::capture_modpack_server_override(
                        engine,
                        instance_id,
                        path,
                    )
                    .await?;
                }
            }
            if let Err(error) = instance::capture_game_options_pack_base(
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
            instance::reconcile_instance_after_pack_update(engine, instance_id)
                .await
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use engine::data::{
        InstanceLink, InstanceMetadata, InstanceSyncedOption, ModLoader,
    };
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
            instance::set_global_synced_option(&engine, option, false, None)
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

        PackHooks.before_pack_update(&engine, &id).await.unwrap();
        std::fs::write(&options, "fov:0.0\n").unwrap();
        let overrides = PackOverrides {
            servers: PackServers::Untouched,
            game_options: None,
        };
        PackHooks
            .after_pack_update(&engine, &id, overrides)
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(&options).unwrap(), "fov:0.75\n");
    }
}
