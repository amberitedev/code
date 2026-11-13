//! The desktop follows engine events: Discord and friends status show the running instance, the
//! main window hides on launch and returns on exit, and it comes forward for an opened link.
//! Synced options, packs, screenshots, setting labels and onboarding catch up with each instance
//! as it is created, edited, changed on disk, removed, installed or closed.

use std::collections::BTreeSet;
use std::sync::{Arc, Weak};

use engine::data::ProjectType;
use engine::install::{InstallJobKind, InstallJobStatus};
use engine::{
    DesktopEvent, Engine, Event, InstancePayload, InstancePayloadType,
    ProcessPayload, ProcessPayloadType, instance,
};
use tauri::{AppHandle, Manager};
use tokio::sync::broadcast::{self, Receiver, error::RecvError};

use crate::presence::{Presence, show_presence, start_presence};
use crate::server::events::AppEvent;

#[derive(Debug, PartialEq)]
struct Reaction {
    presence: bool,
    window: Option<WindowChange>,
}

#[derive(Debug, PartialEq)]
enum WindowChange {
    Minimize,
    Restore,
    Focus,
}

/// Connects presence, starts synced options and reacts to `events`, which must be subscribed
/// before the engine starts so no event is missed.
pub fn start(
    engine: &Arc<Engine>,
    presence: Arc<Presence>,
    app: AppHandle,
    events: Receiver<Event>,
) {
    tauri::async_runtime::spawn(start_presence(
        Arc::clone(engine),
        Arc::clone(&presence),
    ));
    let app_events = app.state::<broadcast::Sender<AppEvent>>().inner().clone();
    crate::synced_options::start_synced_options(engine, move || {
        let _ = app_events.send(AppEvent::GameOptionLocalesUpdated);
    });
    tauri::async_runtime::spawn(react(
        Arc::downgrade(engine),
        presence,
        app,
        events,
    ));
}

async fn react(
    engine: Weak<Engine>,
    presence: Arc<Presence>,
    app: AppHandle,
    mut events: Receiver<Event>,
) {
    loop {
        let reaction = match events.recv().await {
            Ok(event) => {
                let Some(engine) = engine.upgrade() else {
                    return;
                };
                if !matches!(event, Event::Log(_) | Event::Loading(_)) {
                    tauri::async_runtime::spawn(follow(
                        Arc::clone(&engine),
                        event.clone(),
                    ));
                }
                reaction(&engine, &event).await
            }
            Err(RecvError::Lagged(_)) => {
                if let Some(engine) = engine.upgrade() {
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) =
                            crate::synced_options::reconcile_all_synced_options(
                                &engine,
                            )
                            .await
                        {
                            tracing::warn!(
                                "Failed to reconcile synced options after missed events: {error}"
                            );
                        }
                    });
                }
                Some(Reaction {
                    presence: true,
                    window: None,
                })
            }
            Err(RecvError::Closed) => return,
        };
        let Some(reaction) = reaction else { continue };
        if reaction.presence
            && let Some(engine) = engine.upgrade()
        {
            show_presence(&engine, &presence).await;
        }
        if let Some(change) = reaction.window {
            change_window(&app, change);
        }
    }
}

async fn reaction(engine: &Engine, event: &Event) -> Option<Reaction> {
    match event {
        Event::Process(process) => {
            let window = match crate::settings::get(engine).await {
                Ok(settings) => match process.event {
                    ProcessPayloadType::Launched => settings
                        .hide_on_process_start
                        .then_some(WindowChange::Minimize),
                    ProcessPayloadType::Finished => settings
                        .refocus_on_game_close
                        .then_some(WindowChange::Restore),
                },
                Err(error) => {
                    tracing::warn!("Failed to read window settings: {error}");
                    None
                }
            };
            Some(Reaction {
                presence: true,
                window,
            })
        }
        Event::Desktop(DesktopEvent::Command(_)) => Some(Reaction {
            presence: false,
            window: Some(WindowChange::Focus),
        }),
        _ => None,
    }
}

async fn follow(engine: Arc<Engine>, event: Event) {
    let (instance_id, result) = match event {
        Event::Instance(InstancePayload { instance_id, event }) => {
            let result = match event {
                InstancePayloadType::Created => {
                    if let Err(error) =
                        crate::onboarding::mark_created_instance(&engine).await
                    {
                        tracing::warn!(
                            "Failed to mark instance creation in onboarding checklist: {error}"
                        );
                    }
                    crate::synced_options::reconcile_instance_synced_options(
                        &engine,
                        &instance_id,
                    )
                    .await
                }
                InstancePayloadType::Edited => {
                    crate::synced_options::reconcile_instance_synced_options(
                        &engine,
                        &instance_id,
                    )
                    .await
                }
                InstancePayloadType::Removed => {
                    crate::hooks::forget_instance(&engine, &instance_id);
                    if let Err(error) =
                        crate::library::hidden_instances::forget_instance(
                            &instance_id,
                            engine.pool(),
                        )
                        .await
                    {
                        tracing::warn!(
                            "Failed to forget hidden instance {instance_id}: {error}"
                        );
                    }
                    crate::synced_options::remove_generated_instance_files(
                        &instance_id,
                        &engine,
                    )
                    .await
                }
                _ => Ok(()),
            };
            (instance_id, result)
        }
        Event::Process(ProcessPayload {
            instance_id,
            event: ProcessPayloadType::Finished,
            ..
        }) => {
            let result =
                crate::synced_options::reconcile_instance_synced_options(
                    &engine,
                    &instance_id,
                )
                .await;
            (instance_id, result)
        }
        Event::InstallJob(job)
            if matches!(job.status, InstallJobStatus::Succeeded) =>
        {
            crate::synced_options::queue_game_locale_index(&engine);
            if let Some(instance_id) = &job.instance_id
                && matches!(job.kind, InstallJobKind::BulkUpdateContent)
            {
                crate::synced_options::reconcile_synced_packs_after_content_change(
                    &engine,
                    instance_id,
                )
                .await;
            }
            return;
        }
        Event::FilesChanged { instance_id, paths } => {
            files_changed(&engine, &instance_id, &paths).await;
            return;
        }
        _ => return,
    };
    if let Err(error) = result {
        tracing::warn!(
            "Synced options could not catch up with {instance_id}: {error}"
        );
    }
}

async fn files_changed(engine: &Engine, instance_id: &str, paths: &[String]) {
    let mut option_files = BTreeSet::new();
    let (mut packs, mut locales, mut screenshots) = (false, false, false);
    for path in paths {
        let first = path.split('/').next().unwrap_or_default();
        match first {
            "options.txt" => {
                locales = true;
                option_files.insert(first);
            }
            "command_history.txt" | "hotbar.nbt" | "servers.dat" => {
                option_files.insert(first);
            }
            "screenshots" => screenshots = true,
            _ if ProjectType::iterator()
                .any(|project_type| project_type.get_folder() == first) =>
            {
                locales = true;
                packs |= matches!(first, "resourcepacks" | "datapacks");
            }
            _ => {}
        }
    }
    if packs {
        crate::synced_options::queue_synced_pack_reconciliation(
            engine,
            instance_id,
        );
    }
    if locales {
        crate::synced_options::queue_game_locale_index(engine);
    }
    if screenshots
        && let Err(error) =
            crate::screenshots::reconcile_screenshots(engine, instance_id).await
    {
        tracing::warn!(
            "Failed to reconcile screenshots after filesystem change: {error}"
        );
    }
    for file_name in option_files {
        if let Err(error) = crate::synced_options::reconcile_synced_option_file(
            engine,
            instance_id,
            file_name,
        )
        .await
        {
            tracing::warn!(
                "Failed to reconcile {file_name} after filesystem change: {error}"
            );
        }
    }
}

fn change_window(app: &AppHandle, change: WindowChange) {
    // The ads webview makes "main" a multi-webview window, which only get_window finds.
    let Some(window) = app.get_window("main") else {
        return;
    };
    let changed = match change {
        WindowChange::Minimize => window.minimize(),
        WindowChange::Restore => {
            window.unminimize().and_then(|()| window.set_focus())
        }
        WindowChange::Focus => window.set_focus(),
    };
    if let Err(error) = changed {
        tracing::warn!("Failed to change the main window: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{ProcessPayload, prelude::CommandPayload};

    fn process(event: ProcessPayloadType) -> Event {
        Event::Process(ProcessPayload {
            instance_id: "instance".to_string(),
            uuid: uuid::Uuid::nil().to_string(),
            event,
            message: String::new(),
        })
    }

    #[tokio::test]
    async fn process_events_update_presence_and_follow_the_window_settings() {
        engine::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let config = crate::data_dir::engine_config(
            dir.path().to_path_buf(),
            engine::EventSink::new(),
            None,
        )
        .await
        .unwrap();
        let engine = Engine::new(config).await.unwrap();

        for (hide, refocus, on_launch, on_exit) in [
            (true, false, Some(WindowChange::Minimize), None),
            (false, true, None, Some(WindowChange::Restore)),
        ] {
            let mut settings = crate::settings::get(&engine).await.unwrap();
            settings.hide_on_process_start = hide;
            settings.refocus_on_game_close = refocus;
            crate::settings::set(&engine, settings).await.unwrap();

            for (event, window) in [
                (ProcessPayloadType::Launched, on_launch),
                (ProcessPayloadType::Finished, on_exit),
            ] {
                assert_eq!(
                    reaction(&engine, &process(event)).await,
                    Some(Reaction {
                        presence: true,
                        window,
                    })
                );
            }
        }

        let opened =
            Event::Desktop(DesktopEvent::Command(CommandPayload::RunMRPack {
                path: "pack.mrpack".to_string(),
            }));
        assert_eq!(
            reaction(&engine, &opened).await,
            Some(Reaction {
                presence: false,
                window: Some(WindowChange::Focus),
            })
        );
    }

    async fn next_instance_event(
        events: &mut Receiver<Event>,
        wanted: fn(&InstancePayloadType) -> bool,
    ) -> Event {
        loop {
            let event = events.recv().await.unwrap();
            if let Event::Instance(payload) = &event
                && wanted(&payload.event)
            {
                return event;
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn created_and_removed_instances_are_followed() {
        let dir = tempfile::tempdir().unwrap();
        let engine = crate::hooks::tests::offline_engine(dir.path()).await;
        let mut events = engine.events.subscribe();
        let created = crate::hooks::tests::create(&engine).await;
        let id = created.instance.id;

        let event = next_instance_event(&mut events, |event| {
            matches!(event, InstancePayloadType::Created)
        })
        .await;
        let onboarding = async || {
            crate::onboarding::get(&engine)
                .await
                .unwrap()
                .has_created_instance
        };
        assert!(!onboarding().await);
        follow(Arc::clone(&engine), event).await;
        assert!(onboarding().await);

        let generated =
            crate::synced_options::get_synced_options_folder(&engine)
                .await
                .unwrap()
                .join("servers/generated")
                .join(id.replace(':', "_"));
        std::fs::create_dir_all(&generated).unwrap();
        let hidden = crate::library::hidden_instances::set_instance_hidden(
            "user",
            &id,
            true,
            engine.pool(),
        );
        hidden.await.unwrap();
        let folder = instance::get_full_path(&engine, &id).await.unwrap();
        instance::remove(&engine, &id).await.unwrap();
        let removed = next_instance_event(&mut events, |event| {
            matches!(event, InstancePayloadType::Removed)
        })
        .await;
        follow(Arc::clone(&engine), removed).await;
        assert!(!generated.exists());
        let hidden = crate::library::hidden_instances::get_hidden_instance_ids(
            "user",
            engine.pool(),
        );
        assert!(hidden.await.unwrap().is_empty());

        // An edit can still be in flight for an instance removed under it.
        let edited = Event::Instance(InstancePayload {
            instance_id: id.clone(),
            event: InstancePayloadType::Edited,
        });
        follow(Arc::clone(&engine), edited).await;
        assert!(!folder.exists());
        crate::synced_options::reconcile_instance_synced_options(&engine, &id)
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn following_a_synced_instance_settles() {
        let dir = tempfile::tempdir().unwrap();
        let engine = crate::hooks::tests::offline_engine(dir.path()).await;
        let id = crate::hooks::tests::create(&engine).await.instance.id;
        // A library that places packs makes every pack pass emit Synced.
        let packs = crate::synced_options::get_synced_options_folder(&engine)
            .await
            .unwrap()
            .join("packs");
        std::fs::create_dir_all(&packs).unwrap();
        std::fs::write(
            packs.join("packs.json"),
            format!(r#"{{"packs":{{}},"instances":{{"{id}":{{}}}}}}"#),
        )
        .unwrap();
        crate::synced_options::start_synced_options(&engine, || {});
        let mut events = engine.events.subscribe();

        follow(
            Arc::clone(&engine),
            Event::Instance(InstancePayload {
                instance_id: id.clone(),
                event: InstancePayloadType::Synced,
            }),
        )
        .await;
        let mut synced = 0;
        while let Ok(event) = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            events.recv(),
        )
        .await
        {
            let event = event.unwrap();
            if matches!(
                &event,
                Event::Instance(InstancePayload {
                    event: InstancePayloadType::Synced,
                    ..
                })
            ) {
                synced += 1;
                assert!(synced < 5, "following Synced keeps re-running packs");
            }
            follow(Arc::clone(&engine), event).await;
        }
    }
}
