use engine::self_hosted::accounts::{api, enabled, origin};
use engine::{
    BackendSession, BackendSessionKind, FetchSemaphore, ModrinthSession,
    Sessions,
};
use reqwest::Method;

use crate::accounts::state::ModrinthCredentials;

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
    semaphore: &FetchSemaphore,
    sessions: &Sessions,
) -> engine::Result<()> {
    if !enabled(sessions) {
        return Ok(());
    }
    #[derive(serde::Deserialize)]
    struct Session {
        id: String,
        current: bool,
    }
    let response = engine::fetch_advanced(
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
        engine::fetch_advanced(
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
        let (pool, sessions) = (engine.pool(), &engine.sessions);
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
