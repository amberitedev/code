use crate::Engine;
use crate::api::instance::CONFIG_DIRECTORY;
use crate::event::emit::{emit_instance, emit_warning};
use crate::event::{EventSink, InstancePayloadType};
use crate::state::{
    DirectoryInfo, InstanceInstallStage, ProjectType, attached_world_data,
};
use crate::worlds::WorldType;
use contracts::Side;
use dashmap::{DashMap, mapref::entry::Entry};
use notify::event::{AccessKind, AccessMode};
use notify::{
    Config, Event, EventHandler, EventKind, RecommendedWatcher, RecursiveMode,
    Watcher, WatcherKind,
};
use notify_debouncer_mini::{
    DebounceEventResult, DebouncedEvent, Debouncer, new_debouncer_opt,
};
use sqlx::SqlitePool;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Weak;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{
    RwLock,
    mpsc::{Receiver, channel},
};

use super::adapters::sqlite::instance_rows;

pub struct FileWatcher {
    side: Side,
    watcher: RwLock<Watching>,
    content_syncs: DashMap<String, bool>,
}

/// Each instance's folder is watched alone, never what is in it, so a server's world is not.
/// The folders of [`watched_subpaths`] are watched whole once they exist; until then the
/// folder's own watch sees them appear.
struct Watching {
    debouncer: Debouncer<WriteOnlyWatcher>,
    /// Each instance's folder, by its watched spelling and its canonical one: a platform can name
    /// events by the real path (FSEvents gives `/private/var` for `/var`, or a linked data
    /// folder's target).
    roots: HashMap<PathBuf, Root>,
    folders: HashSet<PathBuf>,
}

struct Root {
    /// The folder as watched.
    path: PathBuf,
    instance_id: String,
}

impl Watching {
    /// The instance whose folder holds `path`, and the name of the entry in that folder it is or
    /// is under: `None` for the folder itself.
    fn locate<'a>(&self, path: &'a Path) -> Option<(&str, Option<&'a OsStr>)> {
        path.ancestors().find_map(|folder| {
            let root = self.roots.get(folder)?;
            let mut inside = path.strip_prefix(folder).ok()?.components();
            Some((
                root.instance_id.as_str(),
                inside.next().map(|c| c.as_os_str()),
            ))
        })
    }

    /// Watch `folder` and everything in it, if it is a folder now and not watched yet.
    fn attach(&mut self, folder: &Path) {
        if self.folders.contains(folder) || !folder.is_dir() {
            return;
        }
        match self
            .debouncer
            .watcher()
            .watch(folder, RecursiveMode::Recursive)
        {
            Ok(()) => {
                self.folders.insert(folder.to_owned());
            }
            Err(e) => tracing::error!(
                "Failed to watch directory for watcher {folder:?}: {e}"
            ),
        }
    }

    fn unwatch(&mut self, path: &Path) {
        self.folders.remove(path);
        if let Err(error) = self.debouncer.watcher().unwatch(path)
            && !matches!(
                error.kind,
                notify::ErrorKind::WatchNotFound
                    | notify::ErrorKind::PathNotFound
            )
        {
            tracing::warn!(
                "Failed to unwatch instance path {}: {error}",
                path.display()
            );
        }
    }
}

/// Wraps the platform watcher and drops read-only access events before they
/// reach the debouncer.
///
/// The inotify backend reports `IN_OPEN`/`IN_CLOSE_NOWRITE`, and
/// `notify-debouncer-mini` discards event kinds, so without this every read
/// of a watched file (e.g. the locale indexer reading `options.txt`) looks
/// like a change and can re-trigger the reader in a loop.
pub struct WriteOnlyWatcher(RecommendedWatcher);

fn is_read_only_access(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Access(access)
        if *access != AccessKind::Close(AccessMode::Write))
}

impl Watcher for WriteOnlyWatcher {
    fn new<F: EventHandler>(
        mut event_handler: F,
        config: Config,
    ) -> notify::Result<Self> {
        RecommendedWatcher::new(
            move |event: notify::Result<Event>| {
                if event
                    .as_ref()
                    .is_ok_and(|event| is_read_only_access(&event.kind))
                {
                    return;
                }
                event_handler.handle_event(event);
            },
            config,
        )
        .map(Self)
    }

    fn watch(
        &mut self,
        path: &Path,
        recursive_mode: RecursiveMode,
    ) -> notify::Result<()> {
        self.0.watch(path, recursive_mode)
    }

    fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
        self.0.unwatch(path)
    }

    fn configure(&mut self, option: Config) -> notify::Result<bool> {
        self.0.configure(option)
    }

    fn kind() -> WatcherKind {
        RecommendedWatcher::kind()
    }
}

fn queue_content_sync(instance_id: String, state: Arc<Engine>) {
    match state.file_watcher.content_syncs.entry(instance_id.clone()) {
        Entry::Occupied(mut entry) => {
            *entry.get_mut() = true;
            return;
        }
        Entry::Vacant(entry) => {
            entry.insert(false);
        }
    }
    tokio::spawn(async move {
        loop {
            let result: crate::Result<bool> = async {
                let Some(instance) = instance_rows::get_instance_by_id(
                    &instance_id,
                    &state.pool,
                )
                .await?
                else {
                    return Ok(true);
                };
                if matches!(
                    instance.install_stage,
                    InstanceInstallStage::MinecraftInstalling
                        | InstanceInstallStage::PackInstalling
                ) {
                    return Ok(false);
                }
                if let Some(mut queued) =
                    state.file_watcher.content_syncs.get_mut(&instance_id)
                {
                    *queued = false;
                }
                crate::state::sync_content_files(&instance_id, &state).await?;
                Ok(true)
            }
            .await;
            if matches!(result, Ok(false)) {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            if let Err(error) = result {
                tracing::warn!(
                    instance_id,
                    "Content reconciliation deferred: {error}"
                );
            }
            let _ = emit_instance(
                &state.events,
                &instance_id,
                InstancePayloadType::Synced,
            )
            .await;
            // The listing's read-only marks follow the index this sync just wrote.
            state.events.send(crate::event::Event::FilesChanged {
                instance_id: instance_id.clone(),
            });
            match state.file_watcher.content_syncs.entry(instance_id.clone()) {
                Entry::Occupied(mut entry) if *entry.get() => {
                    *entry.get_mut() = false
                }
                Entry::Occupied(entry) => {
                    entry.remove();
                    break;
                }
                Entry::Vacant(_) => break,
            }
        }
    });
}

pub(crate) fn init_watcher(
    side: Side,
) -> crate::Result<(FileWatcher, Receiver<DebounceEventResult>)> {
    let (tx, rx) = channel(1);
    let file_watcher = new_debouncer_opt::<_, WriteOnlyWatcher>(
        notify_debouncer_mini::Config::default()
            .with_timeout(Duration::from_secs_f32(1.0)),
        move |res: DebounceEventResult| {
            tx.blocking_send(res).ok();
        },
    )?;

    Ok((
        FileWatcher {
            side,
            watcher: RwLock::new(Watching {
                debouncer: file_watcher,
                roots: HashMap::new(),
                folders: HashSet::new(),
            }),
            content_syncs: DashMap::new(),
        },
        rx,
    ))
}

/// The loop ends when the engine drops, which drops the watcher and closes `rx`.
pub(crate) fn watch_instance_events(
    state: Weak<Engine>,
    mut rx: Receiver<DebounceEventResult>,
) {
    tokio::task::spawn(async move {
        let span = tracing::span!(tracing::Level::INFO, "init_watcher");
        tracing::info!(parent: &span, "Initing watcher");
        while let Some(res) = rx.recv().await {
            let Some(state) = state.upgrade() else {
                return;
            };
            let _span = span.enter();

            match res {
                Ok(events) => {
                    attach_or_drop(&state.file_watcher, &events).await;
                    let watching = state.file_watcher.watcher.read().await;
                    let mut visited_instances = Vec::new();
                    let mut visited_screenshot_instances = Vec::new();
                    let mut changed_files =
                        HashMap::<String, BTreeSet<String>>::new();
                    let mut touched = BTreeSet::new();

                    for e in &events {
                        if let Some((instance_id, first_file_name)) =
                            watching.locate(&e.path)
                        {
                            let instance_id = instance_id.to_owned();
                            touched.insert(instance_id.clone());
                            let sync_content =
                                first_file_name.as_ref().is_some_and(|name| {
                                    content_folders(state.side)
                                        .any(|folder| *name == folder)
                                });
                            if !sync_content
                                && let Some(name) =
                                    first_file_name.and_then(|x| x.to_str())
                            {
                                changed_files
                                    .entry(instance_id.clone())
                                    .or_default()
                                    .insert(name.to_owned());
                            }
                            let is_screenshot_event = first_file_name
                                .as_ref()
                                .is_some_and(|name| *name == "screenshots");
                            if first_file_name
                                .as_ref()
                                .is_some_and(|x| *x == "crash-reports")
                                && e.path
                                    .extension()
                                    .as_ref()
                                    .is_some_and(|x| *x == "txt")
                            {
                                crash_task(
                                    instance_id,
                                    state.pool.clone(),
                                    state.events.clone(),
                                );
                            } else if (is_screenshot_event
                                && !visited_screenshot_instances
                                    .contains(&instance_id))
                                || (!is_screenshot_event
                                    && !visited_instances
                                        .contains(&instance_id))
                            {
                                let event = if first_file_name
                                    .as_ref()
                                    .is_some_and(|x| *x == "servers.dat")
                                {
                                    Some(InstancePayloadType::ServersUpdated)
                                } else if first_file_name
                                    .as_ref()
                                    .is_some_and(|x| *x == "screenshots")
                                {
                                    Some(
                                        InstancePayloadType::ScreenshotsUpdated,
                                    )
                                } else if first_file_name.as_ref().is_some_and(
                                    |x| {
                                        *x == "saves"
                                            && e.path
                                                .file_name()
                                                .as_ref()
                                                .is_some_and(|x| {
                                                    *x == "level.dat"
                                                })
                                    },
                                ) {
                                    tracing::info!(
                                        "World updated: {}",
                                        e.path.display()
                                    );
                                    let world = e
                                        .path
                                        .parent()
                                        .unwrap()
                                        .file_name()
                                        .unwrap()
                                        .to_string_lossy()
                                        .to_string();
                                    if !e.path.is_file() {
                                        let instance_id = instance_id.clone();
                                        let world = world.clone();
                                        let pool = state.pool.clone();
                                        tokio::spawn(async move {
                                            if let Err(e) = attached_world_data::AttachedWorldData::remove_for_world(
												&instance_id,
												WorldType::Singleplayer,
												&world,
												&pool
											).await {
												tracing::warn!("Failed to remove AttachedWorldData for '{world}': {e}")
											}
                                        });
                                    }
                                    Some(InstancePayloadType::WorldUpdated {
                                        world,
                                    })
                                } else if first_file_name.as_ref().is_none_or(
                                    |x| *x != "saves" && *x != CONFIG_DIRECTORY,
                                ) {
                                    Some(InstancePayloadType::Synced)
                                } else {
                                    None
                                };
                                if let Some(event) = event {
                                    let emit_instance_id = instance_id.clone();
                                    if sync_content {
                                        queue_content_sync(
                                            emit_instance_id,
                                            Arc::clone(&state),
                                        );
                                    } else {
                                        let events = state.events.clone();
                                        tokio::spawn(async move {
                                            let _ = emit_instance(
                                                &events,
                                                &emit_instance_id,
                                                event,
                                            )
                                            .await;
                                        });
                                    }
                                    if is_screenshot_event {
                                        visited_screenshot_instances
                                            .push(instance_id);
                                    } else {
                                        visited_instances.push(instance_id);
                                    }
                                }
                            }
                        }
                    }

                    for (instance_id, names) in changed_files {
                        state.events.send(crate::event::Event::PathsChanged {
                            instance_id,
                            paths: names.into_iter().collect(),
                        });
                    }
                    for instance_id in touched {
                        state.events.send(crate::event::Event::FilesChanged {
                            instance_id,
                        });
                    }
                }
                Err(error) => tracing::warn!("Unable to watch file: {error}"),
            }
        }
    });
}

pub(crate) async fn watch_instances_init(
    watcher: &FileWatcher,
    dirs: &DirectoryInfo,
    pool: &sqlx::SqlitePool,
) {
    let Ok(instances) = instance_rows::list_instances(pool).await else {
        return;
    };

    for instance in instances {
        watch_instance_folder(&instance.id, &instance.path, watcher, dirs)
            .await;
    }
}

pub(crate) async fn watch_instance_folder(
    instance_id: &str,
    instance_path: &str,
    watcher: &FileWatcher,
    dirs: &DirectoryInfo,
) {
    let full_instance_path = dirs.instances_dir().join(instance_path);

    let Ok(metadata) = tokio::fs::metadata(&full_instance_path).await else {
        return;
    };

    if !metadata.is_dir() {
        return;
    }

    let mut watching = watcher.watcher.write().await;
    // First, so a folder made while the others are attached is seen.
    if let Err(e) = watching
        .debouncer
        .watcher()
        .watch(&full_instance_path, RecursiveMode::NonRecursive)
    {
        tracing::error!(
            "Failed to watch root instance directory for watcher {full_instance_path:?}: {e}"
        );
    }
    let root = || Root {
        path: full_instance_path.clone(),
        instance_id: instance_id.to_owned(),
    };
    if let Ok(canonical) = full_instance_path.canonicalize() {
        watching.roots.insert(canonical, root());
    }
    watching.roots.insert(full_instance_path.clone(), root());
    for sub_path in watched_subpaths(watcher.side) {
        watching.attach(&full_instance_path.join(sub_path));
    }
}

/// An allowlisted folder that appeared is watched; one that went is no longer.
async fn attach_or_drop(watcher: &FileWatcher, events: &[DebouncedEvent]) {
    let mut watching = watcher.watcher.write().await;
    let folders: Vec<_> = events
        .iter()
        .filter_map(|event| {
            let root = watching.roots.get(event.path.parent()?)?;
            let name = event.path.file_name()?;
            watched_subpaths(watcher.side)
                .any(|sub| name == sub)
                .then(|| root.path.join(name))
        })
        .collect();
    for folder in folders {
        if folder.is_dir() {
            watching.attach(&folder);
        } else if watching.folders.contains(&folder) {
            watching.unwatch(&folder);
        }
    }
}

/// The folders whose files are content. A server's datapacks are inside its world, which is not
/// watched: it is written all the time while the server runs.
fn content_folders(side: Side) -> impl Iterator<Item = &'static str> {
    let folders: Vec<_> = match side {
        Side::Client => {
            ProjectType::iterator().map(|x| x.get_folder()).collect()
        }
        Side::Server => vec![
            ProjectType::Mod.get_folder(),
            ProjectType::Plugin.get_folder(),
        ],
    };
    folders.into_iter()
}

fn watched_subpaths(side: Side) -> impl Iterator<Item = &'static str> {
    let client_only: &[&str] = match side {
        Side::Client => &["saves", "screenshots"],
        Side::Server => &[],
    };
    content_folders(side)
        .chain(["crash-reports"])
        .chain(client_only.iter().copied())
        .chain([CONFIG_DIRECTORY])
}

pub(crate) async fn unwatch_instance_folder(
    instance_id: &str,
    instance_path: &str,
    watcher: &FileWatcher,
    dirs: &DirectoryInfo,
) {
    let full_instance_path = dirs.instances_dir().join(instance_path);
    let mut watching = watcher.watcher.write().await;
    if watching
        .roots
        .get(&full_instance_path)
        .is_none_or(|root| root.instance_id != instance_id)
    {
        return;
    }
    watching
        .roots
        .retain(|_, root| root.path != full_instance_path);
    for sub_path in watched_subpaths(watcher.side) {
        watching.unwatch(&full_instance_path.join(sub_path));
    }
    watching.unwatch(&full_instance_path);
}

fn crash_task(instance_id: String, pool: SqlitePool, events: EventSink) {
    tokio::task::spawn(async move {
        let res = async {
            let Some(instance) =
                instance_rows::get_instance_by_id(&instance_id, &pool).await?
            else {
                return Ok(());
            };

            if instance.install_stage == InstanceInstallStage::Installed {
                emit_warning(&events, &format!(
					"Instance {} has crashed! Visit the logs page to see a crash report.",
					instance.name
				))
				.await?;
            }

            Ok::<(), crate::Error>(())
        }
        .await;

        match res {
            Ok(()) => {}
            Err(err) => {
                tracing::warn!("Unable to send crash report to frontend: {err}")
            }
        };
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::instances::adapters::sqlite::content_rows;
    use crate::state::instances::commands::set_instance_install_stage;
    use crate::state::tests::{add_instance, engine, link_dir, write};
    use notify_debouncer_mini::DebouncedEventKind;

    #[tokio::test(flavor = "multi_thread")]
    async fn a_content_folder_is_watched_once_it_appears_and_never_made() {
        crate::install_crypto_provider();
        let tmp = tempfile::tempdir().unwrap();
        let engine = engine(tmp.path().to_path_buf()).await;
        add_instance(&engine, "a", "Survival").await;
        let root = engine.directories.instances_dir().join("Survival");
        std::fs::create_dir_all(&root).unwrap();
        let mut events = engine.events.subscribe();
        watch_instance_folder(
            "a",
            "Survival",
            &engine.file_watcher,
            &engine.directories,
        )
        .await;
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);

        tokio::time::timeout(Duration::from_secs(60), async {
            // The folder's own watch sees `mods` appear; a file added to it afterwards is seen
            // only through the watch then attached to it.
            for jar in ["a.jar", "b.jar"] {
                write(&root.join("mods").join(jar));
                let path = format!("mods/{jar}");
                while !content_rows::get_instance_files("a", &engine.pool)
                    .await
                    .unwrap()
                    .iter()
                    .any(|file| file.relative_path == path)
                {
                    loop {
                        if let Ok(crate::event::Event::Instance(payload)) =
                            events.recv().await
                            && payload.instance_id == "a"
                            && matches!(
                                payload.event,
                                InstancePayloadType::Synced
                            )
                        {
                            break;
                        }
                    }
                }
            }
        })
        .await
        .expect("mods/b.jar was never seen");
    }

    /// A platform can name an event by the real path of a folder watched through a link, as
    /// FSEvents does for a linked data folder. An event is the instance's by its folder, not by
    /// where that folder is.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_content_folder_named_by_its_real_path_is_watched() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        std::fs::create_dir_all(real.join("Survival")).unwrap();
        let instances = tmp.path().join("profiles");
        link_dir(&real, &instances);
        let dirs = DirectoryInfo::init(
            tmp.path().join("config"),
            instances.clone(),
            None,
            None,
        )
        .await
        .unwrap();
        // Its events go nowhere, so only the event below attaches anything.
        let (watcher, _) = init_watcher(Side::Client).unwrap();
        watch_instance_folder("a", "Survival", &watcher, &dirs).await;

        let mods = real.join("Survival").join("mods");
        std::fs::create_dir(&mods).unwrap();
        let appeared = DebouncedEvent {
            path: mods.canonicalize().unwrap(),
            kind: DebouncedEventKind::Any,
        };
        attach_or_drop(&watcher, &[appeared]).await;
        let watched = instances.join("Survival").join("mods");
        assert!(watcher.watcher.read().await.folders.contains(&watched));

        // A Core's instances are its servers, in `servers/<id>`.
        let servers = tmp.path().join("servers");
        std::fs::create_dir_all(servers.join("srv")).unwrap();
        let core_dirs = DirectoryInfo::init(
            tmp.path().join("core"),
            servers.clone(),
            None,
            None,
        )
        .await
        .unwrap();
        watch_instance_folder("b", "srv", &watcher, &core_dirs).await;
        for (path, instance, first) in [
            (mods.canonicalize().unwrap().join("a.jar"), "a", "mods"),
            (watched.join("a.jar"), "a", "mods"),
            (
                servers.join("srv").join("plugins").join("b.jar"),
                "b",
                "plugins",
            ),
        ] {
            let watching = watcher.watcher.read().await;
            assert_eq!(
                watching.locate(&path),
                Some((instance, Some(OsStr::new(first)))),
                "{path:?}"
            );
        }
        unwatch_instance_folder("b", "srv", &watcher, &core_dirs).await;

        unwatch_instance_folder("a", "Survival", &watcher, &dirs).await;
        let watching = watcher.watcher.read().await;
        assert!(watching.roots.is_empty() && watching.folders.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn two_engines_queue_the_same_instance_id_separately() {
        crate::install_crypto_provider();
        let tmp = tempfile::tempdir().unwrap();
        let (a_dir, b_dir) = (tmp.path().join("a"), tmp.path().join("b"));
        write(&b_dir.join("profiles/Survival/mods/b.jar"));

        tokio::time::timeout(Duration::from_secs(60), async {
            let (a, b) = (engine(a_dir).await, engine(b_dir).await);
            add_instance(&a, "same", "Survival").await;
            add_instance(&b, "same", "Survival").await;
            // A's sync keeps its queue entry while the instance installs.
            set_instance_install_stage(
                "same",
                InstanceInstallStage::MinecraftInstalling,
                &a.pool,
            )
            .await
            .unwrap();

            let mut events = b.events.subscribe();
            queue_content_sync("same".to_string(), Arc::clone(&a));
            queue_content_sync("same".to_string(), Arc::clone(&b));
            while !matches!(
                events.recv().await,
                Ok(crate::event::Event::FilesChanged { instance_id }) if instance_id == "same"
            ) {}
            assert!(
                !content_rows::get_instance_files("same", &b.pool)
                    .await
                    .unwrap()
                    .is_empty()
            );
        })
        .await
        .expect("the second engine's sync never ran");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_listing_is_read_again_once_the_sync_indexed_it() {
        crate::install_crypto_provider();
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("profiles/Survival/mods/a.jar"));
        let engine = engine(tmp.path().to_path_buf()).await;
        add_instance(&engine, "a", "Survival").await;
        let mut events = engine.events.subscribe();

        // Nothing watches the folder, so only the sync can say its files changed.
        queue_content_sync("a".to_string(), Arc::clone(&engine));
        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                if let Ok(crate::event::Event::FilesChanged { instance_id }) =
                    events.recv().await
                    && instance_id == "a"
                {
                    break;
                }
            }
        })
        .await
        .expect("the sync never said the files changed");
        let files = content_rows::get_instance_files("a", &engine.pool)
            .await
            .unwrap();
        assert!(files.iter().any(|file| file.relative_path == "mods/a.jar"));
    }
}
