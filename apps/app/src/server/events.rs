//! The `/events` socket: every engine and App event, as JSON, to every client. One local user sees
//! everything, so there is no subscribe protocol.

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    routing::MethodRouter,
};
use engine::{
    DesktopEvent, Event, EventSink, FriendPayload,
    InstanceGroupsChangedPayload, data::OnboardingChecklist,
    prelude::CommandPayload,
};
use serde::Serialize;
use tokio::sync::broadcast::{self, Receiver, error::RecvError};

use super::endpoints::events::EventSocket;

const APP_EVENT_CAPACITY: usize = 256;

/// What the App reports, beside the engine's events.
#[derive(Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
#[cfg_attr(
    test,
    ts(tag = "type", content = "payload", rename_all = "snake_case")
)]
pub enum AppEvent {
    Friend(#[cfg_attr(test, ts(type = "FriendPayload"))] FriendPayload),
    Notification(#[cfg_attr(test, ts(type = "unknown"))] serde_json::Value),
    OnboardingChecklist(
        #[cfg_attr(test, ts(type = "OnboardingChecklist"))] OnboardingChecklist,
    ),
    InstanceGroupsChanged(
        #[cfg_attr(test, ts(type = "InstanceGroupsChangedPayload"))]
        InstanceGroupsChangedPayload,
    ),
    Command(Command),
    GameOptionLocalesUpdated,
    /// This socket fell behind and lost events: refetch what is shown.
    Resync,
}

/// A link or file the App was asked to open: a Modrinth one, which the engine parses, or an
/// Amberite one. The frontend tells them apart by `event`.
#[derive(Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(untagged)]
pub enum Command {
    Modrinth(#[cfg_attr(test, ts(type = "CommandPayload"))] CommandPayload),
    Amberite(AmberiteCommand),
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "event")]
pub enum AmberiteCommand {
    /// `amberite://approve/<code>`: open "Add Core" with the code a Core is showing filled in.
    ApproveCore { code: String },
}

/// Everything `/events` sends.
#[derive(Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(untagged)]
pub enum LocalEvent {
    Engine(#[cfg_attr(test, ts(type = "EngineEvent"))] Event),
    App(AppEvent),
}

impl From<Event> for LocalEvent {
    fn from(event: Event) -> Self {
        let Event::Desktop(event) = event else {
            return Self::Engine(event);
        };
        Self::App(match event {
            DesktopEvent::Friend(payload) => AppEvent::Friend(payload),
            DesktopEvent::Notification(payload) => {
                AppEvent::Notification(payload)
            }
            DesktopEvent::OnboardingChecklist(checklist) => {
                AppEvent::OnboardingChecklist(checklist)
            }
            DesktopEvent::InstanceGroupsChanged(payload) => {
                AppEvent::InstanceGroupsChanged(payload)
            }
            DesktopEvent::Command(command) => {
                AppEvent::Command(Command::Modrinth(command))
            }
            DesktopEvent::GameOptionLocalesUpdated => {
                AppEvent::GameOptionLocalesUpdated
            }
        })
    }
}

/// Where the socket's events come from: the engine's sink and the App's own sender.
#[derive(Clone)]
pub struct Events {
    pub engine: EventSink,
    pub app: broadcast::Sender<AppEvent>,
}

impl Default for Events {
    fn default() -> Self {
        Self {
            engine: EventSink::new(),
            app: broadcast::Sender::new(APP_EVENT_CAPACITY),
        }
    }
}

/// Subscribes before answering the upgrade, so the socket misses nothing raised after it opens.
pub fn route(events: Events) -> (&'static str, MethodRouter) {
    contracts::server::route::<EventSocket, _, _, _>(
        move |upgrade: WebSocketUpgrade| {
            let engine = events.engine.subscribe();
            let app = events.app.subscribe();
            async move {
                upgrade.on_upgrade(move |socket| forward(socket, engine, app))
            }
        },
    )
}

async fn forward(
    mut socket: WebSocket,
    mut engine: Receiver<Event>,
    mut app: Receiver<AppEvent>,
) {
    loop {
        let event = tokio::select! {
            event = next(&mut engine, &mut app) => event,
            message = socket.recv() => match message {
                Some(Ok(_)) => continue,
                _ => return,
            },
        };
        let Some(event) = event else { return };
        let text = match serde_json::to_string(&event) {
            Ok(text) => text,
            Err(error) => {
                tracing::warn!(%error, "Failed to encode an app event");
                continue;
            }
        };
        if socket.send(Message::Text(text.into())).await.is_err() {
            return;
        }
    }
}

/// `None` once the App is shutting down. A receiver that fell behind lost events, so the client
/// is told to refetch instead.
async fn next(
    engine: &mut Receiver<Event>,
    app: &mut Receiver<AppEvent>,
) -> Option<LocalEvent> {
    let received = tokio::select! {
        event = engine.recv() => event.map(LocalEvent::from),
        event = app.recv() => event.map(LocalEvent::App),
    };
    match received {
        Ok(event) => Some(event),
        Err(RecvError::Lagged(skipped)) => {
            tracing::warn!("An events socket fell {skipped} events behind");
            Some(LocalEvent::App(AppEvent::Resync))
        }
        Err(RecvError::Closed) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_lagged_receiver_gets_one_resync_then_carries_on() {
        let events = Events::default();
        let mut engine = events.engine.subscribe();
        let mut app = events.app.subscribe();
        for _ in 0..=APP_EVENT_CAPACITY {
            let _ = events.app.send(AppEvent::GameOptionLocalesUpdated);
        }
        assert!(matches!(
            next(&mut engine, &mut app).await,
            Some(LocalEvent::App(AppEvent::Resync))
        ));
        assert!(matches!(
            next(&mut engine, &mut app).await,
            Some(LocalEvent::App(AppEvent::GameOptionLocalesUpdated))
        ));
    }
}
