use crate::api::Result;
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use tauri::plugin::TauriPlugin;
use tauri::{Manager, Runtime, UserAttentionType};
use theseus::prelude::*;
use uuid::Uuid;

const AMBERITE_KEYRING_SERVICE: &str = "dev.amberite.app";
const LEGACY_SESSION_JWT_ACCOUNT: &str = "amberite-session-jwt";
const LEGACY_REFRESH_TOKEN_ACCOUNT: &str = "amberite-session-refresh-token";
const AMBERITE_LOCAL_CORE_DATA_DIR_ENV: &str = "AMBERITE_LOCAL_CORE_DATA_DIR";

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    let account = match crate::dev::config() {
        Some(config) => {
            format!("amberite-product-session:dev:{}", config.app_id)
        }
        None => "amberite-product-session".to_string(),
    };
    minecraft_auth::configure_product_session_account(account);

    tauri::plugin::Builder::<R>::new("auth")
        .invoke_handler(tauri::generate_handler![
            check_reachable,
            login,
            remove_user,
            get_default_user,
            set_default_user,
            get_users,
            amberite_product_sign_in,
            restore_amberite_product_session,
            refresh_amberite_product_session,
            sign_out_amberite_product_session,
            get_remembered_amberite_identity,
            get_amberite_local_setup_secret,
        ])
        .build()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeAmberiteSessionSummary {
    access_token: String,
    user: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvexTokens {
    token: String,
    refresh_token: String,
}

#[derive(Deserialize)]
struct ConvexSignInValue {
    tokens: Option<ConvexTokens>,
}

#[derive(Deserialize)]
struct ConvexEnvelope<T> {
    status: String,
    value: Option<T>,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
}

fn other_error(
    error: impl std::fmt::Display,
) -> crate::api::TheseusSerializableError {
    theseus::ErrorKind::OtherError(error.to_string())
        .as_error()
        .into()
}

fn legacy_account(base: &str) -> String {
    match crate::dev::config() {
        Some(config) => format!("{base}:dev:{}", config.app_id),
        None => base.to_string(),
    }
}

#[tauri::command]
pub async fn amberite_product_sign_in<R: Runtime>(
    app: tauri::AppHandle<R>,
    convex_url: String,
    mode: String,
    expected_minecraft_uuid: Option<String>,
) -> Result<NativeAmberiteSessionSummary> {
    let expected = expected_minecraft_uuid
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(other_error)?;
    let staged =
        match run_minecraft_login(&app, true, mode == "use_another_account")
            .await?
            .ok_or_else(|| other_error("Minecraft sign-in was cancelled"))?
        {
            MinecraftLoginResult::Staged(staged) => staged,
            MinecraftLoginResult::Persisted(_) => {
                return Err(other_error("Minecraft sign-in was not staged"));
            }
        };
    if expected
        .is_some_and(|uuid| uuid != staged.credentials.offline_profile.id)
    {
        return Err(other_error("Minecraft UUID mismatch"));
    }

    let (tokens, user) = convex_minecraft_sign_in(
        &convex_url,
        &staged.credentials.access_token,
        expected.map(|value| value.as_hyphenated().to_string()),
    )
    .await?;
    let identity_uuid = verified_user_uuid(&user)?;
    if identity_uuid != staged.credentials.offline_profile.id {
        return Err(other_error("Amberite identity UUID mismatch"));
    }

    let session = minecraft_auth::AmberiteNativeSession {
        access_token: tokens.token.clone(),
        refresh_token: tokens.refresh_token,
        user: user.clone(),
        active_identity_uuid: identity_uuid,
        expires_at: jwt_expiry(&tokens.token)?,
        updated_at: Utc::now(),
    };
    let remembered = remembered_identity(&user, identity_uuid)?;
    minecraft_auth::commit_amberite_product_session(
        staged, expected, session, remembered,
    )
    .await
    .map_err(other_error)?;
    clear_legacy_session_entries();
    Ok(NativeAmberiteSessionSummary {
        access_token: tokens.token,
        user,
    })
}

#[tauri::command]
pub async fn restore_amberite_product_session(
    convex_url: String,
) -> Result<Option<NativeAmberiteSessionSummary>> {
    if let Some(session) = minecraft_auth::amberite_product_session()
        .await
        .map_err(other_error)?
    {
        if session.expires_at <= Utc::now() + Duration::minutes(5) {
            return refresh_amberite_product_session(convex_url).await;
        }
        match convex_current_user(&convex_url, &session.access_token).await {
            Ok(user) => return Ok(Some(summary(session.access_token, user))),
            Err(error) if is_unauthorized(&error) => {
                return refresh_amberite_product_session(convex_url).await;
            }
            Err(error) => return Err(error),
        }
    }
    migrate_legacy_amberite_session(&convex_url).await
}

#[tauri::command]
pub async fn refresh_amberite_product_session(
    convex_url: String,
) -> Result<Option<NativeAmberiteSessionSummary>> {
    let Some(previous) = minecraft_auth::amberite_product_session()
        .await
        .map_err(other_error)?
    else {
        return Ok(None);
    };
    let (tokens, user) =
        match convex_refresh(&convex_url, &previous.refresh_token).await {
            Ok(value) => value,
            Err(error) => {
                if is_terminal_refresh_error(&error) {
                    minecraft_auth::clear_product_session_preserving_identity()
                        .await
                        .map_err(other_error)?;
                }
                return Err(error);
            }
        };
    let identity_uuid = verified_user_uuid(&user)?;
    if identity_uuid != previous.active_identity_uuid {
        minecraft_auth::clear_product_session_preserving_identity()
            .await
            .map_err(other_error)?;
        return Err(other_error("Amberite identity UUID mismatch"));
    }
    minecraft_auth::update_amberite_product_session(
        minecraft_auth::AmberiteNativeSession {
            access_token: tokens.token.clone(),
            refresh_token: tokens.refresh_token,
            user: user.clone(),
            active_identity_uuid: identity_uuid,
            expires_at: jwt_expiry(&tokens.token)?,
            updated_at: Utc::now(),
        },
    )
    .await
    .map_err(other_error)?;
    Ok(Some(summary(tokens.token, user)))
}

#[tauri::command]
pub async fn get_remembered_amberite_identity()
-> Result<Option<minecraft_auth::RememberedAmberiteIdentity>> {
    minecraft_auth::remembered_amberite_identity()
        .await
        .map_err(other_error)
}

#[tauri::command]
pub async fn sign_out_amberite_product_session(
    convex_url: String,
) -> Result<()> {
    if let Ok(Some(session)) = minecraft_auth::amberite_product_session().await
    {
        let _ = convex_call::<Value>(
            &convex_url,
            "action",
            "auth:signOut",
            json!({}),
            Some(&session.access_token),
        )
        .await;
    }
    minecraft_auth::clear_product_session_preserving_identity()
        .await
        .map_err(other_error)?;
    clear_legacy_session_entries();
    Ok(())
}

async fn migrate_legacy_amberite_session(
    convex_url: &str,
) -> Result<Option<NativeAmberiteSessionSummary>> {
    let refresh = read_legacy_keyring(LEGACY_REFRESH_TOKEN_ACCOUNT);
    let Some(refresh_token) = refresh else {
        clear_legacy_session_entries();
        return Ok(None);
    };
    let Some(credentials) = minecraft_auth::default_credential()
        .await
        .map_err(other_error)?
    else {
        clear_legacy_session_entries();
        return Ok(None);
    };
    let (tokens, user) = convex_refresh(convex_url, &refresh_token).await?;
    let identity_uuid = verified_user_uuid(&user)?;
    if identity_uuid != credentials.offline_profile.id {
        clear_legacy_session_entries();
        return Err(other_error("Legacy Amberite session UUID mismatch"));
    }
    minecraft_auth::attach_legacy_amberite_product_session(
        credentials,
        minecraft_auth::AmberiteNativeSession {
            access_token: tokens.token.clone(),
            refresh_token: tokens.refresh_token,
            user: user.clone(),
            active_identity_uuid: identity_uuid,
            expires_at: jwt_expiry(&tokens.token)?,
            updated_at: Utc::now(),
        },
        remembered_identity(&user, identity_uuid)?,
    )
    .await
    .map_err(other_error)?;
    clear_legacy_session_entries();
    Ok(Some(summary(tokens.token, user)))
}

async fn convex_minecraft_sign_in(
    convex_url: &str,
    minecraft_access_token: &str,
    expected_minecraft_uuid: Option<String>,
) -> Result<(ConvexTokens, Value)> {
    let mut params = json!({ "minecraftAccessToken": minecraft_access_token });
    if let Some(expected) = expected_minecraft_uuid {
        params["expectedMinecraftUuid"] = Value::String(expected);
    }
    let value: ConvexSignInValue = convex_call(
        convex_url,
        "action",
        "auth:signIn",
        json!({ "provider": "minecraft-token", "params": params }),
        None,
    )
    .await?;
    let tokens = value
        .tokens
        .ok_or_else(|| other_error("Amberite sign-in returned no session"))?;
    let user = convex_current_user(convex_url, &tokens.token).await?;
    Ok((tokens, user))
}

async fn convex_refresh(
    convex_url: &str,
    refresh_token: &str,
) -> Result<(ConvexTokens, Value)> {
    let value: ConvexSignInValue = convex_call(
        convex_url,
        "action",
        "auth:signIn",
        json!({ "refreshToken": refresh_token }),
        None,
    )
    .await?;
    let tokens = value
        .tokens
        .ok_or_else(|| other_error("Amberite refresh session is invalid"))?;
    let user = convex_current_user(convex_url, &tokens.token).await?;
    Ok((tokens, user))
}

async fn convex_current_user(
    convex_url: &str,
    access_token: &str,
) -> Result<Value> {
    convex_call(
        convex_url,
        "query",
        "profiles:current",
        json!({}),
        Some(access_token),
    )
    .await
}

async fn convex_call<T: for<'de> Deserialize<'de>>(
    convex_url: &str,
    kind: &str,
    path: &str,
    args: Value,
    access_token: Option<&str>,
) -> Result<T> {
    let endpoint = convex_endpoint(convex_url, kind)?;
    let client = reqwest::Client::new();
    let mut request = client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .json(&json!({ "path": path, "args": args, "format": "json" }));
    if let Some(token) = access_token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(other_error)?;
    let status = response.status();
    let envelope: ConvexEnvelope<T> =
        response.json().await.map_err(other_error)?;
    if !status.is_success() || envelope.status != "success" {
        return Err(other_error(format!(
            "Convex {kind} failed ({status}): {}",
            envelope
                .error_message
                .unwrap_or_else(|| "request rejected".to_string())
        )));
    }
    envelope
        .value
        .ok_or_else(|| other_error("Convex response did not contain a value"))
}

fn convex_endpoint(convex_url: &str, kind: &str) -> Result<String> {
    let base = url::Url::parse(convex_url).map_err(other_error)?;
    let local_http = base.scheme() == "http"
        && matches!(base.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if base.scheme() != "https" && !local_http {
        return Err(other_error("Convex URL must use HTTPS or localhost HTTP"));
    }
    Ok(format!("{}/api/{kind}", convex_url.trim_end_matches('/')))
}

fn jwt_expiry(token: &str) -> Result<DateTime<Utc>> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| other_error("Amberite access token is malformed"))?;
    let decoded = BASE64_URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(other_error)?;
    let value: Value = serde_json::from_slice(&decoded).map_err(other_error)?;
    let expiry = value
        .get("exp")
        .and_then(Value::as_i64)
        .ok_or_else(|| other_error("Amberite access token has no expiry"))?;
    Utc.timestamp_opt(expiry, 0)
        .single()
        .ok_or_else(|| other_error("Amberite access token expiry is invalid"))
}

fn verified_user_uuid(user: &Value) -> Result<Uuid> {
    user.get("minecraftUuid")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            other_error("Amberite profile has no verified Minecraft UUID")
        })
        .and_then(|value| Uuid::parse_str(value).map_err(other_error))
}

fn remembered_identity(
    user: &Value,
    minecraft_uuid: Uuid,
) -> Result<minecraft_auth::RememberedAmberiteIdentity> {
    let handle = user
        .get("verifiedMinecraftHandle")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            other_error("Amberite profile has no verified Minecraft handle")
        })?;
    let display_name =
        user.get("name").and_then(Value::as_str).unwrap_or(handle);
    Ok(minecraft_auth::RememberedAmberiteIdentity {
        minecraft_uuid,
        verified_minecraft_handle: handle.to_string(),
        display_name: display_name.to_string(),
        avatar_url: user
            .get("avatar_url")
            .and_then(Value::as_str)
            .map(str::to_string),
        last_successful_sign_in: Utc::now(),
    })
}

fn summary(access_token: String, user: Value) -> NativeAmberiteSessionSummary {
    NativeAmberiteSessionSummary { access_token, user }
}

fn is_unauthorized(error: &crate::api::TheseusSerializableError) -> bool {
    error
        .to_string()
        .to_lowercase()
        .contains("not authenticated")
        || error.to_string().contains("401")
}

fn is_terminal_refresh_error(
    error: &crate::api::TheseusSerializableError,
) -> bool {
    let message = error.to_string().to_lowercase();
    message.contains("401")
        || message.contains("403")
        || message.contains("not authenticated")
        || message.contains("invalid session")
        || message.contains("invalid refresh")
        || message.contains("expired")
        || message.contains("revoked")
        || (message.contains("refresh") && message.contains("reuse"))
}

fn read_legacy_keyring(base: &str) -> Option<String> {
    keyring::Entry::new(AMBERITE_KEYRING_SERVICE, &legacy_account(base))
        .ok()?
        .get_password()
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn clear_legacy_session_entries() {
    for account in [LEGACY_SESSION_JWT_ACCOUNT, LEGACY_REFRESH_TOKEN_ACCOUNT] {
        if let Ok(entry) = keyring::Entry::new(
            AMBERITE_KEYRING_SERVICE,
            &legacy_account(account),
        ) {
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(error) => tracing::warn!(
                    "Failed to clear legacy Amberite keyring entry: {error}"
                ),
            }
        }
    }
}

enum MinecraftLoginResult {
    Persisted(Credentials),
    Staged(minecraft_auth::StagedMinecraftLogin),
}

async fn run_minecraft_login<R: Runtime>(
    app: &tauri::AppHandle<R>,
    staged: bool,
    select_account: bool,
) -> Result<Option<MinecraftLoginResult>> {
    let flow = if staged {
        minecraft_auth::begin_login_staged_with_prompt(select_account).await?
    } else {
        minecraft_auth::begin_login_with_prompt(select_account).await?
    };
    let start = Utc::now();
    if let Some(window) = app.get_webview_window("signin") {
        window.close()?;
    }
    let window = tauri::WebviewWindowBuilder::new(
        app,
        "signin",
        tauri::WebviewUrl::External(flow.auth_request_uri.parse().map_err(
            |_| {
                theseus::ErrorKind::OtherError(
                    "Error parsing auth redirect URL".to_string(),
                )
                .as_error()
            },
        )?),
    )
    .title("Continue with Minecraft")
    .always_on_top(true)
    .center()
    .build()?;
    window.request_user_attention(Some(UserAttentionType::Critical))?;

    while (Utc::now() - start) < Duration::minutes(10) {
        if window.title().is_err() {
            return Ok(None);
        }
        if window
            .url()?
            .as_str()
            .starts_with("https://login.live.com/oauth20_desktop.srf")
            && let Some((_, code)) =
                window.url()?.query_pairs().find(|value| value.0 == "code")
        {
            window.close()?;
            let result = if staged {
                MinecraftLoginResult::Staged(
                    minecraft_auth::finish_login_staged(&code, flow).await?,
                )
            } else {
                MinecraftLoginResult::Persisted(
                    minecraft_auth::finish_login(&code, flow).await?,
                )
            };
            return Ok(Some(result));
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    window.close()?;
    Ok(None)
}

/// Checks if the authentication servers are reachable.
#[tauri::command]
pub async fn check_reachable() -> Result<()> {
    minecraft_auth::check_reachable().await?;
    Ok(())
}

#[tauri::command]
pub async fn login<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<minecraft_auth::MinecraftAccountSummary>> {
    match run_minecraft_login(&app, false, true).await? {
        Some(MinecraftLoginResult::Persisted(credentials)) => {
            Ok(Some(credentials.account_summary().await))
        }
        Some(MinecraftLoginResult::Staged(_)) => {
            Err(other_error("Minecraft sign-in persisted an invalid result"))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn remove_user(user: Uuid) -> Result<()> {
    Ok(minecraft_auth::remove_user(user).await?)
}

#[tauri::command]
pub async fn get_default_user() -> Result<Option<Uuid>> {
    Ok(minecraft_auth::get_default_user().await?)
}

#[tauri::command]
pub async fn set_default_user(user: Uuid) -> Result<()> {
    Ok(minecraft_auth::set_default_user(user).await?)
}

#[tauri::command]
pub async fn get_users() -> Result<Vec<minecraft_auth::MinecraftAccountSummary>>
{
    Ok(minecraft_auth::users().await?)
}

#[tauri::command]
pub async fn get_amberite_local_setup_secret() -> Result<Option<String>> {
    for data_dir in local_core_data_dir_candidates() {
        let path = data_dir.join(".setup_secret");
        let Ok(value) = tokio::fs::read_to_string(&path).await else {
            continue;
        };
        let secret = value.trim().to_string();
        if !secret.is_empty() {
            return Ok(Some(secret));
        }
    }
    Ok(None)
}

fn local_core_data_dir_candidates() -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(value) = env::var(AMBERITE_LOCAL_CORE_DATA_DIR_ENV) {
        let value = value.trim();
        if !value.is_empty() {
            candidates.push(std::path::PathBuf::from(value));
        }
    }
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join(".copal"));
        candidates.push(cwd.join("apps").join("core").join(".copal"));
        candidates.push(cwd.join("..").join("core").join(".copal"));
    }
    candidates
}
