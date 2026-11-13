use engine::emit_warning;
use tauri::{Manager, Runtime};
use tokio::sync::broadcast;

use crate::accounts::oauth_utils::auth_code_reply::{
    normalize_deep_link_command, parse_auth_deeplink, submit_deeplink,
};
use crate::api::Result;
use crate::server::events::{AmberiteCommand, AppEvent, Command};

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
) -> engine::Result<Command> {
    if let Some(command) = amberite_command(command) {
        return Ok(Command::Amberite(command));
    }
    Ok(Command::Modrinth(
        crate::desktop::deep_links::parse_command(events, command).await?,
    ))
}

// Get opening command
// For example, if a user clicks on an .mrpack to open the app.
// This should be called once and only when the app is done booting up and ready to receive a command
// Returns a Command struct- see events.js
#[cfg(target_os = "macos")]
pub async fn get_opening_command(
    app: &tauri::AppHandle,
) -> engine::Result<Option<Command>> {
    if crate::dev::config().is_some() {
        return Ok(None);
    }
    let state = crate::macos::deep_link::get_or_init_payload(app);
    let events = app.state::<engine::EventSink>();
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

#[cfg(not(target_os = "macos"))]
pub async fn get_opening_command(
    app: &tauri::AppHandle,
) -> engine::Result<Option<Command>> {
    if crate::dev::config().is_some() {
        return Ok(None);
    }
    let events = app.state::<engine::EventSink>();
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
    Ok(
        crate::desktop::deep_links::parse_and_emit_command(&events, &command)
            .await?,
    )
}

async fn opening_command(
    events: &engine::EventSink,
    command: &str,
) -> engine::Result<Option<Command>> {
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
