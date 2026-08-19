use crate::api::Result;
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::sync::LazyLock;
use std::time::Duration as StdDuration;
use tauri::plugin::TauriPlugin;
use tauri::{Manager, Runtime, UserAttentionType};
use theseus::prelude::*;
use uuid::Uuid;

const AMBERITE_KEYRING_SERVICE: &str = "dev.amberite.app";
const AMBERITE_SESSION_JWT_ACCOUNT: &str = "amberite-session-jwt";
const AMBERITE_REFRESH_TOKEN_ACCOUNT: &str = "amberite-session-refresh-token";
const AMBERITE_AUTH_METADATA_FILE: &str = "amberite-auth.json";
const AMBERITE_LOCAL_CORE_DATA_DIR_ENV: &str = "AMBERITE_LOCAL_CORE_DATA_DIR";
static CONVEX_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(10))
        .timeout(StdDuration::from_secs(30))
        .build()
        .expect("valid Convex HTTP client")
});

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("auth")
        .invoke_handler(tauri::generate_handler![
            check_reachable,
            login,
            remove_user,
            get_default_user,
            set_default_user,
            get_users,
            check_amberite_reachable,
            amberite_product_sign_in,
            restore_amberite_product_session,
            refresh_amberite_product_session,
            sign_out_amberite_product_session,
            get_remembered_amberite_identity,
            get_amberite_local_setup_secret,
            set_amberite_shared_clients_session,
        ])
        .build()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeAmberiteSessionSummary {
    access_token: String,
    user: Value,
    expires_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RememberedAmberiteIdentity {
    pub minecraft_uuid: Uuid,
    pub verified_minecraft_handle: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub last_successful_sign_in: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct AmberiteAuthMetadata {
    remembered_identity: Option<RememberedAmberiteIdentity>,
    signed_out: bool,
    pending_minecraft_uuid: Option<Uuid>,
    connection_error: Option<String>,
}

struct AmberiteSessionPair {
    access_token: String,
    refresh_token: String,
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

fn auth_error(
    code: &str,
    recovery: &str,
    error: impl std::fmt::Display,
) -> crate::api::TheseusSerializableError {
    other_error(json!({
        "code": code,
        "message": error.to_string(),
        "recovery": recovery,
    }))
}

fn keyring_account(base: &str) -> String {
    match crate::dev::config() {
        Some(config) => {
            format!("{base}:dev:{}", config.credential_namespace)
        }
        None => base.to_string(),
    }
}

fn metadata_path() -> Result<std::path::PathBuf> {
    let directories = DirectoryInfo::global_handle_if_ready()
        .ok_or_else(|| other_error("Launcher state is not ready"))?;
    Ok(directories.settings_dir.join(AMBERITE_AUTH_METADATA_FILE))
}

async fn read_auth_metadata() -> Result<AmberiteAuthMetadata> {
    let path = metadata_path()?;
    match tokio::fs::read(path).await {
        Ok(value) => serde_json::from_slice(&value).map_err(other_error),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(AmberiteAuthMetadata::default())
        }
        Err(error) => Err(other_error(error)),
    }
}

async fn write_auth_metadata(metadata: &AmberiteAuthMetadata) -> Result<()> {
    let path = metadata_path()?;
    let value = serde_json::to_vec(metadata).map_err(other_error)?;
    tokio::fs::write(path, value).await.map_err(other_error)
}

fn clear_keyring_entries_blocking(
    access_account: &str,
    refresh_account: &str,
) -> std::result::Result<(), keyring::Error> {
    let mut first_error = None;
    for account in [access_account, refresh_account] {
        let entry = match keyring::Entry::new(AMBERITE_KEYRING_SERVICE, account)
        {
            Ok(entry) => entry,
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
                continue;
            }
        };
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) if first_error.is_none() => first_error = Some(error),
            Err(_) => {}
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

async fn clear_amberite_session() -> Result<()> {
    let access_account = keyring_account(AMBERITE_SESSION_JWT_ACCOUNT);
    let refresh_account = keyring_account(AMBERITE_REFRESH_TOKEN_ACCOUNT);
    tokio::task::spawn_blocking(move || {
        clear_keyring_entries_blocking(&access_account, &refresh_account)
    })
    .await
    .map_err(other_error)?
    .map_err(other_error)
}

async fn read_amberite_session() -> Result<Option<AmberiteSessionPair>> {
    let access_account = keyring_account(AMBERITE_SESSION_JWT_ACCOUNT);
    let refresh_account = keyring_account(AMBERITE_REFRESH_TOKEN_ACCOUNT);
    tokio::task::spawn_blocking(move || {
        let access_entry =
            keyring::Entry::new(AMBERITE_KEYRING_SERVICE, &access_account)?;
        let refresh_entry =
            keyring::Entry::new(AMBERITE_KEYRING_SERVICE, &refresh_account)?;
        let access = match access_entry.get_password() {
            Ok(value) => Some(value),
            Err(keyring::Error::NoEntry) => None,
            Err(error) => return Err(error),
        };
        let refresh = match refresh_entry.get_password() {
            Ok(value) => Some(value),
            Err(keyring::Error::NoEntry) => None,
            Err(error) => return Err(error),
        };
        match (access, refresh) {
            (Some(access_token), Some(refresh_token))
                if !access_token.trim().is_empty()
                    && !refresh_token.trim().is_empty() =>
            {
                Ok(Some(AmberiteSessionPair {
                    access_token,
                    refresh_token,
                }))
            }
            (None, None) => Ok(None),
            _ => {
                clear_keyring_entries_blocking(
                    &access_account,
                    &refresh_account,
                )?;
                Ok(None)
            }
        }
    })
    .await
    .map_err(other_error)?
    .map_err(other_error)
}

async fn write_amberite_session(tokens: &ConvexTokens) -> Result<()> {
    let access_account = keyring_account(AMBERITE_SESSION_JWT_ACCOUNT);
    let refresh_account = keyring_account(AMBERITE_REFRESH_TOKEN_ACCOUNT);
    let access_token = tokens.token.clone();
    let refresh_token = tokens.refresh_token.clone();
    tokio::task::spawn_blocking(move || {
        let result = (|| {
            let access_entry =
                keyring::Entry::new(AMBERITE_KEYRING_SERVICE, &access_account)?;
            let refresh_entry = keyring::Entry::new(
                AMBERITE_KEYRING_SERVICE,
                &refresh_account,
            )?;
            access_entry.set_password(&access_token)?;
            refresh_entry.set_password(&refresh_token)
        })();
        if let Err(error) = result {
            let _ = clear_keyring_entries_blocking(
                &access_account,
                &refresh_account,
            );
            return Err(error);
        }
        Ok(())
    })
    .await
    .map_err(other_error)?
    .map_err(other_error)
}

#[tauri::command]
pub async fn check_amberite_reachable(convex_url: String) -> Result<()> {
    let _: Option<Value> =
        convex_call(&convex_url, "query", "auth:currentUser", json!({}), None)
            .await?;
    Ok(())
}

#[tauri::command]
pub fn set_amberite_shared_clients_session(
    convex_site_url: String,
    access_token: Option<String>,
    user_id: Option<String>,
) -> Result<()> {
    let url = url::Url::parse(&convex_site_url).map_err(other_error)?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1"));
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        return Err(other_error(
            "Convex site URL must use HTTPS unless it is local",
        ));
    }
    theseus::instance::set_shared_clients_session(
        convex_site_url,
        access_token,
        user_id,
    );
    Ok(())
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
    let login_result =
        run_minecraft_login(&app, true, mode == "use_another_account")
            .await?
            .ok_or_else(|| {
                auth_error(
                    "cancelled",
                    "return_to_provider",
                    "Minecraft sign-in was cancelled",
                )
            })?;
    let staged = match login_result {
        MinecraftLoginResult::Staged(staged) => staged,
        MinecraftLoginResult::Persisted(_) => {
            return Err(other_error(
                "Amberite sign-in persisted an invalid result",
            ));
        }
    };
    if expected
        .is_some_and(|uuid| uuid != staged.credentials.offline_profile.id)
    {
        return Err(auth_error(
            "identity_mismatch",
            "clear_session",
            "Minecraft UUID mismatch",
        ));
    }
    let minecraft_uuid = staged.credentials.offline_profile.id;
    let previous_session = read_amberite_session().await?;
    let mut metadata = read_auth_metadata().await?;

    let (tokens, user) = match convex_minecraft_sign_in(
        &convex_url,
        &staged.credentials.access_token,
        expected.map(|value| value.as_hyphenated().to_string()),
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            if !is_retryable_error(&error) {
                metadata.pending_minecraft_uuid = None;
                metadata.connection_error = Some(error.to_string());
                write_auth_metadata(&metadata).await?;
                // IMPORTANT: Capture permanent identity-link failures in Sentry once available.
            }
            return Err(error);
        }
    };
    let identity_uuid = verified_user_uuid(&user)?;
    if identity_uuid != minecraft_uuid {
        return Err(auth_error(
            "identity_mismatch",
            "clear_session",
            "Amberite identity UUID mismatch",
        ));
    }
    minecraft_auth::commit_staged_login(staged)
        .await
        .map_err(other_error)?;
    let expires_at = jwt_expiry(&tokens.token)?;
    write_amberite_session(&tokens).await?;
    metadata.signed_out = false;
    metadata.remembered_identity =
        Some(remembered_identity(&user, identity_uuid)?);
    metadata.pending_minecraft_uuid = None;
    metadata.connection_error = None;
    write_auth_metadata(&metadata).await?;

    if let Some(previous) = previous_session
        && previous.access_token != tokens.token
    {
        let _ = convex_call::<Value>(
            &convex_url,
            "action",
            "auth:signOut",
            json!({}),
            Some(&previous.access_token),
        )
        .await;
    }
    Ok(summary(tokens.token, user, expires_at))
}

#[tauri::command]
pub async fn restore_amberite_product_session(
    convex_url: String,
) -> Result<Option<NativeAmberiteSessionSummary>> {
    let mut metadata = read_auth_metadata().await?;
    if metadata.signed_out {
        return Ok(None);
    }
    let credentials = minecraft_auth::default_credential()
        .await
        .map_err(other_error)?;
    let Some(credentials) = credentials else {
        clear_amberite_session().await?;
        metadata.pending_minecraft_uuid = None;
        write_auth_metadata(&metadata).await?;
        return Ok(None);
    };

    let Some(session) = read_amberite_session().await? else {
        if metadata.connection_error.is_none()
            && metadata.pending_minecraft_uuid
                == Some(credentials.offline_profile.id)
        {
            let (tokens, user) = convex_minecraft_sign_in(
                &convex_url,
                &credentials.access_token,
                Some(credentials.offline_profile.id.to_string()),
            )
            .await?;
            let identity_uuid = verified_user_uuid(&user)?;
            if identity_uuid != credentials.offline_profile.id {
                metadata.pending_minecraft_uuid = None;
                metadata.connection_error =
                    Some("Amberite identity UUID mismatch".to_string());
                write_auth_metadata(&metadata).await?;
                return Err(auth_error(
                    "identity_mismatch",
                    "clear_session",
                    "Amberite identity UUID mismatch",
                ));
            }
            let expires_at = jwt_expiry(&tokens.token)?;
            write_amberite_session(&tokens).await?;
            metadata.remembered_identity =
                Some(remembered_identity(&user, identity_uuid)?);
            metadata.pending_minecraft_uuid = None;
            write_auth_metadata(&metadata).await?;
            return Ok(Some(summary(tokens.token, user, expires_at)));
        }
        return Ok(None);
    };
    let expires_at = jwt_expiry(&session.access_token)?;
    if expires_at <= Utc::now() + Duration::minutes(1) {
        return refresh_amberite_product_session(convex_url).await;
    }
    match convex_current_user(&convex_url, &session.access_token).await {
        Ok(user) => {
            if verified_user_uuid(&user)? != credentials.offline_profile.id {
                clear_amberite_session().await?;
                return Err(auth_error(
                    "identity_mismatch",
                    "clear_session",
                    "Amberite identity UUID mismatch",
                ));
            }
            Ok(Some(summary(session.access_token, user, expires_at)))
        }
        Err(error) if is_unauthorized(&error) => {
            refresh_amberite_product_session(convex_url).await
        }
        Err(error) => Err(error),
    }
}

#[tauri::command]
pub async fn refresh_amberite_product_session(
    convex_url: String,
) -> Result<Option<NativeAmberiteSessionSummary>> {
    let Some(previous) = read_amberite_session().await? else {
        return Ok(None);
    };
    let (tokens, user) =
        match convex_refresh(&convex_url, &previous.refresh_token).await {
            Ok(value) => value,
            Err(error) => {
                if is_terminal_refresh_error(&error) {
                    clear_amberite_session().await?;
                }
                return Err(error);
            }
        };
    let identity_uuid = verified_user_uuid(&user)?;
    let minecraft_uuid = minecraft_auth::get_default_user()
        .await
        .map_err(other_error)?;
    if minecraft_uuid != Some(identity_uuid) {
        clear_amberite_session().await?;
        return Err(auth_error(
            "identity_mismatch",
            "clear_session",
            "Amberite identity UUID mismatch",
        ));
    }
    let expires_at = jwt_expiry(&tokens.token)?;
    write_amberite_session(&tokens).await?;
    let mut metadata = read_auth_metadata().await?;
    metadata.remembered_identity =
        Some(remembered_identity(&user, identity_uuid)?);
    metadata.pending_minecraft_uuid = None;
    metadata.connection_error = None;
    write_auth_metadata(&metadata).await?;
    Ok(Some(summary(tokens.token, user, expires_at)))
}

#[tauri::command]
pub async fn get_remembered_amberite_identity()
-> Result<Option<RememberedAmberiteIdentity>> {
    Ok(read_auth_metadata().await?.remembered_identity)
}

#[tauri::command]
pub async fn sign_out_amberite_product_session(
    convex_url: String,
) -> Result<()> {
    if let Ok(Some(session)) = read_amberite_session().await {
        let _ = convex_call::<Value>(
            &convex_url,
            "action",
            "auth:signOut",
            json!({}),
            Some(&session.access_token),
        )
        .await;
    }
    clear_amberite_session().await?;
    let mut metadata = read_auth_metadata().await?;
    metadata.signed_out = true;
    metadata.pending_minecraft_uuid = None;
    metadata.connection_error = None;
    write_auth_metadata(&metadata).await?;
    Ok(())
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
    let mut request = CONVEX_CLIENT
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
) -> Result<RememberedAmberiteIdentity> {
    let handle = user
        .get("verifiedMinecraftHandle")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            other_error("Amberite profile has no verified Minecraft handle")
        })?;
    let display_name =
        user.get("name").and_then(Value::as_str).unwrap_or(handle);
    Ok(RememberedAmberiteIdentity {
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

fn summary(
    access_token: String,
    user: Value,
    expires_at: DateTime<Utc>,
) -> NativeAmberiteSessionSummary {
    NativeAmberiteSessionSummary {
        access_token,
        user,
        expires_at,
    }
}

fn is_unauthorized(error: &crate::api::TheseusSerializableError) -> bool {
    let message = error.to_string().to_lowercase();
    message.contains("(401 ") || message.contains("not authenticated")
}

fn is_terminal_refresh_error(
    error: &crate::api::TheseusSerializableError,
) -> bool {
    let message = error.to_string().to_lowercase();
    message.contains("(401 ")
        || message.contains("(403 ")
        || message.contains("not authenticated")
        || message.contains("invalid session")
        || message.contains("invalid refresh")
        || message.contains("revoked")
        || (message.contains("refresh") && message.contains("reuse"))
}

fn is_retryable_error(error: &crate::api::TheseusSerializableError) -> bool {
    let message = error.to_string().to_lowercase();
    message.contains("network")
        || message.contains("connect")
        || message.contains("timeout")
        || message.contains("offline")
        || message.contains("unreachable")
        || message.contains("error sending request")
}

enum MinecraftLoginResult {
    Persisted(Credentials),
    Staged(minecraft_auth::StagedMinecraftLogin),
}

enum MicrosoftOAuthRedirect {
    Code(String),
    Cancelled,
}

fn microsoft_oauth_redirect(url: &url::Url) -> Option<MicrosoftOAuthRedirect> {
    if url.host_str() != Some("login.live.com")
        || !url.path().eq_ignore_ascii_case("/oauth20_desktop.srf")
    {
        return None;
    }

    let values =
        url.query_pairs().chain(url.fragment().into_iter().flat_map(
            |fragment| url::form_urlencoded::parse(fragment.as_bytes()),
        ));
    for (key, value) in values {
        if key == "code" {
            return Some(MicrosoftOAuthRedirect::Code(value.into_owned()));
        }
        if key == "error" {
            return Some(MicrosoftOAuthRedirect::Cancelled);
        }
    }
    Some(MicrosoftOAuthRedirect::Cancelled)
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
    .title("Sign into Amberite")
    .always_on_top(true)
    .min_inner_size(500.0, 500.0)
    .inner_size(1000.0, 700.0)
    .focused(true)
    .center()
    .build()?;
    window.request_user_attention(Some(UserAttentionType::Critical))?;

    while (Utc::now() - start) < Duration::minutes(10) {
        if window.title().is_err() {
            return Ok(None);
        }
        if let Some(redirect) = microsoft_oauth_redirect(&window.url()?) {
            window.close()?;
            let MicrosoftOAuthRedirect::Code(code) = redirect else {
                return Ok(None);
            };
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

#[cfg(test)]
mod tests {
    use super::{MicrosoftOAuthRedirect, microsoft_oauth_redirect};

    #[test]
    fn extracts_microsoft_oauth_codes_from_query_or_fragment() {
        for url in [
            "https://login.live.com/oauth20_desktop.srf?code=query-code",
            "https://login.live.com/oauth20_desktop.srf#code=fragment-code",
        ] {
            assert!(matches!(
                microsoft_oauth_redirect(&url::Url::parse(url).unwrap()),
                Some(MicrosoftOAuthRedirect::Code(_))
            ));
        }
    }

    #[test]
    fn ignores_non_callback_urls_and_closes_cancelled_callbacks() {
        assert!(
            microsoft_oauth_redirect(
                &url::Url::parse(
                    "https://login.live.com/oauth20_authorize.srf"
                )
                .unwrap()
            )
            .is_none()
        );
        assert!(matches!(
            microsoft_oauth_redirect(
                &url::Url::parse(
                    "https://login.live.com/oauth20_desktop.srf?error=access_denied"
                )
                .unwrap()
            ),
            Some(MicrosoftOAuthRedirect::Cancelled)
        ));
    }
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
    minecraft_auth::remove_user(user).await?;
    let mut metadata = read_auth_metadata().await?;
    if metadata
        .remembered_identity
        .as_ref()
        .is_some_and(|identity| identity.minecraft_uuid == user)
    {
        clear_amberite_session().await?;
        metadata = AmberiteAuthMetadata::default();
        write_auth_metadata(&metadata).await?;
    }
    Ok(())
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
