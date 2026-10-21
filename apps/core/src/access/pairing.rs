//! Pairing: how a Core comes to belong to an account and gets its own backend key (`amc_...`).
//!
//! An unpaired Core asks the backend for a code and the key it will use, prints the code, and
//! polls. The owner enters the code in the App, which approves it with their account; from then
//! on the key works. The App never talks to the Core during pairing. The code being shown is kept
//! in the `pairing` table, so every copal process on one data folder shows the same code, and
//! `copal status` reads it there. `copal pair` drops it and wakes the loop for a fresh one.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use sqlx::SqlitePool;
use tokio::sync::Notify;
use tracing::warn;

/// Wait between attempts to reach the backend.
const RETRY: Duration = Duration::from_secs(10);

/// What pairing needs to know about this Core.
pub struct Pairing<'a> {
    pub db: &'a SqlitePool,
    pub http: &'a reqwest::Client,
    pub backend_url: &'a str,
    pub core_id: &'a str,
    /// Where the App reaches this Core, and the certificate it pins there.
    pub address: &'a str,
    pub cert_fingerprint: &'a str,
    /// Woken when the owner asks for a fresh code.
    pub new_code: &'a Notify,
}

/// The code an unpaired Core shows, as the App takes it.
pub struct Shown {
    /// Like `MZUX-D576`.
    pub code: String,
    /// Unix seconds.
    pub expires_at: i64,
}

/// What the owner does with `code`, shown until `expires_at`.
pub fn instructions(code: &str, expires_at: i64) -> String {
    format!(
        "Open Amberite, go to Settings > Cores > Add Core, and enter {code}.\nOr open amberite://approve/{code} on a computer with Amberite.\nThe code works for {} minutes.",
        ((expires_at - now()).max(0) + 59) / 60
    )
}

/// The code this Core shows now, `None` while it has none.
pub async fn shown(db: &SqlitePool) -> sqlx::Result<Option<Shown>> {
    let row: Option<(String, i64)> = sqlx::query_as(
        "SELECT code, expires_at FROM pairing WHERE id = 1 AND expires_at > ?",
    )
    .bind(now())
    .fetch_optional(db)
    .await?;
    Ok(row.map(|(code, expires_at)| Shown {
        code: display_code(&code),
        expires_at,
    }))
}

/// Drop the code being shown, so the pairing loop asks for a new one once woken.
pub async fn drop_code(db: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM pairing WHERE id = 1")
        .execute(db)
        .await?;
    Ok(())
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

/// Show pairing codes until one is approved, then store the owner and key.
pub async fn pair(state: &Pairing<'_>) {
    loop {
        // Another copal process on this data folder may have finished pairing.
        if matches!(paired(state.db).await, Ok(true)) {
            return;
        }
        let pending = match current_code(state).await {
            Some(pending) => pending,
            None => match request_code(state).await {
                Ok(pending) => pending,
                Err(problem) => {
                    println!(
                        "\nThis Core cannot sign in yet: {problem}\nRetrying in {} seconds.\n",
                        RETRY.as_secs()
                    );
                    // Also in Core's log, where `copal status` sends the owner.
                    warn!("no pairing code yet: {problem}");
                    tokio::time::sleep(RETRY).await;
                    continue;
                }
            },
        };
        println!(
            "\n{}\n",
            instructions(&display_code(&pending.code), pending.expires_at)
        );

        let interval =
            Duration::from_secs(pending.poll_interval.clamp(1, 60) as u64);
        let mut last_problem = String::new();
        loop {
            tokio::select! {
                () = tokio::time::sleep(interval) => {}
                () = state.new_code.notified() => {}
            }
            // `copal pair` dropped this code, or another copal process replaced it.
            if pending.expires_at > now()
                && current_code(state)
                    .await
                    .is_none_or(|current| current.code != pending.code)
            {
                break;
            }
            match poll(state, &pending.key).await {
                Poll::Approved(account) => {
                    if let Err(error) =
                        set_owner(state.db, &account, &pending.key).await
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
                        println!(
                            "\nWaiting for approval, but {problem}. Retrying.\n"
                        );
                        last_problem = problem;
                    }
                }
            }
        }
    }
}

/// Whether this Core has an owner and a backend key.
pub async fn paired(db: &SqlitePool) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT owner_user_id IS NOT NULL AND backend_key IS NOT NULL FROM core WHERE id = 1",
    )
    .fetch_optional(db)
    .await
    .map(|paired| paired.unwrap_or(false))
}

/// Record the owner and the key, and the owner's name until their first ticket names them.
async fn set_owner(
    db: &SqlitePool,
    owner: &Approved,
    key: &str,
) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query(
        "UPDATE core SET owner_user_id = ?, backend_key = ? WHERE id = 1",
    )
    .bind(&owner.id)
    .bind(key)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO users (id, name, updated_at) VALUES (?, ?, unixepoch())          ON CONFLICT (id) DO NOTHING",
    )
    .bind(&owner.id)
    .bind(&owner.username)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// The code this Core is already showing, if it has not expired.
async fn current_code(state: &Pairing<'_>) -> Option<Pending> {
    let pending: Option<Pending> = sqlx::query_as(
        "SELECT code, key, expires_at, poll_interval FROM pairing WHERE id = 1",
    )
    .fetch_optional(state.db)
    .await
    .ok()
    .flatten();
    pending.filter(|pending| pending.expires_at > now())
}

/// Forget the code being shown, unless another process has already replaced it.
async fn forget_code(state: &Pairing<'_>, code: &str) {
    let _ = sqlx::query("DELETE FROM pairing WHERE code = ?")
        .bind(code)
        .execute(state.db)
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
async fn request_code(state: &Pairing<'_>) -> Result<Pending, String> {
    let response = state
        .http
        .post(format!("{}/core/auth/code", state.backend_url))
        .json(&json!({
            "core_id": state.core_id,
            "name": core_name(),
            "address": state.address,
            "cert_fingerprint": state.cert_fingerprint,
        }))
        .send()
        .await
        .map_err(|error| {
            format!(
                "Amberite could not be reached at {} ({error})",
                state.backend_url
            )
        })?;
    let status = response.status();
    if !status.is_success() {
        let body: BackendError = response.json().await.unwrap_or_default();
        return Err(match body.error.as_str() {
            "already_signed_in" => "this Core is still listed under an account. Remove it in the App under Settings, Cores.".into(),
            "invalid_address" => format!(
                "this Core's address {} is not allowed. {}",
                state.address, body.description
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
        "INSERT INTO pairing (id, code, key, expires_at, poll_interval) VALUES (1, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET code = excluded.code, key = excluded.key, expires_at = excluded.expires_at, poll_interval = excluded.poll_interval",
    )
    .bind(&pending.code)
    .bind(&pending.key)
    .bind(pending.expires_at)
    .bind(pending.poll_interval)
    .execute(state.db)
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

async fn poll(state: &Pairing<'_>, key: &str) -> Poll {
    let response = state
        .http
        .post(format!("{}/core/auth/poll", state.backend_url))
        .bearer_auth(key)
        .send()
        .await;
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            return Poll::Unavailable(format!(
                "Amberite could not be reached ({error})"
            ));
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

#[derive(Default, Deserialize)]
struct BackendError {
    #[serde(default)]
    error: String,
    #[serde(default)]
    description: String,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// Name shown for this Core in the App: the machine's name.
fn core_name() -> String {
    std::env::var("COMPUTERNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|name| name.trim().chars().take(64).collect::<String>())
        .filter(|name| !name.is_empty())
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
