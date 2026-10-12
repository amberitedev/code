//! Sign-in: how a Core comes to belong to an account.
//!
//! A signed-out Core asks the backend for a code and the key it will use, prints the code, and
//! polls. The owner enters the code in the App, which approves it with their account; from then
//! on the key works and the Core is listed under that account. The App never talks to the Core
//! during sign-in, and the account session never leaves the App. A signed-in Core reports its
//! address with the key on every start and every few minutes after that, and signs itself out
//! when the key is refused: that is how a Core removed in the App finds out.
//!
//! `copal auth` runs the same sign-in from another process. The code being shown is kept in the
//! database, so a running Core and `copal auth` show one code instead of replacing each other's.

use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use tracing::{info, warn};

use crate::application::state::AppState;

/// Wait between attempts to reach the backend.
const RETRY: Duration = Duration::from_secs(10);
/// How often a signed-in Core reports itself, which is also how soon it notices being removed.
const CHECK_EVERY: Duration = Duration::from_secs(5 * 60);

/// Keeps this Core either signed in and known to the backend, or showing a sign-in code. Runs for
/// Core's lifetime.
pub async fn run(state: Arc<AppState>) {
    let mut outside = state.network.api_address();
    loop {
        let owner = match state.owner().await {
            Ok(owner) => owner,
            Err(error) => {
                warn!(%error, "could not read the Core owner");
                tokio::time::sleep(RETRY).await;
                continue;
            }
        };
        let Some(owner) = owner else {
            sign_in(&state, false).await;
            continue;
        };
        match report_address(&state, &owner.backend_key).await {
            // Report again after a while, when the address on the internet changes, or when
            // another call to the backend saw the key refused.
            Report::Accepted => tokio::select! {
                () = state.auth_changed.notified() => {}
                _ = outside.changed() => {}
                () = tokio::time::sleep(CHECK_EVERY) => {}
            },
            Report::Changed => {}
            Report::Rejected => {
                println!(
                    "\nThis Core was removed from its account. Its servers and files are untouched.\n"
                );
                if let Err(error) = state.forget_owner().await {
                    warn!(%error, "could not forget the Core owner");
                    tokio::time::sleep(RETRY).await;
                }
            }
        }
    }
}

enum Report {
    Accepted,
    /// The backend refused the key: this Core was removed from its account.
    Rejected,
    /// Sign-in changed while waiting to retry.
    Changed,
}

/// Tell the backend where this Core is. Retries until the backend answers.
async fn report_address(state: &AppState, key: &str) -> Report {
    loop {
        let response = state
            .http
            .post(format!("{}/core/address", state.config.account_api_url))
            .bearer_auth(key)
            .json(&json!({
                "address": state.config.public_url,
                "cert_fingerprint": state.tls.fingerprint,
                "version": env!("CARGO_PKG_VERSION"),
                // Where the App finds this Core from outside the home, once that is proven.
                "public_address": state
                    .network
                    .api_address()
                    .borrow()
                    .map(|address| format!("https://{address}")),
            }))
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                info!(address = %state.config.public_url, "reported Core address");
                return Report::Accepted;
            }
            Ok(response) if response.status() == StatusCode::UNAUTHORIZED => {
                return Report::Rejected;
            }
            Ok(response) => {
                let reason = error_text(response).await;
                println!(
                    "\nAmberite did not accept this Core's address ({}): {reason}\nThe App may not find this Core until this is fixed. Retrying.\n",
                    state.config.public_url
                );
            }
            Err(error) => {
                warn!(%error, "could not reach Amberite to report this Core's address, retrying");
            }
        }
        tokio::select! {
            _ = tokio::time::sleep(RETRY * 3) => {}
            _ = state.auth_changed.notified() => return Report::Changed,
        }
    }
}

/// A sign-in waiting for approval: the code being shown and the key that works once it is
/// approved. Shared through the database with any other copal process on this data directory.
#[derive(sqlx::FromRow)]
struct Pending {
    code: String,
    key: String,
    /// Unix seconds.
    expires_at: i64,
    poll_interval: i64,
}

/// Another call to the backend saw this Core's key refused. Wakes `run` so it signs out now
/// instead of at its next check.
pub fn key_refused(state: &AppState) {
    state.auth_changed.notify_one();
}

/// Show sign-in codes until one is approved, then store the owner and key. Returns once signed
/// in, or when the sign-in task is woken by `auth_changed`. With `interactive`, a code is also
/// opened in the App on this computer when it is installed, like `claude auth` does.
pub async fn sign_in(state: &AppState, interactive: bool) {
    loop {
        let pending = match current_code(state).await {
            Some(pending) => pending,
            None => match request_code(state).await {
                Ok(pending) => pending,
                Err(problem) => {
                    println!(
                        "\nThis Core cannot sign in yet: {problem}\nRetrying in {} seconds.\n",
                        RETRY.as_secs()
                    );
                    tokio::select! {
                        () = tokio::time::sleep(RETRY) => continue,
                        () = state.auth_changed.notified() => return,
                    }
                }
            },
        };
        let shown = display_code(&pending.code);
        println!(
            "\nOpen Amberite, go to Settings > Cores > Add Core, and enter {shown}.\nOr open amberite://approve/{shown} on a computer with Amberite.\nThe code works for {} minutes.\n",
            ((pending.expires_at - now()).max(0) + 59) / 60
        );
        if interactive {
            open_in_app(&shown);
        }

        let interval =
            Duration::from_secs(pending.poll_interval.clamp(1, 60) as u64);
        let mut last_problem = String::new();
        loop {
            tokio::select! {
                () = tokio::time::sleep(interval) => {}
                () = state.auth_changed.notified() => return,
            }
            match poll(state, &pending.key).await {
                Poll::Approved(account) => {
                    if let Err(error) =
                        state.set_owner(&account.id, &pending.key).await
                    {
                        warn!(%error, "could not store the Core owner");
                        continue;
                    }
                    forget_code(state, &pending.code).await;
                    println!("\nSigned in as {}.\n", account.username);
                    return;
                }
                Poll::Pending if pending.expires_at > now() => {}
                Poll::Pending | Poll::Gone => {
                    forget_code(state, &pending.code).await;
                    println!("\nThe code expired. Here is a new one.");
                    break;
                }
                Poll::Unavailable(problem) => {
                    if problem != last_problem {
                        println!("\nWaiting for approval, but {problem}. Retrying.\n");
                        last_problem = problem;
                    }
                }
            }
        }
    }
}

/// Sign this Core out: its key stops working on the backend and the owner is forgotten here.
/// Servers and files stay. The backend call is best effort; a Core still listed there says so
/// when it next asks for a code.
pub async fn sign_out(state: &AppState) -> sqlx::Result<()> {
    if let Some(owner) = state.owner().await? {
        let revoked = state
            .http
            .delete(format!("{}/core/auth", state.config.account_api_url))
            .bearer_auth(&owner.backend_key)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status);
        if let Err(error) = revoked {
            println!(
                "Amberite could not be told ({error}). If this Core stays listed in the App, remove it under Settings, Cores."
            );
        }
    }
    state.forget_owner().await?;
    sqlx::query("DELETE FROM core_auth_pending")
        .execute(&state.pool)
        .await?;
    state.auth_changed.notify_one();
    Ok(())
}

/// The code this Core is already showing, if it has not expired.
async fn current_code(state: &AppState) -> Option<Pending> {
    let pending: Option<Pending> = sqlx::query_as(
        "SELECT code, key, expires_at, poll_interval FROM core_auth_pending WHERE id = 1",
    )
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    pending.filter(|pending| pending.expires_at > now())
}

/// Forget the code being shown, unless another process has already replaced it.
async fn forget_code(state: &AppState, code: &str) {
    let _ = sqlx::query("DELETE FROM core_auth_pending WHERE code = ?")
        .bind(code)
        .execute(&state.pool)
        .await;
}

#[derive(Deserialize)]
struct CodeResponse {
    code: String,
    key: String,
    expires_in: i64,
    poll_interval: i64,
}

/// Ask the backend for a new code and key, and remember them.
async fn request_code(state: &AppState) -> Result<Pending, String> {
    let response = state
        .http
        .post(format!("{}/core/auth/code", state.config.account_api_url))
        .json(&json!({
            "core_id": state.core_id,
            "name": core_name(),
            "address": state.config.public_url,
            "cert_fingerprint": state.tls.fingerprint,
        }))
        .send()
        .await
        .map_err(|error| {
            format!(
                "Amberite could not be reached at {} ({error})",
                state.config.account_api_url
            )
        })?;
    let status = response.status();
    if !status.is_success() {
        let body: BackendError = response.json().await.unwrap_or_default();
        return Err(match body.error.as_str() {
            "already_signed_in" => "this Core is still listed under an account. Remove it in the App under Settings, Cores.".into(),
            "invalid_address" => format!(
                "this Core's address {} is not allowed. {}",
                state.config.public_url, body.description
            ),
            "too_many_requests" => "this Core asked for too many codes in a short time".into(),
            _ => format!("Amberite answered {status}. {}", body.description),
        });
    }
    let issued: CodeResponse = response.json().await.map_err(|error| {
        format!("Amberite gave an unexpected answer ({error})")
    })?;
    let pending = Pending {
        code: issued.code,
        key: issued.key,
        expires_at: now() + issued.expires_in,
        poll_interval: issued.poll_interval,
    };
    sqlx::query(
        "INSERT INTO core_auth_pending (id, code, key, expires_at, poll_interval) VALUES (1, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET code = excluded.code, key = excluded.key, expires_at = excluded.expires_at, poll_interval = excluded.poll_interval",
    )
    .bind(&pending.code)
    .bind(&pending.key)
    .bind(pending.expires_at)
    .bind(pending.poll_interval)
    .execute(&state.pool)
    .await
    .map_err(|error| format!("the code could not be stored ({error})"))?;
    Ok(pending)
}

/// The account that approved the code.
#[derive(Deserialize)]
pub struct Approved {
    pub id: String,
    pub username: String,
}

enum Poll {
    Pending,
    Approved(Approved),
    /// The backend no longer knows the code: it expired or was replaced.
    Gone,
    Unavailable(String),
}

#[derive(Deserialize)]
struct PollResponse {
    status: String,
    account: Option<Approved>,
}

async fn poll(state: &AppState, key: &str) -> Poll {
    let response = state
        .http
        .post(format!("{}/core/auth/poll", state.config.account_api_url))
        .bearer_auth(key)
        .send()
        .await;
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            return Poll::Unavailable(format!(
                "Amberite could not be reached ({error})"
            ))
        }
    };
    match response.status() {
        StatusCode::GONE => Poll::Gone,
        status if !status.is_success() => {
            Poll::Unavailable(format!("Amberite answered {status}"))
        }
        _ => match response.json::<PollResponse>().await {
            Ok(PollResponse {
                account: Some(account),
                ..
            }) => Poll::Approved(account),
            Ok(PollResponse { status, .. }) if status == "pending" => {
                Poll::Pending
            }
            Ok(_) => {
                Poll::Unavailable("Amberite gave an unexpected answer".into())
            }
            Err(error) => Poll::Unavailable(format!(
                "Amberite gave an unexpected answer ({error})"
            )),
        },
    }
}

/// `abcdefgh` as `ABCD-EFGH`, how the code is shown and typed.
fn display_code(code: &str) -> String {
    let upper = code.to_ascii_uppercase();
    match upper.len() {
        8 => format!("{}-{}", &upper[..4], &upper[4..]),
        _ => upper,
    }
}

/// Open the code in the App on this computer, when there is one to open it. Only from a terminal:
/// a service has no desktop, and the printed instructions cover SSH.
fn open_in_app(code: &str) {
    if !std::io::IsTerminal::is_terminal(&std::io::stdout()) || !app_installed()
    {
        return;
    }
    let _ = open::that_detached(format!("amberite://approve/{code}"));
}

/// Whether something on this computer opens `amberite://` links, so no "find an app" dialog
/// appears where the App is not installed.
fn app_installed() -> bool {
    #[cfg(windows)]
    {
        return std::process::Command::new("reg")
            .args(["query", r"HKCR\amberite"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
    }
    #[cfg(target_os = "macos")]
    {
        return true;
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        return std::env::var_os("DISPLAY").is_some()
            || std::env::var_os("WAYLAND_DISPLAY").is_some();
    }
}

#[derive(Default, Deserialize)]
struct BackendError {
    #[serde(default)]
    error: String,
    #[serde(default)]
    description: String,
}

async fn error_text(response: reqwest::Response) -> String {
    let status = response.status();
    let body: BackendError = response.json().await.unwrap_or_default();
    if body.description.is_empty() {
        format!("Amberite answered {status}")
    } else {
        body.description
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// Name shown for this Core in the App: the machine's name.
fn core_name() -> String {
    sysinfo::System::host_name()
        .filter(|name| !name.trim().is_empty())
        .map(|name| name.chars().take(64).collect())
        .unwrap_or_else(|| "Core".into())
}

#[cfg(test)]
mod tests {
    use super::display_code;

    #[test]
    fn codes_are_shown_in_two_groups() {
        assert_eq!(display_code("k7pm2xrq"), "K7PM-2XRQ");
    }
}
