//! Live updates: the console socket `GET /servers/{id}/ws`, and
//! `POST /servers/{id}/v1/logs/clear` (no body, no response). The events it sends are in
//! `crate::event`.

use std::sync::Arc;

use axum::{
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::{IntoResponse, Response},
};
use tokio::sync::{broadcast, mpsc};

use super::{
    AppState,
    activity::Action,
    servers::{ServerPowerState, power_state},
};
use crate::{
    access::{Caller, Changed, Level, StreamGuard, activity, now, sessions},
    api::error::{ApiError, ErrorCode},
    servers::console,
};

use crate::event::{LogStream, PowerState, WsClientMessage, WsEvent};

/// Console lines a new console socket starts with.
const BACKLOG: usize = 200;
/// How often an open console checks that its session has not expired.
const RECHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(600);
/// How often an open console looks for a power change. A server's phase changes in many places,
/// so the socket reads it, which costs a lock, instead of each of them announcing it.
const POWER_EVERY: std::time::Duration = std::time::Duration::from_millis(500);

/// `GET /servers/{id}/ws`: the console. Sends the server's power state as `power-state` events,
/// and the last console lines and every new one as `log` events,
/// leaving out the answers to Core's health check, and passes `command` messages to the server. Editors and the owner only: it carries chat and
/// player IPs. The upgrade request carries the session; the first message must still be `auth`,
/// as the hosting API has it, but its token is not read. The stream closes when its session ends
/// or its user loses the role, and when the session expires.
pub async fn console(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Response {
    // Before the role check, so a change from here on is never missed; the socket checks again
    // before it shows anything, for the changes before.
    let changes = state.access.changed.subscribe();
    if let Err(error) = caller.require(&state.db, &id, Level::Editor).await {
        return error.into_response();
    }
    let Some(guard) = state.access.open_stream(&caller) else {
        return ApiError::new(
            ErrorCode::RateLimited,
            "This session has too many consoles open.",
        )
        .into_response();
    };
    upgrade.on_upgrade(move |socket| {
        console_socket(state, caller, guard, changes, id, socket)
    })
}

/// Whether `caller` may still read `server`'s console: their session is live and their role
/// still allows it.
async fn still_allowed(
    state: &AppState,
    caller: &Caller,
    server: &str,
) -> bool {
    if let Caller::Session { id, .. } = caller {
        let exists = sqlx::query(
            "SELECT 1 FROM sessions WHERE id = ?1 AND expires_at > ?2 AND last_used_at > ?2 - ?3",
        )
        .bind(id)
        .bind(now())
        .bind(sessions::IDLE_S)
        .fetch_optional(&state.db)
        .await;
        if !matches!(exists, Ok(Some(_))) {
            return false;
        }
    }
    matches!(
        caller.level(&state.db, server).await,
        Ok(Some(level)) if level >= Level::Editor
    )
}

async fn console_socket(
    state: Arc<AppState>,
    caller: Caller,
    _guard: StreamGuard,
    mut changes: broadcast::Receiver<Changed>,
    id: String,
    mut socket: WebSocket,
) {
    let first = socket.recv().await;
    let authed = matches!(
        first,
        Some(Ok(Message::Text(text)))
            if matches!(serde_json::from_str(&text), Ok(WsClientMessage::Auth { .. }))
    );
    if !authed
        || !still_allowed(&state, &caller, &id).await
        || send(&mut socket, &WsEvent::AuthOk).await.is_err()
    {
        return;
    }
    // The App shows a server as stopped until told otherwise, so it learns the power state
    // before it treats the page as ready.
    let mut phase = state.servers.phase(&id);
    let mut power = power_of(&state, &id).await;
    if send(&mut socket, &WsEvent::PowerState { state: power })
        .await
        .is_err()
    {
        return;
    }
    // The App treats the server page as ready once it knows what is installing, so it is told
    // straight away, even when that is nothing.
    let installing = WsEvent::InstallProgress {
        items: state.content.activity(&id).items,
    };
    if send(&mut socket, &installing).await.is_err() {
        return;
    }
    let mut recheck = tokio::time::interval(RECHECK_EVERY);
    recheck.tick().await;
    let mut power_check = tokio::time::interval(POWER_EVERY);
    power_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let path = state.servers.paths().console_log(&id);
    let (backlog, end) = console::tail(&path, BACKLOG).unwrap_or_default();
    for line in backlog.into_iter().filter(|line| console::for_people(line)) {
        if send(&mut socket, &log(line)).await.is_err() {
            return;
        }
    }
    let (sender, mut lines) = mpsc::channel(256);
    console::follow(path, end, sender);
    let mut installs = state.content.events.subscribe();
    loop {
        tokio::select! {
            event = installs.recv() => {
                if let Ok((server, event)) = event
                    && server == id
                    && send(&mut socket, &event).await.is_err()
                {
                    return;
                }
            }
            line = lines.recv() => {
                let Some(line) = line else { return };
                if console::for_people(&line) && send(&mut socket, &log(line)).await.is_err() {
                    return;
                }
            }
            change = changes.recv() => {
                let concerns_me = match change {
                    Ok(Changed::User(user)) => caller.user_id() == Some(user.as_str()),
                    Ok(Changed::Everyone) | Err(_) => true,
                };
                if concerns_me && !still_allowed(&state, &caller, &id).await {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
            }
            _ = power_check.tick() => {
                let now = state.servers.phase(&id);
                if now == phase {
                    continue;
                }
                phase = now;
                let next = power_of(&state, &id).await;
                if next != power {
                    power = next;
                    if send(&mut socket, &WsEvent::PowerState { state: power }).await.is_err() {
                        return;
                    }
                }
            }
            _ = recheck.tick() => {
                if !still_allowed(&state, &caller, &id).await {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
            }
            message = socket.recv() => match message {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(WsClientMessage::Command { cmd }) = serde_json::from_str(&text) {
                        match state.servers.command(&id, &cmd).await {
                            Ok(()) => {
                                activity::log(
                                    &state.db,
                                    &caller,
                                    Some(&id),
                                    Action::ConsoleCommand { command: cmd },
                                )
                                .await;
                            }
                            Err(error) => {
                                tracing::debug!(server = %id, %error, "console command");
                            }
                        }
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return,
            },
        }
    }
}

/// `id`'s power state as the hosting API's socket names it. A killed server reads as stopped.
async fn power_of(state: &AppState, id: &str) -> PowerState {
    match power_state(state, id)
        .await
        .unwrap_or(ServerPowerState::Offline)
    {
        ServerPowerState::Starting => PowerState::Starting,
        ServerPowerState::Running => PowerState::Running,
        ServerPowerState::Stopping => PowerState::Stopping,
        ServerPowerState::Crashed => PowerState::Crashed,
        ServerPowerState::Offline | ServerPowerState::Killed => {
            PowerState::Stopped
        }
    }
}

fn log(message: String) -> WsEvent {
    WsEvent::Log {
        stream: LogStream::Stdout,
        message,
    }
}

async fn send(
    socket: &mut WebSocket,
    event: &WsEvent,
) -> Result<(), axum::Error> {
    let text = serde_json::to_string(event).expect("events serialize");
    socket.send(Message::Text(text.into())).await
}
