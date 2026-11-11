use engine::{
    emit_warning, handler,
    prelude::{CommandPayload, app_db_backup_dir},
};
use serde::Serialize;
use tauri::{Manager, Runtime};
use tauri_plugin_opener::OpenerExt;

use crate::api::Result;
use crate::api::oauth_utils::auth_code_reply::{
    normalize_deep_link_command, parse_auth_deeplink, submit_deeplink,
};
use dashmap::DashMap;
use engine::prelude::canonicalize;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;
use url::Url;

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
            is_network_metered,
            highlight_in_folder,
            open_path,
            show_launcher_logs_folder,
            show_app_db_backups_folder,
            export_debug_info,
            cancel_debug_info_export,
            progress_bars_list,
            get_opening_command,
            super::thumbnails::get_image_thumbnail,
            cache_remote_icon,
        ])
        .build()
}

#[tauri::command]
pub async fn cache_remote_icon(source: &str) -> Result<String> {
    let path = engine::instance::cache_remote_icon(source).await?;
    Ok(tauri_convert_file_src(&path)?.to_string())
}

#[tauri::command]
pub async fn is_network_metered() -> Result<bool> {
    Ok(engine::prelude::is_network_metered().await?)
}

// Lists active progress bars
// Create a new HashMap with the same keys
// Values provided should not be used directly, as they are not guaranteed to be up-to-date
#[tauri::command]
pub async fn progress_bars_list()
-> Result<DashMap<uuid::Uuid, engine::LoadingBar>> {
    let res = engine::EventState::list_progress_bars().await?;
    Ok(res)
}

#[tauri::command]
pub async fn highlight_in_folder<R: Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = app.opener().reveal_item_in_dir(path) {
            tracing::error!("Failed to highlight file in folder: {}", e);
        }
    })
    .await
    .ok();
}

#[tauri::command]
pub async fn open_path<R: Runtime>(app: tauri::AppHandle<R>, path: PathBuf) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) =
            app.opener().open_path(path.to_string_lossy(), None::<&str>)
        {
            tracing::error!("Failed to open path: {}", e);
        }
    })
    .await
    .ok();
}

#[tauri::command]
pub async fn show_launcher_logs_folder<R: Runtime>(app: tauri::AppHandle<R>) {
    if let Some(state) = engine::State::get_if_initialized() {
        open_path(app, state.directories.launcher_logs_dir()).await;
    }
}

#[tauri::command]
pub async fn show_app_db_backups_folder<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<()> {
    let path = app_db_backup_dir()?;
    tokio::fs::create_dir_all(&path).await?;
    open_path(app, path).await;
    Ok(())
}

#[tauri::command]
pub async fn export_debug_info<R: Runtime>(
    window: tauri::Window<R>,
    state: tauri::State<'_, DebugInfoExports>,
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
    let result =
        engine::debug_info::export_debug_info(path, cancel, move |progress| {
            let _ = on_progress.send(progress);
        })
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

/// Amberite's own deep links, next to the Modrinth ones the engine parses. The frontend handles
/// both in `handleCommand`, told apart by `event`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event")]
pub enum AmberiteCommand {
    /// `amberite://approve/<code>`: open "Add Core" with the code a Core is showing filled in.
    ApproveCore { code: String },
}

/// What the App was opened with: a Modrinth link or file, or an Amberite link.
#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum OpeningCommand {
    Modrinth(CommandPayload),
    Amberite(AmberiteCommand),
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

async fn parse_opening_command(command: &str) -> Result<OpeningCommand> {
    if let Some(command) = amberite_command(command) {
        return Ok(OpeningCommand::Amberite(command));
    }
    Ok(OpeningCommand::Modrinth(
        handler::parse_command(command).await?,
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
) -> Result<Option<OpeningCommand>> {
    if crate::dev::config().is_some() {
        return Ok(None);
    }
    let payload = state.payload.lock().await;
    let cmd_arg = std::env::args_os()
        .nth(1)
        .map(|path| path.to_string_lossy().to_string());

    return if let Some(payload) = payload.as_ref() {
        opening_command(payload).await
    } else if let Some(cmd_arg) = cmd_arg {
        opening_command(&cmd_arg).await
    } else {
        Ok(None)
    };
}

#[tauri::command]
#[cfg(not(target_os = "macos"))]
pub async fn get_opening_command() -> Result<Option<OpeningCommand>> {
    if crate::dev::config().is_some() {
        return Ok(None);
    }
    // Tauri is not CLI, we use arguments as path to file to call
    let cmd_arg = std::env::args_os().nth(1);

    let cmd_arg = cmd_arg.map(|path| path.to_string_lossy().to_string());
    if let Some(cmd) = cmd_arg {
        return opening_command(&cmd).await;
    }
    Ok(None)
}

// helper function called when redirected by a weblink (ie: modrith://do-something) or when redirected by a .mrpack file (in which case its a filepath)
// We hijack the deep link library (which also contains functionality for instance-checking)
pub async fn handle_command<R: Runtime>(
    app: tauri::AppHandle<R>,
    command: String,
) -> Result<()> {
    let command = normalize_deep_link_command(&command);
    if accept_auth_deeplink(&command).await {
        return Ok(());
    }

    tracing::info!("handle command: {command}");
    // Amberite's own links go straight to the frontend; the engine only knows the Modrinth ones.
    if let Some(command) = amberite_command(&command) {
        use tauri::Emitter;
        app.emit("amberite-command", command)?;
        if let Some(window) = app.get_window("main") {
            let _ = window.set_focus();
        }
        return Ok(());
    }
    Ok(engine::handler::parse_and_emit_command(&command).await?)
}

async fn opening_command(command: &str) -> Result<Option<OpeningCommand>> {
    let command = normalize_deep_link_command(command);
    if accept_auth_deeplink(&command).await {
        return Ok(None);
    }

    tracing::info!("opening command {command}");
    Ok(Some(parse_opening_command(&command).await?))
}

async fn accept_auth_deeplink(command: &str) -> bool {
    let Some((code, nonce)) = parse_auth_deeplink(command) else {
        return false;
    };

    tracing::info!("Handling Modrinth auth deep link");
    if !submit_deeplink(code, &nonce) {
        const MESSAGE: &str = "Couldn't finish signing in. Please try again.";
        if let Err(error) = emit_warning(MESSAGE).await {
            tracing::error!("{MESSAGE} ({error})");
        }
    }
    true
}

// Remove when (and if) https://github.com/tauri-apps/tauri/issues/12022 is implemented
pub(crate) fn tauri_convert_file_src(path: &Path) -> Result<Url> {
    #[cfg(any(windows, target_os = "android"))]
    const BASE: &str = "http://asset.localhost/";
    #[cfg(not(any(windows, target_os = "android")))]
    const BASE: &str = "asset://localhost/";

    let path = canonicalize(path)?;
    let path = path.to_string_lossy();
    let encoded = urlencoding::encode(&path);

    Ok(Url::parse(&format!("{BASE}{encoded}"))?)
}
