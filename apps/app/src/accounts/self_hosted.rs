use contracts::Empty;
use contracts::backend::accounts::{DeleteSession, ListSessions, SessionRef};
use contracts::client::{Remote, call};
use engine::self_hosted::accounts::{api, enabled, origin};
use engine::{
    BackendSession, BackendSessionKind, ErrorKind, INSECURE_REQWEST_CLIENT,
    ModrinthSession, Sessions,
};
use reqwest::Url;

use crate::accounts::state::ModrinthCredentials;

/// The backend at `base`, called as `bearer`.
pub(crate) fn remote<'a>(base: &'a Url, bearer: &'a str) -> Remote<'a> {
    Remote {
        http: &INSECURE_REQWEST_CLIENT,
        base: base.as_str(),
        bearer: Some(bearer),
        read_ticket: None,
        timeout: None,
    }
}

/// The signed-in account's backend session, or `None` when accounts go to Modrinth.
pub(crate) fn backend(
    sessions: &Sessions,
) -> engine::Result<Option<BackendSession>> {
    if !enabled(sessions) {
        return Ok(None);
    }
    Ok(Some(sessions.backend().ok_or(ErrorKind::NotLoggedIn)?))
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
    sessions.set_backend_session(api(sessions).zip(credentials).map(
        |(api, credentials)| BackendSession {
            base_url: api.clone(),
            bearer: credentials.session.clone(),
            kind: BackendSessionKind::User {
                user_id: credentials.user_id.clone(),
            },
        },
    ));
}

/// Copied development data may contain credentials for a different service.
/// Deactivate them before any requests; preserve the rows for the original app.
pub(crate) async fn bind_origin(
    pool: &sqlx::SqlitePool,
    sessions: &Sessions,
) -> engine::Result<()> {
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
    sessions: &Sessions,
) -> engine::Result<()> {
    let Some(api) = api(sessions) else {
        return Ok(());
    };
    let backend = remote(api, &credentials.session);
    let listed = call::<ListSessions>(&backend, &(), &(), Empty).await?;
    if let Some(session) =
        listed.into_iter().find(|session| session.details.current)
    {
        call::<DeleteSession>(
            &backend,
            &SessionRef {
                id: session.details.id,
            },
            &(),
            Empty,
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

    #[tokio::test]
    async fn account_calls_go_to_the_backend_with_the_session() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        engine::install_crypto_provider();
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = format!("http://{}/api", listener.local_addr().unwrap());
        let request = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut head = vec![0; 4096];
            let read = socket.read(&mut head).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n[]")
                .await
                .unwrap();
            String::from_utf8_lossy(&head[..read]).to_lowercase()
        });
        let dir = tempfile::tempdir().unwrap();
        let engine = engine::Engine::new(
            crate::data_dir::engine_config(
                dir.path().to_path_buf(),
                engine::EventSink::new(),
                Some(engine::AccountEndpoints::new(&api, None).unwrap()),
            )
            .await
            .unwrap(),
        )
        .await
        .unwrap();
        set_sessions(&engine.sessions, Some(&credentials()));

        let friends = crate::friends::state::FriendsSocket::friends(
            &engine.api_semaphore,
            &engine.sessions,
        )
        .await
        .unwrap();

        assert!(friends.is_empty());
        let request = request.await.unwrap();
        assert!(request.starts_with("get /api/friends "), "{request}");
        assert!(
            request.contains("\r\nauthorization: bearer imported-session\r\n"),
            "{request}"
        );
        assert_eq!(
            engine
                .sessions
                .modrinth_token(concat!(env!("MODRINTH_API_URL"), "project/x")),
            None
        );
    }

    #[tokio::test]
    async fn imported_accounts_cannot_be_reactivated_on_another_service() {
        engine::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let engine = engine::Engine::new(
            crate::data_dir::engine_config(
                dir.path().to_path_buf(),
                engine::EventSink::new(),
                Some(
                    engine::AccountEndpoints::new(
                        "https://accounts.example/",
                        None,
                    )
                    .unwrap(),
                ),
            )
            .await
            .unwrap(),
        )
        .await
        .unwrap();
        let (pool, sessions) = (crate::db::pool(&engine), &engine.sessions);
        credentials().upsert(pool).await.unwrap();
        sqlx::query("INSERT INTO app_metadata(key,value,updated_at) VALUES ('account_origin','https://another-service.example/',unixepoch())")
            .execute(pool).await.unwrap();
        set_sessions(sessions, Some(&credentials()));
        assert!(sessions.backend().is_some());
        bind_origin(pool, sessions).await.unwrap();
        assert!(
            sessions
                .modrinth_token("https://accounts.example/v2/user")
                .is_none()
        );
        assert!(sessions.backend().is_none());
        assert!(
            ModrinthCredentials::get_active(pool)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            ModrinthCredentials::get_all(pool, sessions)
                .await
                .unwrap()
                .is_empty()
        );
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM modrinth_users")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(count, 1);
        // A restart must not relabel foreign credentials as belonging to this service.
        bind_origin(pool, sessions).await.unwrap();
        assert!(
            ModrinthCredentials::get_all(pool, sessions)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
