use crate::state::{
    BackendSession, BackendSessionKind, ModrinthCredentials, ModrinthSession,
    Sessions, User,
};
use crate::util::fetch::{FetchSemaphore, fetch_json};
use reqwest::{Method, Url};
use std::borrow::Cow;

/// The account service the host routes Modrinth account calls to.
#[derive(Debug)]
pub struct AccountEndpoints {
    pub(crate) api: Url,
    web: String,
    socket: String,
}

impl AccountEndpoints {
    pub fn new(api: &str, web: Option<&str>) -> crate::Result<Self> {
        let api = Url::parse(api)?;
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
        Ok(Self {
            api: Url::parse(&api)?,
            web,
            socket,
        })
    }
}

pub fn enabled(sessions: &Sessions) -> bool {
    sessions.accounts.is_some()
}

pub fn login_url(sessions: &Sessions, sign_up: bool) -> Option<String> {
    sessions.accounts.as_ref().map(|config| {
        format!(
            "{}auth/{}",
            config.web,
            if sign_up { "sign-up" } else { "sign-in" }
        )
    })
}

pub(crate) fn socket_base(sessions: &Sessions) -> &str {
    sessions
        .accounts
        .as_ref()
        .map_or(env!("MODRINTH_SOCKET_URL"), |config| config.socket.as_str())
}

/// Only account routes move. Projects, versions, teams and CDN bytes stay public.
pub(crate) fn route<'a>(
    sessions: &Sessions,
    url: &'a str,
    path: Option<&str>,
) -> Cow<'a, str> {
    let Some(config) = &sessions.accounts else {
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

pub(crate) fn may_send_credentials(sessions: &Sessions, url: &str) -> bool {
    if let Some(config) = &sessions.accounts {
        return Url::parse(url).is_ok_and(|url| {
            url.origin() == config.api.origin()
                && url.path().starts_with(config.api.path())
        });
    }
    url.starts_with(env!("MODRINTH_API_URL"))
        || url.starts_with(env!("MODRINTH_API_URL_V3"))
        || url.starts_with("https://cdn.modrinth.com/")
}

/// Signs the engine in as `credentials`, the account now active in the database, or out.
pub(crate) fn set_sessions(
    sessions: &Sessions,
    credentials: Option<&ModrinthCredentials>,
) {
    sessions.set_modrinth_session(credentials.map(|credentials| {
        ModrinthSession {
            token: credentials.session.clone(),
        }
    }));
    sessions.set_backend_session(
        sessions.accounts.as_ref().zip(credentials).map(
            |(config, credentials)| BackendSession {
                base_url: config.api.clone(),
                bearer: credentials.session.clone(),
                kind: BackendSessionKind::User {
                    user_id: credentials.user_id.clone(),
                },
            },
        ),
    );
}

/// Local accounts bypass the public author cache, where the same ID could refer
/// to a completely different person embedded in a Modrinth project team.
pub(crate) async fn users(
    ids: &[&str],
    semaphore: &FetchSemaphore,
    sessions: &Sessions,
) -> crate::Result<Vec<User>> {
    let ids = urlencoding::encode(&serde_json::to_string(ids)?).into_owned();
    fetch_json(
        Method::GET,
        &format!("{}users?ids={ids}", env!("MODRINTH_API_URL")),
        None,
        None,
        Some("/v2/users"),
        semaphore,
        sessions,
    )
    .await
}

pub(crate) fn origin(sessions: &Sessions) -> &str {
    sessions
        .accounts
        .as_ref()
        .map_or(env!("MODRINTH_API_URL"), |config| config.api.as_str())
}

/// Copied development data may contain credentials for a different service.
/// Deactivate them before any requests; preserve the rows for the original app.
pub(crate) async fn bind_origin(
    pool: &sqlx::SqlitePool,
    sessions: &Sessions,
) -> crate::Result<()> {
    let origin = origin(sessions);
    let previous: Option<String> = sqlx::query_scalar(
        "SELECT value FROM app_metadata WHERE key = 'account_origin'",
    )
    .fetch_optional(pool)
    .await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS self_hosted_account_origins (user_id TEXT PRIMARY KEY REFERENCES modrinth_users(id) ON DELETE CASCADE, origin TEXT NOT NULL)")
        .execute(pool).await?;
    // Existing saved accounts belong to the service recorded before this startup.
    sqlx::query("INSERT OR IGNORE INTO self_hosted_account_origins (user_id,origin) SELECT id,? FROM modrinth_users")
        .bind(previous.as_deref().unwrap_or(env!("MODRINTH_API_URL")))
        .execute(pool).await?;
    if previous.as_deref().unwrap_or(env!("MODRINTH_API_URL")) != origin {
        sqlx::query("UPDATE modrinth_users SET active = FALSE")
            .execute(pool)
            .await?;
    }
    sqlx::query("INSERT INTO app_metadata (key,value,updated_at) VALUES ('account_origin',?,unixepoch()) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at").bind(origin).execute(pool).await?;
    set_sessions(
        sessions,
        ModrinthCredentials::get_active(pool).await?.as_ref(),
    );
    Ok(())
}

pub(crate) async fn revoke(
    credentials: &ModrinthCredentials,
    semaphore: &FetchSemaphore,
    sessions: &Sessions,
) -> crate::Result<()> {
    if !enabled(sessions) {
        return Ok(());
    }
    #[derive(serde::Deserialize)]
    struct Session {
        id: String,
        current: bool,
    }
    let response = crate::util::fetch::fetch_advanced(
        Method::GET,
        concat!(env!("MODRINTH_API_URL"), "session/list"),
        None,
        None,
        Some(("Authorization", &credentials.session)),
        None,
        None,
        Some("/v2/session/list"),
        semaphore,
        sessions,
    )
    .await?;
    let listed: Vec<Session> = serde_json::from_slice(&response)?;
    if let Some(session) = listed.into_iter().find(|session| session.current) {
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
            sessions,
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credentials() -> ModrinthCredentials {
        ModrinthCredentials {
            session: "imported-session".into(),
            expires: chrono::Utc::now() + chrono::Duration::days(1),
            user_id: "imported-user".into(),
            active: true,
        }
    }

    fn signed_in(accounts: Option<&str>) -> Sessions {
        let sessions = Sessions::new(
            accounts.map(|api| AccountEndpoints::new(api, None).unwrap()),
        );
        set_sessions(&sessions, Some(&credentials()));
        sessions
    }

    #[test]
    fn the_session_reaches_only_urls_that_may_receive_credentials() {
        let modrinth = concat!(env!("MODRINTH_API_URL"), "project/x");
        let modrinth_v3 = concat!(env!("MODRINTH_API_URL_V3"), "project/x");
        let cases = [
            (None, modrinth, true),
            (None, modrinth_v3, true),
            (None, "https://cdn.modrinth.com/data/x.jar", true),
            (None, "https://accounts.example/v2/user", false),
            (
                Some("https://accounts.example/api"),
                "https://accounts.example/api/v2/user",
                true,
            ),
            (
                Some("https://accounts.example/api"),
                "https://accounts.example/other",
                false,
            ),
            (
                Some("https://accounts.example/api"),
                "http://accounts.example/api/v2/user",
                false,
            ),
            (Some("https://accounts.example/api"), modrinth, false),
            (
                Some("https://accounts.example/api"),
                "https://cdn.modrinth.com/data/x.jar",
                false,
            ),
        ];
        for (accounts, url, allowed) in cases {
            let sessions = signed_in(accounts);
            assert_eq!(
                sessions.backend().is_some(),
                accounts.is_some(),
                "{url}"
            );
            assert_eq!(
                sessions.modrinth_token(url).as_deref(),
                allowed.then_some("imported-session"),
                "{url}"
            );
            set_sessions(&sessions, None);
            assert_eq!(sessions.modrinth_token(url), None, "{url}");
            assert!(sessions.backend().is_none());
        }
    }

    #[tokio::test]
    async fn imported_accounts_cannot_be_reactivated_on_another_service() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        credentials().upsert(&pool).await.unwrap();
        sqlx::query("INSERT INTO app_metadata(key,value,updated_at) VALUES ('account_origin','https://another-service.example/',unixepoch())")
            .execute(&pool).await.unwrap();
        let sessions = signed_in(Some("https://accounts.example/"));
        assert!(sessions.backend().is_some());
        bind_origin(&pool, &sessions).await.unwrap();
        assert!(
            sessions
                .modrinth_token("https://accounts.example/v2/user")
                .is_none()
        );
        assert!(sessions.backend().is_none());
        assert!(
            ModrinthCredentials::get_active(&pool)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            ModrinthCredentials::get_all(&pool, &sessions)
                .await
                .unwrap()
                .is_empty()
        );
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM modrinth_users")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1);
        // A restart must not relabel foreign credentials as belonging to this service.
        bind_origin(&pool, &sessions).await.unwrap();
        assert!(
            ModrinthCredentials::get_all(&pool, &sessions)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
