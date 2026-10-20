//! Tickets: short Ed25519-signed JWTs the backend issues for one user and one Core. A ticket
//! makes a session (`sessions::sign_in`) or authorizes one dangerous action
//! (`access::fresh_ticket`), and is good for one use either way.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde::{Deserialize, de::DeserializeOwned};
use sqlx::SqliteConnection;

use super::keys::Keys;
use crate::api::{
    backend::TicketClaims,
    error::{ApiError, ErrorCode},
};

/// The `iss` of every ticket the backend signs.
pub const ISSUER: &str = "amberite";
/// How far apart Core's clock and the backend's may be.
pub const LEEWAY_S: i64 = 60;

#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: String,
}

fn invalid(description: &str) -> ApiError {
    ApiError::new(ErrorCode::TicketInvalid, description)
}

fn decode<T: DeserializeOwned>(part: &str) -> Option<T> {
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(part).ok()?).ok()
}

/// The claims of a ticket the backend signed for this Core, checked at `now`. It is not used up
/// until [`spend`].
pub async fn verify(
    keys: &Keys,
    ticket: &str,
    core_id: &str,
    now: i64,
) -> Result<TicketClaims, ApiError> {
    let mut parts = ticket.split('.');
    let (Some(header), Some(claims), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(invalid("That is not a ticket."));
    };
    let Header { alg, kid } =
        decode(header).ok_or_else(|| invalid("That is not a ticket."))?;
    // Pinned: a ticket names its algorithm, and only EdDSA is ever accepted.
    if alg != "EdDSA" {
        return Err(invalid("Tickets are signed with EdDSA only."));
    }
    let key = keys.get(&kid).await.ok_or_else(|| {
        invalid("The ticket is signed with a key this Core does not know.")
    })?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| invalid("The ticket's signature is unreadable."))?;
    UnparsedPublicKey::new(&ED25519, key)
        .verify(format!("{header}.{claims}").as_bytes(), &signature)
        .map_err(|_| invalid("The ticket's signature is wrong."))?;
    let claims: TicketClaims = decode(claims)
        .ok_or_else(|| invalid("The ticket's claims are unreadable."))?;

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

/// Signing tickets the way the backend does, for tests.
#[cfg(test)]
pub mod test_support {
    use ring::{
        rand::SystemRandom,
        signature::{Ed25519KeyPair, KeyPair},
    };

    use super::*;
    use crate::api::backend::{PublicKey, PublicKeys};

    pub struct Signer {
        pair: Ed25519KeyPair,
        pub kid: String,
    }

    impl Signer {
        pub fn new(kid: &str) -> Self {
            let pkcs8 =
                Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
            Self {
                pair: Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap(),
                kid: kid.into(),
            }
        }

        pub fn public_keys(&self) -> PublicKeys {
            PublicKeys {
                keys: vec![PublicKey {
                    kty: "OKP".into(),
                    crv: "Ed25519".into(),
                    kid: self.kid.clone(),
                    x: URL_SAFE_NO_PAD.encode(self.pair.public_key()),
                }],
            }
        }

        /// A ticket with `alg` in its header and these claims.
        pub fn sign_as(&self, alg: &str, claims: &TicketClaims) -> String {
            let encode = |value: serde_json::Value| {
                URL_SAFE_NO_PAD.encode(value.to_string())
            };
            let signed = format!(
                "{}.{}",
                encode(
                    serde_json::json!({ "alg": alg, "typ": "JWT", "kid": self.kid })
                ),
                encode(serde_json::to_value(claims).unwrap())
            );
            let signature = self.pair.sign(signed.as_bytes());
            format!("{signed}.{}", URL_SAFE_NO_PAD.encode(signature))
        }

        pub fn sign(&self, claims: &TicketClaims) -> String {
            self.sign_as("EdDSA", claims)
        }
    }

    /// Claims for `user` on `core`, issued at `iat`.
    pub fn claims(user: &str, core: &str, iat: i64) -> TicketClaims {
        TicketClaims {
            iss: ISSUER.into(),
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
        let keys = Keys::load(db, reqwest::Client::new(), None).await.unwrap();
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
    async fn only_eddsa_with_a_known_key_is_accepted() {
        let signer = Signer::new("k1");
        let keys = keys(&signer).await;
        let claims = claims("u1", "core", NOW);
        for alg in ["none", "HS256", "RS256", "eddsa"] {
            let ticket = signer.sign_as(alg, &claims);
            assert_eq!(
                error(&keys, &ticket, NOW).await,
                ErrorCode::TicketInvalid
            );
        }
        let unsigned = signer.sign(&claims);
        let unsigned = unsigned.rsplit_once('.').unwrap().0;
        assert_eq!(
            error(&keys, &format!("{unsigned}."), NOW).await,
            ErrorCode::TicketInvalid
        );
        let stranger = Signer::new("k1");
        assert_eq!(
            error(&keys, &stranger.sign(&claims), NOW).await,
            ErrorCode::TicketInvalid,
            "same key id, another key"
        );
        let unknown = Signer::new("k2");
        assert_eq!(
            error(&keys, &unknown.sign(&claims), NOW).await,
            ErrorCode::TicketInvalid
        );
    }

    #[tokio::test]
    async fn issuer_and_core_are_checked() {
        let signer = Signer::new("k1");
        let keys = keys(&signer).await;
        let mut other_issuer = claims("u1", "core", NOW);
        other_issuer.iss = "someone".into();
        assert_eq!(
            error(&keys, &signer.sign(&other_issuer), NOW).await,
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
