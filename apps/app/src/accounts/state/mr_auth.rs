use crate::accounts::self_hosted::remote;
use chrono::{DateTime, Duration, TimeZone, Utc};
use contracts::Empty;
use contracts::backend::accounts::{GetCurrentUser, RefreshSession};
use contracts::client::call;
use engine::data::{CacheBehaviour, CachedEntry, User};
use engine::self_hosted::accounts::api;
use engine::{FetchSemaphore, Sessions, fetch_advanced};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ModrinthCredentials {
    pub session: String,
    pub expires: DateTime<Utc>,
    pub user_id: String,
    pub active: bool,
}

impl ModrinthCredentials {
    pub async fn get_and_refresh(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
        semaphore: &FetchSemaphore,
        sessions: &Sessions,
    ) -> engine::Result<Option<Self>> {
        let creds = Self::get_active(exec).await?;

        if let Some(mut creds) = creds {
            if creds.expires < Utc::now() {
                #[derive(Deserialize)]
                struct Session {
                    session: String,
                }

                let refreshed = match api(sessions) {
                    Some(api) => call::<RefreshSession>(
                        &remote(api, &creds.session),
                        &(),
                        &(),
                        Empty,
                    )
                    .await
                    .ok()
                    .map(|answer| answer.session),
                    None => fetch_advanced(
                        Method::POST,
                        concat!(env!("MODRINTH_API_URL"), "session/refresh"),
                        None,
                        None,
                        Some(("Authorization", &*creds.session)),
                        None,
                        None,
                        Some("/v2/session/refresh"),
                        semaphore,
                        sessions,
                    )
                    .await
                    .ok()
                    .and_then(|resp| {
                        serde_json::from_slice::<Session>(&resp).ok()
                    })
                    .map(|value| value.session),
                };

                if let Some(session) = refreshed {
                    creds.session = session;
                    creds.expires = Utc::now() + Duration::weeks(2);
                    creds.upsert(exec).await?;
                    crate::accounts::self_hosted::set_sessions(
                        sessions,
                        Some(&creds),
                    );

                    Ok(Some(creds))
                } else {
                    Self::remove(&creds.user_id, exec).await?;
                    crate::accounts::self_hosted::set_sessions(sessions, None);

                    Ok(None)
                }
            } else {
                Ok(Some(creds))
            }
        } else {
            Ok(None)
        }
    }

    pub async fn get_active(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> engine::Result<Option<Self>> {
        let res = sqlx::query!(
            "
            SELECT
                id, active, session_id, expires
            FROM modrinth_users
            WHERE active = TRUE
            "
        )
        .fetch_optional(exec)
        .await?;

        Ok(res.map(|x| Self {
            session: x.session_id,
            expires: Utc
                .timestamp_opt(x.expires, 0)
                .single()
                .unwrap_or_else(Utc::now),
            user_id: x.id,
            active: x.active == 1,
        }))
    }

    pub async fn get_all(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
        sessions: &Sessions,
    ) -> engine::Result<Vec<Self>> {
        let res: Vec<(String, i64, String, i64)> = sqlx::query_as(
            "SELECT users.id, users.active, users.session_id, users.expires FROM modrinth_users users JOIN self_hosted_account_origins origins ON origins.user_id = users.id WHERE origins.origin = ? ORDER BY users.rowid",
        )
        .bind(engine::self_hosted::accounts::origin(sessions))
        .fetch_all(exec)
        .await?;

        Ok(res
            .into_iter()
            .map(|(user_id, active, session, expires)| Self {
                session,
                expires: Utc
                    .timestamp_opt(expires, 0)
                    .single()
                    .unwrap_or_else(Utc::now),
                user_id,
                active: active == 1,
            })
            .collect())
    }

    pub async fn upsert(
        &self,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> engine::Result<()> {
        let expires = self.expires.timestamp();

        if self.active {
            Self::deactivate_all(exec).await?;
        }

        sqlx::query!(
            "
            INSERT INTO modrinth_users (id, active, session_id, expires)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id) DO UPDATE SET
                active = $2,
                session_id = $3,
                expires = $4
            ",
            self.user_id,
            self.active,
            self.session,
            expires,
        )
        .execute(exec)
        .await?;

        Ok(())
    }

    pub async fn deactivate_all(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> engine::Result<()> {
        sqlx::query!(
            "
                UPDATE modrinth_users
                SET active = FALSE
                "
        )
        .execute(exec)
        .await?;

        Ok(())
    }

    pub async fn remove(
        user_id: &str,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> engine::Result<()> {
        sqlx::query!(
            "
            DELETE FROM modrinth_users WHERE id = $1
            ",
            user_id,
        )
        .execute(exec)
        .await?;

        Ok(())
    }

    pub(crate) async fn refresh_all(
        pool: &SqlitePool,
        fetch_semaphore: &FetchSemaphore,
        sessions: &Sessions,
    ) -> engine::Result<()> {
        let all = Self::get_all(pool, sessions).await?;

        let user_ids = all.into_iter().map(|x| x.user_id).collect::<Vec<_>>();

        if engine::self_hosted::accounts::enabled(sessions) {
            engine::self_hosted::accounts::users(
                &user_ids.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
                fetch_semaphore,
                sessions,
            )
            .await?;
            return Ok(());
        }
        CachedEntry::get_user_many(
            &user_ids.iter().map(|x| &**x).collect::<Vec<_>>(),
            Some(CacheBehaviour::Bypass),
            pool,
            fetch_semaphore,
            sessions,
        )
        .await?;

        Ok(())
    }
}

pub const fn get_login_url() -> &'static str {
    concat!(env!("MODRINTH_URL"), "auth/sign-in")
}

pub const fn get_signup_url() -> &'static str {
    concat!(env!("MODRINTH_URL"), "auth/sign-up")
}

pub async fn finish_login_flow(
    code: &str,
    semaphore: &FetchSemaphore,
    sessions: &Sessions,
) -> engine::Result<ModrinthCredentials> {
    // The authorization code actually is the access token, since labrinth doesn't
    // issue separate authorization codes. Therefore, this is equivalent to an
    // implicit OAuth grant flow, and no additional exchanging or finalization is
    // needed. TODO not do this for the reasons outlined at
    // https://oauth.net/2/grant-types/implicit/

    let user_id = fetch_user_id(code, semaphore, sessions).await?;

    Ok(ModrinthCredentials {
        session: code.to_string(),
        expires: Utc::now() + Duration::weeks(2),
        user_id,
        active: true,
    })
}

async fn fetch_user_id(
    token: &str,
    semaphore: &FetchSemaphore,
    sessions: &Sessions,
) -> engine::Result<String> {
    if let Some(api) = api(sessions) {
        let user = call::<GetCurrentUser>(&remote(api, token), &(), &(), Empty)
            .await?;
        return Ok(user.id);
    }
    let result = fetch_advanced(
        Method::GET,
        concat!(env!("MODRINTH_API_URL"), "user"),
        None,
        None,
        Some(("Authorization", token)),
        None,
        None,
        Some("/v2/user"),
        semaphore,
        sessions,
    )
    .await?;
    let user: User = serde_json::from_slice(&result)?;

    Ok(user.id)
}
