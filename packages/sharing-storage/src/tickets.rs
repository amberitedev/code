//! The pool's tickets ([`contracts::tickets`]). The store checks them itself, with the backend's
//! keys, so it stays the authority on what it serves. Each route takes one kind, and a ticket
//! names one version.

use contracts::{
    ApiError, ErrorCode, Hash,
    tickets::{Audience, TICKET_ISSUER, Ticket, TicketKind},
};
use serde::Deserialize;

use crate::keys::{JsonFile, Keys, LEEWAY_S};

/// What every pool ticket is checked for before its kind's own claims are read.
#[derive(Deserialize)]
struct Common {
    iss: String,
    kind: TicketKind,
    aud: Audience,
    iat: i64,
    exp: i64,
    #[serde(default)]
    root: Option<Hash>,
}

fn invalid(description: &str) -> ApiError {
    ApiError::new(ErrorCode::TicketInvalid, description)
}

/// The claims of `ticket`, required to be a `T`, for this Core, for `root`, and current at `now`.
pub async fn verify<T: Ticket>(
    keys: &Keys<JsonFile>,
    ticket: &str,
    core_id: &str,
    root: &Hash,
    now: i64,
) -> Result<T, ApiError> {
    let value = keys.verify(ticket).await.map_err(invalid)?;
    let common: Common = serde_json::from_value(value.clone())
        .map_err(|_| invalid("The ticket's claims are unreadable."))?;
    if common.iss != TICKET_ISSUER {
        return Err(invalid("The ticket is from another issuer."));
    }
    if common.kind != T::KIND {
        return Err(ApiError::new(
            ErrorCode::TicketWrongKind,
            format!(
                "This needs a {} ticket.",
                format!("{:?}", T::KIND).to_lowercase()
            ),
        ));
    }
    let for_core = match &common.aud {
        Audience::One(one) => one == core_id,
        Audience::Many(many) => many.iter().any(|id| id == core_id),
    };
    if !for_core {
        return Err(ApiError::new(
            ErrorCode::TicketWrongCore,
            "The ticket is for another Core.",
        ));
    }
    if common.iat > now + LEEWAY_S {
        return Err(invalid(
            "The ticket is from the future: the clock on this Core's machine is behind.",
        ));
    }
    if now > common.exp + LEEWAY_S {
        return Err(ApiError::new(
            ErrorCode::TicketExpired,
            "The ticket expired. Get a new one.",
        ));
    }
    if common.root.as_ref() != Some(root) {
        return Err(invalid("The ticket is for another version."));
    }
    serde_json::from_value(value)
        .map_err(|_| invalid("The ticket is missing what its kind needs."))
}

/// Pool tickets as the backend signs them, for tests.
#[cfg(test)]
pub mod test_support {
    use contracts::tickets::TICKET_ISSUER;
    use serde_json::{Value, json};

    use crate::keys::test_support::Signer;

    /// A ticket of `kind` for `root` on `aud`, issued at `iat` and good for an hour, with
    /// `extra` claims.
    pub fn ticket(
        signer: &Signer,
        kind: &str,
        aud: Value,
        root: &str,
        iat: i64,
        extra: Value,
    ) -> String {
        let mut claims = json!({
            "iss": TICKET_ISSUER,
            "kind": kind,
            "aud": aud,
            "root": root,
            "iat": iat,
            "exp": iat + 3600,
            "jti": uuid::Uuid::new_v4().to_string(),
        });
        if let (Some(claims), Value::Object(extra)) =
            (claims.as_object_mut(), extra)
        {
            claims.extend(extra);
        }
        signer.sign(&claims)
    }
}

#[cfg(test)]
mod tests {
    use contracts::tickets::{FetchTicket, PullTicket, StoreTicket};
    use serde_json::json;

    use super::{test_support::ticket, *};
    use crate::keys::test_support::Signer;

    const NOW: i64 = 1_800_000_000;

    async fn keys(signer: &Signer) -> (tempfile::TempDir, Keys<JsonFile>) {
        let dir = tempfile::tempdir().unwrap();
        let keys = Keys::load_file(
            dir.path().join("keys.json"),
            crate::http().build().unwrap(),
            None,
        );
        keys.set(&signer.public_keys()).await;
        (dir, keys)
    }

    fn root() -> Hash {
        Hash::parse(&"a".repeat(64)).unwrap()
    }

    async fn code<T: Ticket + std::fmt::Debug>(
        keys: &Keys<JsonFile>,
        ticket: &str,
        now: i64,
    ) -> ErrorCode {
        verify::<T>(keys, ticket, "core", &root(), now)
            .await
            .unwrap_err()
            .error
    }

    #[tokio::test]
    async fn each_kind_verifies_only_on_its_routes() {
        let signer = Signer::new("k1");
        let (_dir, keys) = keys(&signer).await;
        let root = root();
        let store = ticket(
            &signer,
            "store",
            json!("core"),
            &root.to_string(),
            NOW,
            json!({ "bytes": 10 }),
        );
        let claims = verify::<StoreTicket>(&keys, &store, "core", &root, NOW)
            .await
            .unwrap();
        assert_eq!(claims.bytes, 10);
        assert_eq!(
            code::<FetchTicket>(&keys, &store, NOW).await,
            ErrorCode::TicketWrongKind
        );
        let fetch = ticket(
            &signer,
            "fetch",
            json!(["other", "core"]),
            &root.to_string(),
            NOW,
            json!({}),
        );
        assert!(
            verify::<FetchTicket>(&keys, &fetch, "core", &root, NOW)
                .await
                .is_ok()
        );
        assert_eq!(
            code::<StoreTicket>(&keys, &fetch, NOW).await,
            ErrorCode::TicketWrongKind
        );
        let session = signer.sign(&json!({
            "iss": TICKET_ISSUER, "kind": "session", "sub": "u", "aud": "core",
            "iat": NOW, "exp": NOW + 300, "jti": "j",
        }));
        assert_eq!(
            code::<FetchTicket>(&keys, &session, NOW).await,
            ErrorCode::TicketWrongKind
        );
    }

    #[tokio::test]
    async fn core_root_time_and_contents_are_checked() {
        let signer = Signer::new("k1");
        let (_dir, keys) = keys(&signer).await;
        let root = root().to_string();
        let other_core =
            ticket(&signer, "fetch", json!(["other"]), &root, NOW, json!({}));
        assert_eq!(
            code::<FetchTicket>(&keys, &other_core, NOW).await,
            ErrorCode::TicketWrongCore
        );
        let other_root = ticket(
            &signer,
            "fetch",
            json!("core"),
            &"b".repeat(64),
            NOW,
            json!({}),
        );
        assert_eq!(
            code::<FetchTicket>(&keys, &other_root, NOW).await,
            ErrorCode::TicketInvalid
        );
        let fetch =
            ticket(&signer, "fetch", json!("core"), &root, NOW, json!({}));
        assert!(
            verify::<FetchTicket>(
                &keys,
                &fetch,
                "core",
                &Hash::parse(&root).unwrap(),
                NOW + 3660
            )
            .await
            .is_ok()
        );
        assert_eq!(
            code::<FetchTicket>(&keys, &fetch, NOW + 3661).await,
            ErrorCode::TicketExpired
        );
        assert_eq!(
            code::<FetchTicket>(&keys, &fetch, NOW - 61).await,
            ErrorCode::TicketInvalid
        );
        let no_bytes =
            ticket(&signer, "store", json!("core"), &root, NOW, json!({}));
        assert_eq!(
            code::<StoreTicket>(&keys, &no_bytes, NOW).await,
            ErrorCode::TicketInvalid
        );
        let no_source_ticket = ticket(
            &signer,
            "pull",
            json!("core"),
            &root,
            NOW,
            json!({ "source": { "core": { "address": "https://203.0.113.1:1", "cert_fingerprint": "00" } } }),
        );
        assert_eq!(
            code::<PullTicket>(&keys, &no_source_ticket, NOW).await,
            ErrorCode::TicketInvalid
        );
        let stranger = Signer::new("k1");
        let forged =
            ticket(&stranger, "fetch", json!("core"), &root, NOW, json!({}));
        assert_eq!(
            code::<FetchTicket>(&keys, &forged, NOW).await,
            ErrorCode::TicketInvalid
        );
    }
}
