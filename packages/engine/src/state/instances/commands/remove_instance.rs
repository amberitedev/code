use crate::ErrorKind;
use crate::state::Engine;
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::util::io;
use anyhow::bail;

pub(crate) async fn remove_instance(
    instance_id: &str,
    state: &Engine,
) -> crate::Result<()> {
    let instance = instance_rows::get_instance_by_id(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    // A host pass in flight, such as the App's synced options, writes into the folder this deletes.
    let _host_guard = state.hooks().removing(state).await;
    let _content_lock = state.instance_locks.lock_content(instance_id).await;
    let _store_lock = state.content_store.files_lock.lock().await;
    let _store_lease = state.content_store.lease().await;
    if crate::state::instance_has_running_process(instance_id, state).await? {
        bail!(ErrorKind::InputError(
            "stop this instance before removing it".to_string(),
        ));
    }
    state.content_store.recover(Some(instance_id)).await?;

    delete_instance_row_and_locks(&instance.id, state).await?;
    super::super::watcher::unwatch_instance_folder(
        &instance.id,
        &instance.path,
        &state.file_watcher,
        &state.directories,
    )
    .await;

    let path = state.directories.instances_dir().join(&instance.path);
    if path.exists() {
        io::remove_dir_all(&path).await?;
    }

    Ok(())
}

async fn delete_instance_row_and_locks(
    instance_id: &str,
    state: &Engine,
) -> crate::Result<()> {
    // Keep these together so deleted instances cannot leave stale entries in the per-instance lock maps.
    instance_rows::delete_instance_by_id(instance_id, &state.pool).await?;
    state.instance_locks.remove(instance_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::BoxFuture;
    use crate::state::tests::{add_instance, config};
    use crate::state::{EngineConfig, InstanceHooks};
    use std::sync::Arc;
    use tokio::sync::Notify;

    /// Says when a removal enters `removing`, and holds it there until released.
    #[derive(Default)]
    struct Gate {
        entered: Notify,
        released: Notify,
    }

    impl InstanceHooks for Gate {
        fn removing<'a>(
            &'a self,
            _engine: &'a Engine,
        ) -> BoxFuture<'a, Box<dyn Send + 'a>> {
            Box::pin(async {
                self.entered.notify_one();
                self.released.notified().await;
                Box::new(()) as Box<dyn Send>
            })
        }
    }

    #[tokio::test]
    async fn the_folder_stays_until_the_removing_hook_lets_go() {
        let dir = tempfile::tempdir().unwrap();
        let gate = Arc::new(Gate::default());
        let engine = Engine::new(EngineConfig {
            hooks: Some(gate.clone()),
            ..config(dir.path().to_path_buf())
        })
        .await
        .unwrap();
        add_instance(&engine, "a", "A").await;
        let folder = engine.directories.instances_dir().join("A");
        std::fs::create_dir_all(&folder).unwrap();

        let removal = tokio::spawn({
            let engine = Arc::clone(&engine);
            async move { remove_instance("a", &engine).await }
        });
        gate.entered.notified().await;
        assert!(folder.exists());
        gate.released.notify_one();
        removal.await.unwrap().unwrap();
        assert!(!folder.exists());
    }
}
