use super::super::options::{
    get_global_options, instance_dir, instance_is_running, sha1_bytes,
};
use super::PackLibrary;
use super::storage::{read_library, write_library};
use anyhow::{Context, bail};
use engine::ErrorKind;
use engine::state::instances::adapters::{filesystem, sqlite::content_rows};
use engine::state::{Engine, InstanceMetadata};
use parking_lot::Mutex;
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, Weak};
use std::time::Duration;
use tokio::sync::{MutexGuard, oneshot};
use tokio::time::Instant;

#[derive(Default)]
struct Queue {
    instances: BTreeSet<String>,
    waiters:
        BTreeMap<String, Vec<oneshot::Sender<Result<(), Arc<engine::Error>>>>>,
    retry_at: BTreeMap<String, Instant>,
    all: bool,
    running: bool,
}

#[derive(Default)]
pub(crate) struct PackSyncWorker {
    queue: Mutex<Queue>,
    pub(super) revision: AtomicU64,
    engine: OnceLock<Weak<Engine>>,
}

impl PackSyncWorker {
    /// Lets the worker run; requests made before this only queue.
    pub(crate) fn bind(&self, engine: Weak<Engine>) {
        let _ = self.engine.set(engine);
    }
}

pub fn queue_reconciliation(state: &Engine, instance_id: &str) {
    crate::hooks::synced_options(state)
        .packs
        .queue
        .lock()
        .instances
        .insert(instance_id.to_owned());
    start(state);
}

pub(super) fn queue_all(state: &Engine) {
    crate::hooks::synced_options(state).packs.queue.lock().all = true;
    start(state);
}

pub async fn flush(
    state: &Engine,
    instance_id: &str,
) -> Result<(), Arc<engine::Error>> {
    let (sender, receiver) = oneshot::channel();
    {
        let mut queue = crate::hooks::synced_options(state).packs.queue.lock();
        if let Some(retry_at) = queue.retry_at.get(instance_id) {
            let remaining = retry_at.saturating_duration_since(Instant::now());
            if !remaining.is_zero() {
                return Err(Arc::new(
                    ErrorKind::RateLimited {
                        retry_in: remaining,
                    }
                    .into(),
                ));
            }
        }
        queue.instances.insert(instance_id.to_owned());
        queue
            .waiters
            .entry(instance_id.to_owned())
            .or_default()
            .push(sender);
    }
    start(state);
    receiver
        .await
        .context("pack sync worker stopped before completing the request")?
}

fn start(state: &Engine) {
    let Some(state) = crate::hooks::synced_options(state)
        .packs
        .engine
        .get()
        .and_then(Weak::upgrade)
    else {
        return;
    };
    {
        let mut queue = crate::hooks::synced_options(&state).packs.queue.lock();
        if queue.running {
            return;
        }
        queue.running = true;
    }
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let all = std::mem::take(
                &mut crate::hooks::synced_options(&state)
                    .packs
                    .queue
                    .lock()
                    .all,
            );
            if all {
                match engine::state::list_instances(state.pool()).await {
                    Ok(instances) => {
                        crate::hooks::synced_options(&state)
                            .packs
                            .queue
                            .lock()
                            .instances
                            .extend(
                                instances
                                    .into_iter()
                                    .map(|metadata| metadata.instance.id),
                            );
                    }
                    Err(error) => {
                        tracing::warn!(
                            "Could not enumerate pack sync targets: {error}"
                        );
                        crate::hooks::synced_options(&state)
                            .packs
                            .queue
                            .lock()
                            .all = true;
                        tokio::time::sleep(std::time::Duration::from_secs(1))
                            .await;
                    }
                }
            }
            let (instance_id, mut pending) = {
                let mut queue =
                    crate::hooks::synced_options(&state).packs.queue.lock();
                let next = queue
                    .waiters
                    .keys()
                    .chain(queue.instances.iter())
                    .find(|id| !queue.retry_at.contains_key(*id))
                    .cloned();
                let Some(instance_id) = next else {
                    if queue.all {
                        continue;
                    }
                    queue.running = false;
                    break;
                };
                queue.instances.remove(&instance_id);
                let pending =
                    queue.waiters.remove(&instance_id).unwrap_or_default();
                (instance_id, pending)
            };
            let result =
                super::reconciliation::run_queued(&instance_id, &state)
                    .await
                    .map_err(Arc::new);
            if let Err(error) = &result {
                match error.downcast_ref::<engine::ErrorKind>() {
                    Some(engine::ErrorKind::PackSyncChanged) => {
                        let mut queue = crate::hooks::synced_options(&state)
                            .packs
                            .queue
                            .lock();
                        queue.instances.insert(instance_id.clone());
                        if !pending.is_empty() {
                            queue
                                .waiters
                                .entry(instance_id)
                                .or_default()
                                .extend(pending);
                        }
                        continue;
                    }
                    Some(engine::ErrorKind::RateLimited { retry_in }) => {
                        let retry_at = Instant::now()
                            + (*retry_in).max(Duration::from_secs(1));
                        {
                            let mut queue =
                                crate::hooks::synced_options(&state)
                                    .packs
                                    .queue
                                    .lock();
                            queue.instances.insert(instance_id.clone());
                            queue
                                .retry_at
                                .insert(instance_id.clone(), retry_at);
                            pending.extend(
                                queue
                                    .waiters
                                    .remove(&instance_id)
                                    .unwrap_or_default(),
                            );
                        }
                        let state = state.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep_until(retry_at).await;
                            crate::hooks::synced_options(&state)
                                .packs
                                .queue
                                .lock()
                                .retry_at
                                .remove(&instance_id);
                            start(&state);
                        });
                    }
                    _ => tracing::warn!(
                        "Could not reconcile synced packs for {instance_id}: {error}"
                    ),
                }
            }
            for sender in pending {
                let _ = sender.send(result.clone());
            }
        }
    });
}

/// Releases the canonical-state lock for preparation and rejects outdated results.
pub(super) struct Preparation<'a> {
    state: &'a Engine,
    guard: Option<MutexGuard<'a, ()>>,
    metadata: &'a InstanceMetadata,
}

impl<'a> Preparation<'a> {
    pub(super) async fn new(
        state: &'a Engine,
        metadata: &'a InstanceMetadata,
    ) -> Self {
        Self {
            state,
            metadata,
            guard: Some(crate::hooks::synced_options(state).lock().await),
        }
    }

    pub(super) async fn run<T>(
        &mut self,
        library: &PackLibrary,
        work: impl Future<Output = engine::Result<T>>,
    ) -> engine::Result<T> {
        write_library(library, self.state).await?;
        let revision = crate::hooks::synced_options(self.state)
            .packs
            .revision
            .load(Ordering::Acquire);
        self.validate_metadata().await?;
        let before = fingerprint(self.metadata, self.state).await?;
        self.guard.take();
        let result = work.await;
        self.guard =
            Some(crate::hooks::synced_options(self.state).lock().await);
        self.validate_metadata().await?;
        if crate::hooks::synced_options(self.state)
            .packs
            .revision
            .load(Ordering::Acquire)
            != revision
            || fingerprint(self.metadata, self.state).await? != before
        {
            bail!(ErrorKind::PackSyncChanged);
        }
        result
    }

    async fn validate_metadata(&self) -> engine::Result<()> {
        let current = engine::state::get_instance(
            &self.metadata.instance.id,
            &self.state.pool(),
        )
        .await?;
        if current.as_ref().is_none_or(|current| {
            metadata_key(current) != metadata_key(self.metadata)
        }) {
            bail!(ErrorKind::PackSyncChanged);
        }
        Ok(())
    }

    pub(super) async fn library(&self) -> engine::Result<PackLibrary> {
        read_library(self.state).await
    }
}

fn metadata_key(metadata: &InstanceMetadata) -> serde_json::Value {
    let content_set = &metadata.applied_content_set;
    serde_json::json!([
        metadata.instance.path,
        metadata.instance.install_stage,
        content_set.id,
        content_set.source_kind,
        content_set.game_version,
        content_set.loader,
        content_set.loader_version,
        metadata.synced_options,
        engine::instance::ensure_metadata_content_unlocked(metadata).is_ok(),
    ])
}

async fn fingerprint(
    metadata: &InstanceMetadata,
    state: &Engine,
) -> engine::Result<Vec<(String, String)>> {
    let directory = state.directories.instances_dir();
    let instance_path = metadata.instance.path.clone();
    let mut files = tokio::task::spawn_blocking(move || {
        filesystem::scan_content_files(&directory, &instance_path)
    })
    .await??
    .into_iter()
    .map(|file| (file.relative_path, file.hash_cache_key))
    .collect::<Vec<_>>();
    let options = instance_dir(metadata, state).join("options.txt");
    match tokio::fs::read(options).await {
        Ok(bytes) => files.push(("options.txt".to_owned(), sha1_bytes(&bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    files.push((
        "global-options".to_owned(),
        serde_json::to_string(&get_global_options(state).await?)?,
    ));
    files.push((
        "running".to_owned(),
        instance_is_running(metadata, state).await?.to_string(),
    ));
    files.push((
        "pending".to_owned(),
        super::super::options::pending::contains(
            &metadata.instance.id,
            engine::state::SyncedOption::ResourcePacks,
            state,
        )
        .await?
        .to_string(),
    ));
    for id in content_rows::get_locked_instance_file_ids(
        &metadata.instance.id,
        state.pool(),
    )
    .await?
    {
        files.push(("locked-file".to_owned(), id));
    }
    let entries = content_rows::get_content_entries(
        &metadata.applied_content_set.id,
        state.pool(),
    )
    .await?;
    for entry in entries {
        files.push((
            format!("source:{}", entry.id),
            format!("{:?}:{}", entry.file_id, entry.source_kind.as_str()),
        ));
    }
    files.sort();
    Ok(files)
}
