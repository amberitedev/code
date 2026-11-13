//! The desktop follows engine events: Discord and friends status show the running instance, the
//! main window hides on launch and returns on exit, and it comes forward for an opened link.

use std::sync::{Arc, Weak};

use engine::{DesktopEvent, Engine, Event, ProcessPayloadType};
use tauri::{AppHandle, Manager};
use tokio::sync::broadcast::{Receiver, error::RecvError};

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

/// Connects presence and reacts to `events`, which must be subscribed before the engine starts so
/// no event is missed.
pub fn start(engine: &Arc<Engine>, app: AppHandle, events: Receiver<Event>) {
    tauri::async_runtime::spawn(Arc::clone(engine).start_presence());
    tauri::async_runtime::spawn(react(Arc::downgrade(engine), app, events));
}

async fn react(
    engine: Weak<Engine>,
    app: AppHandle,
    mut events: Receiver<Event>,
) {
    loop {
        let reaction = match events.recv().await {
            Ok(event) => {
                let Some(engine) = engine.upgrade() else {
                    return;
                };
                reaction(&engine, &event).await
            }
            Err(RecvError::Lagged(_)) => Some(Reaction {
                presence: true,
                window: None,
            }),
            Err(RecvError::Closed) => return,
        };
        let Some(reaction) = reaction else { continue };
        if reaction.presence
            && let Some(engine) = engine.upgrade()
        {
            engine.show_presence().await;
        }
        if let Some(change) = reaction.window {
            change_window(&app, change);
        }
    }
}

async fn reaction(engine: &Engine, event: &Event) -> Option<Reaction> {
    match event {
        Event::Process(process) => {
            let window = match engine::settings::get(engine).await {
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
            let mut settings = engine::settings::get(&engine).await.unwrap();
            settings.hide_on_process_start = hide;
            settings.refocus_on_game_close = refocus;
            engine::settings::set(&engine, settings).await.unwrap();

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
}
