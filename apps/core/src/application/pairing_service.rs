use std::{
    io::{self, Write},
    path::Path,
    process::{Command, Stdio},
    sync::Arc,
    time::Instant,
};

use color_eyre::eyre::Result;
use serde_json::json;
use sqlx::SqlitePool;
use tracing::{info, warn};

use crate::application::state::{
    AppState, PAIRING_WINDOW, format_pairing_code, generate_pairing_code,
    generate_setup_secret, write_local_setup_secret,
};

/// Register this unpaired Core in Convex so a remote dashboard/app can claim its code.
pub async fn register_pairing_core(state: Arc<AppState>) -> bool {
    let code = state.pairing_code.lock().await.clone();
    let Some(code) = code else {
        return false;
    };
    let core_id = state.core_id.clone();
    let public_url = state.config.public_url.clone();
    let bind_host = state.config.bind_host.clone();
    let port = state.config.port;

    let endpoint = format!(
        "{}/api/mutation",
        state.config.convex_url.trim_end_matches('/')
    );
    let mut args = json!({
        "code": code,
        "coreId": core_id,
        "metadata": {
            "bindHost": bind_host,
            "port": port,
        },
        "ttlMs": PAIRING_WINDOW.as_millis() as u64,
    });
    args["connectionUrl"] = json!(public_url);
    let body = json!({
        "path": "corePairing:registerPairingCore",
        "format": "json",
        "args": args,
    });

    match state.http.post(endpoint).json(&body).send().await {
        Ok(response) if response.status().is_success() => {
            let body: serde_json::Value =
                response.json().await.unwrap_or_else(|_| json!({}));
            if body.get("status").and_then(|value| value.as_str())
                == Some("success")
            {
                info!(%core_id, "registered Core pairing code with Convex");
                let formatted_code = format_pairing_code(&code);
                println!("\nCopal pairing code: {formatted_code}");
                match copy_pairing_code_to_clipboard(&formatted_code) {
                    Ok(()) => println!("Pairing code copied to clipboard."),
                    Err(error) => {
                        warn!(%error, "failed to copy Core pairing code to clipboard");
                    }
                }
                println!(
                    "This code expires in 15 minutes. Restart Core to generate a new code.\n"
                );
                return true;
            } else {
                warn!(response = %body, "Convex rejected Core pairing registration");
            }
        }
        Ok(response) => {
            warn!(status = %response.status(), "failed to register Core pairing code with Convex");
        }
        Err(error) => {
            warn!(%error, "failed to reach Convex for Core pairing registration");
        }
    }
    false
}

fn copy_pairing_code_to_clipboard(code: &str) -> io::Result<()> {
    #[cfg(windows)]
    {
        return write_to_clipboard_command("clip", &[], code);
    }

    #[cfg(target_os = "macos")]
    {
        return write_to_clipboard_command("pbcopy", &[], code);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let attempts: [(&str, &[&str]); 3] = [
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ];
        let mut last_error = None;
        for (program, args) in attempts {
            match write_to_clipboard_command(program, args, code) {
                Ok(()) => return Ok(()),
                Err(error) => last_error = Some(error),
            }
        }
        return Err(last_error.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "no clipboard command found",
            )
        }));
    }

    #[allow(unreachable_code)]
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "clipboard is not supported on this platform",
    ))
}

fn write_to_clipboard_command(
    program: &str,
    args: &[&str],
    text: &str,
) -> io::Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::BrokenPipe,
            "clipboard command stdin is unavailable",
        )
    })?;
    stdin.write_all(text.as_bytes())?;
    drop(stdin);

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("clipboard command `{program}` exited with {status}"),
        ))
    }
}

pub async fn reset_running_pairing(state: Arc<AppState>) -> Result<bool> {
    clear_pairing_storage(&state.pool, &state.config.data_dir).await?;

    let code = generate_pairing_code();
    let secret = generate_setup_secret();
    write_local_setup_secret(&state.config.data_dir, &secret).await?;

    state
        .wrong_pairing_attempts
        .store(0, std::sync::atomic::Ordering::Relaxed);
    let expires_at = Instant::now() + PAIRING_WINDOW;
    *state.pairing_code.lock().await = Some(code);
    *state.pairing_code_expires_at.lock().await = Some(expires_at);
    *state.local_setup_secret.lock().await = Some(secret);

    tokio::spawn(expire_pairing_window(Arc::clone(&state), expires_at));
    Ok(register_pairing_core(state).await)
}

pub async fn clear_pairing_storage(
    pool: &SqlitePool,
    data_dir: &Path,
) -> Result<()> {
    for statement in [
        "DELETE FROM core_config WHERE id = 1",
        "DELETE FROM core_members",
        "DELETE FROM instance_members",
        "DELETE FROM core_group_bans",
        "DELETE FROM core_invitations",
        "DELETE FROM activity_log",
    ] {
        sqlx::query(statement).execute(pool).await?;
    }
    tokio::fs::remove_file(data_dir.join(".setup_secret"))
        .await
        .ok();
    Ok(())
}

pub async fn expire_pairing_window(
    state: Arc<AppState>,
    expected_expires_at: Instant,
) {
    tokio::time::sleep_until(tokio::time::Instant::from_std(
        expected_expires_at,
    ))
    .await;

    let mut pairing_code = state.pairing_code.lock().await;
    let mut local_setup_secret = state.local_setup_secret.lock().await;
    let mut pairing_code_expires_at =
        state.pairing_code_expires_at.lock().await;
    if *pairing_code_expires_at != Some(expected_expires_at) {
        return;
    }
    if pairing_code.is_some() {
        *pairing_code = None;
        *local_setup_secret = None;
        *pairing_code_expires_at = None;
        tokio::fs::remove_file(state.config.data_dir.join(".setup_secret"))
            .await
            .ok();
        println!(
            "\nCopal pairing code expired. Use `clear` to generate a new code.\n"
        );
    }
}
