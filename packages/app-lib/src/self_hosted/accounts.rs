use crate::state::{ModrinthCredentials, User};
use crate::util::fetch::{FetchSemaphore, fetch_json};
use reqwest::Method;
use std::borrow::Cow;
use std::sync::OnceLock;

struct Endpoints {
    api: String,
    web: String,
    socket: String,
}
static ENDPOINTS: OnceLock<Endpoints> = OnceLock::new();

/// Set once before launcher state initializes so database refresh and sockets use
/// the same service as interactive account actions.
pub fn configure(api: &str, web: Option<&str>) -> crate::Result<()> {
    let api = reqwest::Url::parse(api)?;
    if !matches!(api.scheme(), "http" | "https") || api.cannot_be_a_base() {
        return Err(crate::ErrorKind::InputError(
            "Invalid account service URL".into(),
        )
        .into());
    }
    let mut socket = api.clone();
    socket
        .set_scheme(if api.scheme() == "https" { "wss" } else { "ws" })
        .map_err(|_| {
            crate::ErrorKind::InputError("Invalid socket URL".into())
        })?;
    let api = format!("{}/", api.as_str().trim_end_matches('/'));
    let web = format!("{}/", web.unwrap_or(&api).trim_end_matches('/'));
    let socket = format!("{}/", socket.as_str().trim_end_matches('/'));
    ENDPOINTS.set(Endpoints { api, web, socket }).map_err(|_| {
        crate::ErrorKind::InputError(
            "Account service already configured".into(),
        )
    })?;
    Ok(())
}

pub fn enabled() -> bool {
    ENDPOINTS.get().is_some()
}

pub fn login_url(sign_up: bool) -> Option<String> {
    ENDPOINTS.get().map(|config| {
        format!(
            "{}auth/{}",
            config.web,
            if sign_up { "sign-up" } else { "sign-in" }
        )
    })
}

pub(crate) fn socket_base() -> &'static str {
    ENDPOINTS
        .get()
        .map_or(env!("MODRINTH_SOCKET_URL"), |config| config.socket.as_str())
}

/// Only account routes move. Projects, versions, teams and CDN bytes stay public.
pub(crate) fn route<'a>(url: &'a str, path: Option<&str>) -> Cow<'a, str> {
    let Some(config) = ENDPOINTS.get() else {
        return Cow::Borrowed(url);
    };
    let Some(path) = path else {
        return Cow::Borrowed(url);
    };
    let private = path.starts_with("/v2/user")
        || path.starts_with("/v3/user")
        || path.starts_with("/v2/session")
        || path.starts_with("/v3/friend")
        || path.starts_with("/v3/block")
        || path.starts_with("/v2/notification")
        || path.starts_with("/v3/notification");
    if private {
        for (base, version) in [
            (env!("MODRINTH_API_URL"), "v2/"),
            (env!("MODRINTH_API_URL_V3"), "v3/"),
        ] {
            if let Some(suffix) = url.strip_prefix(base) {
                return Cow::Owned(format!("{}{version}{suffix}", config.api));
            }
        }
    }
    Cow::Borrowed(url)
}

pub(crate) fn may_send_credentials(url: &str) -> bool {
    if let Some(config) = ENDPOINTS.get() {
        return reqwest::Url::parse(url)
            .ok()
            .zip(reqwest::Url::parse(&config.api).ok())
            .is_some_and(|(url, base)| {
                url.origin() == base.origin()
                    && url.path().starts_with(base.path())
            });
    }
    url.starts_with(env!("MODRINTH_API_URL"))
        || url.starts_with(env!("MODRINTH_API_URL_V3"))
        || url.starts_with("https://cdn.modrinth.com/")
}

pub(crate) fn sync_sharing(credentials: Option<&ModrinthCredentials>) {
    if let Some(config) = ENDPOINTS.get() {
        crate::instance::set_shared_clients_session(
            config.api.clone(),
            credentials.map(|value| value.session.clone()),
            credentials.map(|value| value.user_id.clone()),
        );
    }
}

/// Local accounts bypass the public author cache, where the same ID could refer
/// to a completely different person embedded in a Modrinth project team.
pub(crate) async fn users(
    ids: &[&str],
    semaphore: &FetchSemaphore,
    pool: &sqlx::SqlitePool,
) -> crate::Result<Vec<User>> {
    let ids = urlencoding::encode(&serde_json::to_string(ids)?).into_owned();
    fetch_json(
        Method::GET,
        &format!("{}users?ids={ids}", env!("MODRINTH_API_URL")),
        None,
        None,
        Some("/v2/users"),
        semaphore,
        pool,
    )
    .await
}

/// Copied development data may contain credentials for a different service.
/// Deactivate them before any requests; preserve the rows for the original app.
pub(crate) async fn bind_origin(pool: &sqlx::SqlitePool) -> crate::Result<()> {
    let origin = ENDPOINTS
        .get()
        .map_or(env!("MODRINTH_API_URL"), |config| config.api.as_str());
    let previous: Option<String> = sqlx::query_scalar(
        "SELECT value FROM app_metadata WHERE key = 'account_origin'",
    )
    .fetch_optional(pool)
    .await?;
    if previous.as_deref().unwrap_or(env!("MODRINTH_API_URL")) != origin {
        sqlx::query("UPDATE modrinth_users SET active = FALSE")
            .execute(pool)
            .await?;
    }
    sqlx::query("INSERT INTO app_metadata (key,value,updated_at) VALUES ('account_origin',?,unixepoch()) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at").bind(origin).execute(pool).await?;
    sync_sharing(ModrinthCredentials::get_active(pool).await?.as_ref());
    Ok(())
}

pub(crate) async fn revoke(
    credentials: &ModrinthCredentials,
    semaphore: &FetchSemaphore,
    pool: &sqlx::SqlitePool,
) -> crate::Result<()> {
    if !enabled() {
        return Ok(());
    }
    #[derive(serde::Deserialize)]
    struct Session {
        id: String,
        current: bool,
    }
    let sessions: Vec<Session> = fetch_json(
        Method::GET,
        concat!(env!("MODRINTH_API_URL"), "session/list"),
        None,
        None,
        Some("/v2/session/list"),
        semaphore,
        pool,
    )
    .await?;
    if let Some(session) = sessions.into_iter().find(|session| session.current)
    {
        crate::util::fetch::fetch_advanced(
            Method::DELETE,
            &format!("{}session/{}", env!("MODRINTH_API_URL"), session.id),
            None,
            None,
            Some(("Authorization", &credentials.session)),
            None,
            None,
            Some("/v2/session/:id"),
            semaphore,
            pool,
        )
        .await?;
    }
    Ok(())
}
