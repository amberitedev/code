//! Live updates: the console socket `GET /servers/{id}/ws`, and
//! `POST /servers/{id}/v1/logs/clear` (no body, no response). Mirrors `Archon.Websocket.v0` in
//! `packages/api-client/src/modules/archon/types.ts`, with only the events Core sends.

use std::sync::Arc;

use axum::{
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use super::{AppState, activity::Action};
use crate::{
    access::{Caller, Changed, Level, StreamGuard, activity, now, sessions},
    api::error::{ApiError, ErrorCode},
    servers::console,
};

/// Console lines a new console socket starts with.
const BACKLOG: usize = 200;
/// How often an open console checks that its session has not expired.
const RECHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(600);

/// `GET /servers/{id}/ws`: the console. Sends the last lines and every new one as `log` events,
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

/// A message the App sends on the console socket. Mirrors Archon.Websocket.v0.WSOutgoingMessage.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum WsClientMessage {
    /// Mirrors Archon.Websocket.v0.WSAuthMessage.
    Auth {},
    /// Mirrors Archon.Websocket.v0.WSCommandMessage.
    Command { cmd: String },
}

/// A message Core sends on the console socket. Mirrors Archon.Websocket.v0.WSEvent.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum WsEvent {
    /// Mirrors Archon.Websocket.v0.WSBackupProgressEvent.
    BackupProgress {
        id: Uuid,
        task: BackupTask,
        state: BackupState,
        progress: f64,
        start_time: Option<i64>,
        finish_time: Option<i64>,
    },
    /// Mirrors Archon.Websocket.v0.WSLogEvent.
    Log { stream: LogStream, message: String },
    /// Mirrors Archon.Websocket.v0.WSAuthOkEvent.
    AuthOk,
    /// Mirrors Archon.Websocket.v0.WSInstallationResultEvent.
    InstallationResult(InstallationResult),
    /// Mirrors Archon.Websocket.v0.WSInstallProgressEvent.
    InstallProgress { items: Vec<InstallProgressItem> },
}

/// Mirrors Archon.Websocket.v0.BackupTask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupTask {
    Create,
    Restore,
}

/// Mirrors Archon.Websocket.v0.BackupState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupState {
    Ongoing,
    Done,
    Failed,
}

/// Mirrors the `stream` union of Archon.Websocket.v0.WSLogEvent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogStream {
    Stdout,
}

/// Serializes as `"result": "ok"` or `"result": "err"` next to the event tag. Mirrors
/// Archon.Websocket.v0.WSInstallationResultOkEvent and WSInstallationResultErrEvent, with
/// Amberite's `game_version` on a pack install that moved the server to another Minecraft.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "result", rename_all = "lowercase")]
pub enum InstallationResult {
    Ok {
        #[serde(skip_serializing_if = "Option::is_none")]
        game_version: Option<GameVersionChange>,
    },
    Err {
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

/// The Minecraft version a pack install moved a server from and to. Amberite-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ts_rs::TS)]
pub struct GameVersionChange {
    pub from: String,
    pub to: String,
}

/// A loader or platform. Mirrors the `platform` union of Archon.Websocket.v0.InstallProgressPlatformKey
/// and Archon.Sync.v1.WorldContentPlatform, which take the same values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentPlatform {
    Forge,
    Neoforge,
    Fabric,
    Quilt,
    Paper,
    Vanilla,
}

/// What an install progress item is installing. Mirrors Archon.Websocket.v0.InstallProgressKey.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallProgressKey {
    /// Mirrors Archon.Websocket.v0.InstallProgressModrinthModpackKey.
    ModrinthModpack {
        project_id: String,
        version_id: String,
    },
    /// Mirrors Archon.Websocket.v0.InstallProgressLocalModpackKey.
    LocalModpack { filename: String },
    /// Mirrors Archon.Websocket.v0.InstallProgressPlatformKey.
    Platform {
        platform: ContentPlatform,
        platform_version: String,
        game_version: String,
    },
}

/// Mirrors Archon.Websocket.v0.InstallProgressItem.
#[derive(Debug, Clone, Serialize)]
pub struct InstallProgressItem {
    pub world_id: Uuid,
    pub key: InstallProgressKey,
    pub id: String,
    pub progress: Option<f64>,
    pub error: Option<String>,
}
