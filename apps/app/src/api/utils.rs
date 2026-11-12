use crate::engine_handle::EngineHandle;
use engine::{emit_warning, handler};
use tauri::{Manager, Runtime};
use tokio::sync::broadcast;

use crate::api::Result;
use crate::api::oauth_utils::auth_code_reply::{
    normalize_deep_link_command, parse_auth_deeplink, submit_deeplink,
};
use crate::server::events::{AmberiteCommand, AppEvent, Command};
use dashmap::DashMap;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct DebugInfoExports(DashMap<(String, String), CancellationToken>);

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("utils")
        .setup(|app, _| {
            app.manage(DebugInfoExports::default());
            Ok(())
        })
        .on_event(|app, event| {
            if let tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::Destroyed,
                ..
            } = event
            {
                for export in &app.state::<DebugInfoExports>().0 {
                    if &export.key().0 == label {
                        export.value().cancel();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            export_debug_info,
            cancel_debug_info_export,
            progress_bars_list,
            get_opening_command,
        ])
        .build()
}

// Lists active progress bars
// Create a new HashMap with the same keys
// Values provided should not be used directly, as they are not guaranteed to be up-to-date
#[tauri::command]
pub async fn progress_bars_list(
    events: tauri::State<'_, engine::EventSink>,
) -> Result<DashMap<uuid::Uuid, engine::LoadingBar>> {
    Ok(events.loading_bars())
}

#[tauri::command]
pub async fn export_debug_info<R: Runtime>(
    window: tauri::Window<R>,
    state: tauri::State<'_, DebugInfoExports>,
    handle: tauri::State<'_, EngineHandle>,
    id: String,
    path: PathBuf,
    on_progress: tauri::ipc::Channel<
        engine::debug_info::DebugInfoExportProgress,
    >,
) -> Result<bool> {
    let key = (window.label().to_string(), id);
    let cancel = CancellationToken::new();
    match state.0.entry(key.clone()) {
        dashmap::mapref::entry::Entry::Vacant(entry) => {
            entry.insert(cancel.clone());
        }
        dashmap::mapref::entry::Entry::Occupied(_) => {
            return Err(std::io::Error::other(
                "Debug info export is already running",
            )
            .into());
        }
    }
    let engine_state = handle.get().await;
    let result = engine::debug_info::export_debug_info(
        &engine_state,
        path,
        cancel,
        move |progress| {
            let _ = on_progress.send(progress);
        },
    )
    .await;
    state.0.remove(&key);
    Ok(result?)
}

#[tauri::command]
pub fn cancel_debug_info_export<R: Runtime>(
    window: tauri::Window<R>,
    state: tauri::State<'_, DebugInfoExports>,
    id: String,
) {
    if let Some(cancel) = state.0.get(&(window.label().to_string(), id)) {
        cancel.cancel();
    }
}

/// The Amberite command behind a deep link, or `None` for anything else.
fn amberite_command(command: &str) -> Option<AmberiteCommand> {
    let path = command.strip_prefix("amberite://")?.trim_end_matches('/');
    let code = path.strip_prefix("approve/")?;
    let valid = code.len() <= 9
        && code.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-'
        });
    valid.then(|| AmberiteCommand::ApproveCore {
        code: code.to_uppercase(),
    })
}

async fn parse_opening_command(
    events: &engine::EventSink,
    command: &str,
) -> Result<Command> {
    if let Some(command) = amberite_command(command) {
        return Ok(Command::Amberite(command));
    }
    Ok(Command::Modrinth(
        handler::parse_command(events, command).await?,
    ))
}

// Get opening command
// For example, if a user clicks on an .mrpack to open the app.
// This should be called once and only when the app is done booting up and ready to receive a command
// Returns a Command struct- see events.js
#[tauri::command]
#[cfg(target_os = "macos")]
pub async fn get_opening_command(
    state: tauri::State<'_, crate::macos::deep_link::InitialPayload>,
    events: tauri::State<'_, engine::EventSink>,
) -> Result<Option<Command>> {
    if crate::dev::config().is_some() {
        return Ok(None);
    }
    let payload = state.payload.lock().await;
    let cmd_arg = std::env::args_os()
        .nth(1)
        .map(|path| path.to_string_lossy().to_string());

    return if let Some(payload) = payload.as_ref() {
        opening_command(&events, payload).await
    } else if let Some(cmd_arg) = cmd_arg {
        opening_command(&events, &cmd_arg).await
    } else {
        Ok(None)
    };
}

#[tauri::command]
#[cfg(not(target_os = "macos"))]
pub async fn get_opening_command(
    events: tauri::State<'_, engine::EventSink>,
) -> Result<Option<Command>> {
    if crate::dev::config().is_some() {
        return Ok(None);
    }
    // Tauri is not CLI, we use arguments as path to file to call
    let cmd_arg = std::env::args_os().nth(1);

    let cmd_arg = cmd_arg.map(|path| path.to_string_lossy().to_string());
    if let Some(cmd) = cmd_arg {
        return opening_command(&events, &cmd).await;
    }
    Ok(None)
}

// helper function called when redirected by a weblink (ie: modrith://do-something) or when redirected by a .mrpack file (in which case its a filepath)
// We hijack the deep link library (which also contains functionality for instance-checking)
pub async fn handle_command<R: Runtime>(
    app: tauri::AppHandle<R>,
    command: String,
) -> Result<()> {
    let events = app.state::<engine::EventSink>();
    let command = normalize_deep_link_command(&command);
    if accept_auth_deeplink(&events, &command).await {
        return Ok(());
    }

    tracing::info!("handle command: {command}");
    // Amberite's own links go straight to the frontend; the engine only knows the Modrinth ones.
    if let Some(command) = amberite_command(&command) {
        let _ = app
            .state::<broadcast::Sender<AppEvent>>()
            .send(AppEvent::Command(Command::Amberite(command)));
        if let Some(window) = app.get_window("main") {
            let _ = window.set_focus();
        }
        return Ok(());
    }
    Ok(engine::handler::parse_and_emit_command(&events, &command).await?)
}

async fn opening_command(
    events: &engine::EventSink,
    command: &str,
) -> Result<Option<Command>> {
    let command = normalize_deep_link_command(command);
    if accept_auth_deeplink(events, &command).await {
        return Ok(None);
    }

    tracing::info!("opening command {command}");
    Ok(Some(parse_opening_command(events, &command).await?))
}

async fn accept_auth_deeplink(
    events: &engine::EventSink,
    command: &str,
) -> bool {
    let Some((code, nonce)) = parse_auth_deeplink(command) else {
        return false;
    };

    tracing::info!("Handling Modrinth auth deep link");
    if !submit_deeplink(code, &nonce) {
        const MESSAGE: &str = "Couldn't finish signing in. Please try again.";
        if let Err(error) = emit_warning(events, MESSAGE).await {
            tracing::error!("{MESSAGE} ({error})");
        }
    }
    true
}
