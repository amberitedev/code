//! Tickets: short Ed25519-signed JWTs the backend issues for one user and one Core. A ticket
//! makes a session (`sessions::sign_in`) or authorizes one dangerous action
//! (`access::fresh_ticket`), and is good for one use either way.

use sqlx::SqliteConnection;

use sharing_storage::keys::{ISSUER, LEEWAY_S};

use super::keys::Keys;
use crate::api::{
    backend::TicketClaims,
    error::{ApiError, ErrorCode},
};

fn invalid(description: &str) -> ApiError {
    ApiError::new(ErrorCode::TicketInvalid, description)
}

/// The claims of a sign-in ticket the backend signed for this Core, checked at `now`. It is not
/// used up until [`spend`].
pub async fn verify(
    keys: &Keys,
    ticket: &str,
    core_id: &str,
    now: i64,
) -> Result<TicketClaims, ApiError> {
    let claims = keys.verify(ticket).await.map_err(invalid)?;
    // Pool tickets (the store's) are signed with the same keys and must never sign in.
    if claims.get("kind").and_then(serde_json::Value::as_str) != Some("session")
    {
        return Err(invalid("That ticket is not for signing in."));
    }
    let claims: TicketClaims = serde_json::from_value(claims)
        .map_err(|_| invalid("The ticket's claims are unreadable."))?;

    if claims.iss != ISSUER {
        return Err(invalid("The ticket is from another issuer."));
    }
    if claims.aud != core_id {
        return Err(ApiError::new(
            ErrorCode::TicketWrongCore,
            "The ticket is for another Core.",
        ));
    }
    // Issued in the future: only a clock that is behind explains it.
    if claims.iat > now + LEEWAY_S {
        return Err(ApiError::new(
            ErrorCode::ClockWrong,
            "The clock on this Core's machine is behind. Set it to the right time and try again.",
        ));
    }
    if now > claims.exp + LEEWAY_S {
        return Err(ApiError::new(
            ErrorCode::TicketExpired,
            "The ticket expired. Get a new one. If a new one fails the same way, the clock on this Core's machine is ahead.",
        ));
    }
    Ok(claims)
}

/// Use a verified ticket up by recording its one-time id. Call it in the transaction that does
/// what the ticket authorizes, so the use and the record are one write.
pub async fn spend(
    conn: &mut SqliteConnection,
    claims: &TicketClaims,
    now: i64,
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM used_tickets WHERE expires_at < ?")
        .bind(now - LEEWAY_S)
        .execute(&mut *conn)
        .await?;
    let inserted =
        sqlx::query("INSERT INTO used_tickets (jti, expires_at) VALUES (?, ?)")
            .bind(&claims.jti)
            .bind(claims.exp)
            .execute(&mut *conn)
            .await;
    match inserted {
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(ApiError::new(
                ErrorCode::TicketReused,
                "That ticket was already used. Get a new one.",
            ))
        }
        inserted => inserted.map(drop).map_err(Into::into),
    }
}

/// Signing sign-in tickets the way the backend does, for tests.
#[cfg(test)]
pub mod test_support {
    pub use sharing_storage::keys::test_support::Signer;

    use super::*;

    /// Claims for `user` on `core`, issued at `iat`.
    pub fn claims(user: &str, core: &str, iat: i64) -> TicketClaims {
        TicketClaims {
            iss: ISSUER.into(),
            kind: "session".into(),
            sub: user.into(),
            name: format!("{user} name"),
            avatar_url: None,
            aud: core.into(),
            iat,
            exp: iat + 300,
            jti: uuid::Uuid::new_v4().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{test_support::*, *};
    use crate::db;

    const NOW: i64 = 1_800_000_000;

    async fn keys(signer: &Signer) -> Keys {
        let db = db::memory().await;
        db::core_id(&db).await.unwrap();
        let keys = crate::access::keys::load(
            db,
            crate::network::http().build().unwrap(),
            None,
        )
        .await
        .unwrap();
        keys.set(&signer.public_keys()).await;
        keys
    }

    async fn error(keys: &Keys, ticket: &str, now: i64) -> ErrorCode {
        verify(keys, ticket, "core", now).await.unwrap_err().error
    }

    #[tokio::test]
    async fn a_good_ticket_verifies() {
        let signer = Signer::new("k1");
        let keys = keys(&signer).await;
        let ticket = signer.sign(&claims("u1", "core", NOW));
        let verified = verify(&keys, &ticket, "core", NOW + 10).await.unwrap();
        assert_eq!(verified.sub, "u1");
    }

    #[tokio::test]
    async fn issuer_kind_and_core_are_checked() {
        let signer = Signer::new("k1");
        let keys = keys(&signer).await;
        let mut other_issuer = claims("u1", "core", NOW);
        other_issuer.iss = "someone".into();
        assert_eq!(
            error(&keys, &signer.sign(&other_issuer), NOW).await,
            ErrorCode::TicketInvalid
        );
        let mut pool = claims("u1", "core", NOW);
        pool.kind = "fetch".into();
        assert_eq!(
            error(&keys, &signer.sign(&pool), NOW).await,
            ErrorCode::TicketInvalid,
            "a pool ticket never signs in"
        );
        let mut no_kind =
            serde_json::to_value(claims("u1", "core", NOW)).unwrap();
        no_kind.as_object_mut().unwrap().remove("kind");
        assert_eq!(
            error(&keys, &signer.sign(&no_kind), NOW).await,
            ErrorCode::TicketInvalid
        );
        let other_core = claims("u1", "another-core", NOW);
        assert_eq!(
            error(&keys, &signer.sign(&other_core), NOW).await,
            ErrorCode::TicketWrongCore
        );
    }

    #[tokio::test]
    async fn expiry_has_a_minute_of_leeway_and_names_a_wrong_clock() {
        let signer = Signer::new("k1");
        let keys = keys(&signer).await;
        let ticket = signer.sign(&claims("u1", "core", NOW));
        // Expires at NOW + 300.
        assert!(verify(&keys, &ticket, "core", NOW + 360).await.is_ok());
        assert_eq!(
            error(&keys, &ticket, NOW + 361).await,
            ErrorCode::TicketExpired
        );
        // Core's clock is behind the backend's.
        assert!(verify(&keys, &ticket, "core", NOW - 60).await.is_ok());
        assert_eq!(
            error(&keys, &ticket, NOW - 61).await,
            ErrorCode::ClockWrong
        );
    }

    #[tokio::test]
    async fn a_ticket_is_spent_once() {
        let db = db::memory().await;
        let claims = claims("u1", "core", NOW);
        let mut conn = db.acquire().await.unwrap();
        spend(&mut conn, &claims, NOW).await.unwrap();
        let again = spend(&mut conn, &claims, NOW).await.unwrap_err();
        assert_eq!(again.error, ErrorCode::TicketReused);
    }
}
