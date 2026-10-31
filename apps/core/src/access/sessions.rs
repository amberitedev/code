//! Sessions: one per device per Core, made from a ticket. Only the SHA-256 of a token is stored,
//! and it is the only thing a session is looked up by, so the lookup's timing depends on the hash
//! and never on the token.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::rand::{SecureRandom, SystemRandom};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use super::{Caller, activity, keys::Keys, tickets};
use crate::{
    api::{
        activity::Action,
        error::{ApiError, ErrorCode},
        sessions::{Session, SignedIn},
    },
    db,
};

/// A session ends this long after it was last used.
pub const IDLE_S: i64 = 30 * 86_400;
/// And this long after it was made, however much it is used.
pub const MAX_S: i64 = 365 * 86_400;
/// `last_used_at` is written at most this often.
const TOUCH_EVERY_S: i64 = 3600;
pub const MAX_DEVICE_NAME: usize = 64;

fn hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn unauthorized() -> ApiError {
    ApiError::new(
        ErrorCode::Unauthorized,
        "This session ended. Sign in again with a ticket.",
    )
}

/// Make a session from a ticket. Not on this Core's access list: no session, and nothing is
/// stored, not even the ticket's use.
pub async fn sign_in(
    db: &SqlitePool,
    keys: &Keys,
    ticket: &str,
    device_name: &str,
    now: i64,
) -> Result<SignedIn, ApiError> {
    let device_name = device_name.trim();
    if device_name.is_empty() || device_name.chars().count() > MAX_DEVICE_NAME {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "A device name has 1 to 64 characters.",
        ));
    }
    let core_id = db::core_id(db).await?;
    let claims = tickets::verify(keys, ticket, &core_id, now).await?;

    let mut tx = db.begin().await?;
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core WHERE id = 1 AND owner_user_id = ?1) \
         OR EXISTS (SELECT 1 FROM access WHERE user_id = ?1)",
    )
    .bind(&claims.sub)
    .fetch_one(&mut *tx)
    .await?;
    if !allowed {
        return Err(ApiError::new(
            ErrorCode::NotInvited,
            "You have not been invited to any server on this Core.",
        ));
    }
    tickets::spend(&mut tx, &claims, now).await?;
    sqlx::query(
        "INSERT INTO users (id, name, avatar_url, updated_at) VALUES (?, ?, ?, ?) \
         ON CONFLICT (id) DO UPDATE SET name = excluded.name, \
         avatar_url = excluded.avatar_url, updated_at = excluded.updated_at",
    )
    .bind(&claims.sub)
    .bind(&claims.name)
    .bind(&claims.avatar_url)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM sessions WHERE expires_at <= ?1 OR last_used_at <= ?1 - ?2")
        .bind(now)
        .bind(IDLE_S)
        .execute(&mut *tx)
        .await?;

    let mut bytes = [0; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| ApiError::new(ErrorCode::Internal, "No randomness."))?;
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO sessions (id, token_hash, user_id, device_name, created_at, \
         last_used_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(hash(&token))
    .bind(&claims.sub)
    .bind(device_name)
    .bind(now)
    .bind(now)
    .bind(now + MAX_S)
    .execute(&mut *tx)
    .await?;
    activity::record(
        &mut tx,
        Some(&claims.sub),
        None,
        &Action::SignedIn {
            session_id: id.clone(),
            device_name: device_name.into(),
        },
    )
    .await?;
    tx.commit().await?;

    let session = get(db, &id, Some(&id)).await?.ok_or_else(unauthorized)?;
    Ok(SignedIn { token, session })
}

/// The caller behind a session token. Extends the session, writing `last_used_at` at most once
/// an hour.
pub async fn authenticate(
    db: &SqlitePool,
    token: &str,
    now: i64,
) -> Result<Caller, ApiError> {
    let row: Option<(String, String, i64, i64, bool)> = sqlx::query_as(
        "SELECT s.id, s.user_id, s.last_used_at, s.expires_at, \
         s.user_id IS (SELECT owner_user_id FROM core WHERE id = 1) \
         FROM sessions s WHERE s.token_hash = ?",
    )
    .bind(hash(token))
    .fetch_optional(db)
    .await?;
    let Some((id, user_id, last_used_at, expires_at, owner)) = row else {
        return Err(unauthorized());
    };
    if now >= expires_at || now >= last_used_at + IDLE_S {
        return Err(unauthorized());
    }
    if now - last_used_at >= TOUCH_EVERY_S {
        sqlx::query("UPDATE sessions SET last_used_at = ? WHERE id = ?")
            .bind(now)
            .bind(&id)
            .execute(db)
            .await?;
    }
    Ok(Caller::Session { id, user_id, owner })
}

const SESSION: &str = "SELECT s.id, s.user_id, u.name, u.avatar_url, s.device_name, \
                       s.created_at, s.last_used_at, s.expires_at \
                       FROM sessions s LEFT JOIN users u ON u.id = s.user_id";

type Row = (
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    i64,
    i64,
    i64,
);

fn session(row: Row, current: Option<&str>) -> Session {
    let (
        id,
        user_id,
        name,
        avatar_url,
        device_name,
        created_at,
        last_used_at,
        expires_at,
    ) = row;
    Session {
        current: current == Some(id.as_str()),
        id,
        user: crate::api::access::User {
            name: name.unwrap_or_else(|| user_id.clone()),
            id: user_id,
            avatar_url,
        },
        device_name,
        created_at,
        last_used_at,
        expires_at: expires_at.min(last_used_at + IDLE_S),
    }
}

/// One session. `current` is the id of the session asking.
pub async fn get(
    db: &SqlitePool,
    id: &str,
    current: Option<&str>,
) -> sqlx::Result<Option<Session>> {
    let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SESSION} WHERE s.id = ?"
    )))
    .bind(id)
    .fetch_optional(db)
    .await?;
    Ok(row.map(|row| session(row, current)))
}

/// Every live session, or only `user`'s.
pub async fn list(
    db: &SqlitePool,
    user: Option<&str>,
    current: Option<&str>,
    now: i64,
) -> sqlx::Result<Vec<Session>> {
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SESSION} WHERE (?1 IS NULL OR s.user_id = ?1) AND s.expires_at > ?2 \
         AND s.last_used_at > ?2 - ?3 ORDER BY s.user_id, s.created_at"
    )))
    .bind(user)
    .bind(now)
    .bind(IDLE_S)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|row| session(row, current)).collect())
}

/// End a session. Its user, or `None` when there was no such session.
pub async fn revoke(db: &SqlitePool, id: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("DELETE FROM sessions WHERE id = ? RETURNING user_id")
        .bind(id)
        .fetch_optional(db)
        .await
}

#[cfg(test)]
mod tests {
    use super::{super::tickets::test_support::*, *};

    const NOW: i64 = 1_800_000_000;

    async fn core() -> (SqlitePool, Keys, Signer, String) {
        let db = db::memory().await;
        let core_id = db::core_id(&db).await.unwrap();
        sqlx::query("UPDATE core SET owner_user_id = 'owner'")
            .execute(&db)
            .await
            .unwrap();
        let signer = Signer::new("k1");
        let keys = crate::access::keys::load(
            db.clone(),
            crate::network::http().build().unwrap(),
            None,
        )
        .await
        .unwrap();
        keys.set(&signer.public_keys()).await;
        (db, keys, signer, core_id)
    }

    #[tokio::test]
    async fn the_owner_signs_in_and_the_token_works() {
        let (db, keys, signer, core) = core().await;
        let ticket = signer.sign(&claims("owner", &core, NOW));
        let signed =
            sign_in(&db, &keys, &ticket, " Laptop ", NOW).await.unwrap();
        assert_eq!(signed.session.device_name, "Laptop");
        assert_eq!(signed.session.user.name, "owner name");
        assert!(signed.session.current);
        let caller = authenticate(&db, &signed.token, NOW + 1).await.unwrap();
        assert!(matches!(caller, Caller::Session { owner: true, .. }));
        // Only the hash is stored.
        let stored: Vec<u8> =
            sqlx::query_scalar("SELECT token_hash FROM sessions")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(stored, hash(&signed.token));
        assert_eq!(signed.token.len(), 43, "256 bits, base64url");
    }

    #[tokio::test]
    async fn someone_not_invited_gets_nothing_stored() {
        let (db, keys, signer, core) = core().await;
        let ticket = signer.sign(&claims("stranger", &core, NOW));
        let error = sign_in(&db, &keys, &ticket, "Laptop", NOW)
            .await
            .unwrap_err();
        assert_eq!(error.error, ErrorCode::NotInvited);
        for table in ["sessions", "used_tickets", "users", "activity"] {
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT COUNT(*) FROM {table}"
            )))
            .fetch_one(&db)
            .await
            .unwrap();
            assert_eq!(count, 0, "{table}");
        }
    }

    #[tokio::test]
    async fn a_ticket_makes_one_session() {
        let (db, keys, signer, core) = core().await;
        let ticket = signer.sign(&claims("owner", &core, NOW));
        sign_in(&db, &keys, &ticket, "Laptop", NOW).await.unwrap();
        let again = sign_in(&db, &keys, &ticket, "Laptop", NOW)
            .await
            .unwrap_err();
        assert_eq!(again.error, ErrorCode::TicketReused);
    }

    #[tokio::test]
    async fn device_names_are_capped() {
        let (db, keys, signer, core) = core().await;
        let ticket = signer.sign(&claims("owner", &core, NOW));
        let long = "x".repeat(65);
        let error = sign_in(&db, &keys, &ticket, &long, NOW).await.unwrap_err();
        assert_eq!(error.error, ErrorCode::InvalidRequest);
    }

    #[tokio::test]
    async fn sessions_end_after_30_idle_days_or_a_year() {
        let (db, keys, signer, core) = core().await;
        let ticket = signer.sign(&claims("owner", &core, NOW));
        let token = sign_in(&db, &keys, &ticket, "Laptop", NOW)
            .await
            .unwrap()
            .token;
        let last_used = || async {
            sqlx::query_scalar::<_, i64>("SELECT last_used_at FROM sessions")
                .fetch_one(&db)
                .await
                .unwrap()
        };

        // Within the hour nothing is written; after it, use extends the session.
        authenticate(&db, &token, NOW + 3599).await.unwrap();
        assert_eq!(last_used().await, NOW);
        authenticate(&db, &token, NOW + IDLE_S - 1).await.unwrap();
        assert_eq!(last_used().await, NOW + IDLE_S - 1);

        // Used every 29 days, it still ends a year after it was made.
        let mut at = NOW + IDLE_S - 1;
        while at + 29 * 86_400 < NOW + MAX_S {
            at += 29 * 86_400;
            authenticate(&db, &token, at).await.unwrap();
        }
        assert!(authenticate(&db, &token, NOW + MAX_S).await.is_err());

        // Unused for 30 days.
        let ticket = signer.sign(&claims("owner", &core, NOW));
        let idle = sign_in(&db, &keys, &ticket, "Phone", NOW)
            .await
            .unwrap()
            .token;
        assert!(authenticate(&db, &idle, NOW + IDLE_S).await.is_err());
        assert!(authenticate(&db, "not-a-token", NOW).await.is_err());
    }
}
