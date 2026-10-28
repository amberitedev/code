//! The pool's tickets. The store checks them itself, with the backend's keys, so it stays the
//! authority on what it serves. Each route takes one kind, and a ticket names one version:
//!
//! - `store`: upload `root`, at most `bytes` in all. `aud` is this Core.
//! - `fetch`: read `root`. `aud` lists the Cores holding it.
//! - `pull`: copy `root` from `source`, with the fetch ticket inside. `aud` is this Core.

use serde::Deserialize;

use super::{Code, StoreError, objects::Hash};
use crate::access::{
    keys::Keys,
    tickets::{ISSUER, LEEWAY_S, Unverified},
};

pub const STORE: &str = "store";
pub const FETCH: &str = "fetch";
pub const PULL: &str = "pull";

/// What a pool ticket grants, once verified.
#[derive(Debug, Deserialize)]
pub struct Claims {
    pub root: Hash,
    /// Store tickets: the most the version may hold.
    #[serde(default)]
    pub bytes: Option<u64>,
    /// Pull tickets: where to copy from.
    #[serde(default)]
    pub source: Option<Source>,
}

/// The Core a pull copies from, as the backend lists holders, and a fetch ticket for it. The
/// ticket may sit beside `core` or inside it.
#[derive(Debug, Clone, Deserialize)]
pub struct Source {
    pub core: SourceCore,
    #[serde(default)]
    ticket: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceCore {
    /// `https://ip:port`.
    pub address: String,
    #[serde(default)]
    pub public_address: Option<String>,
    /// SHA-256 of its certificate, hex. The only certificate the pull accepts.
    pub cert_fingerprint: String,
    #[serde(default)]
    ticket: Option<String>,
}

impl Source {
    pub fn ticket(&self) -> Option<&str> {
        self.ticket.as_deref().or(self.core.ticket.as_deref())
    }

    /// Where to reach it: its public address when it has one.
    pub fn address(&self) -> &str {
        self.core
            .public_address
            .as_deref()
            .unwrap_or(&self.core.address)
    }
}

#[derive(Deserialize)]
struct Common {
    iss: String,
    kind: String,
    aud: Audience,
    iat: i64,
    exp: i64,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}

impl Audience {
    fn contains(&self, core_id: &str) -> bool {
        match self {
            Self::One(one) => one == core_id,
            Self::Many(many) => many.iter().any(|id| id == core_id),
        }
    }
}

fn invalid(description: &str) -> StoreError {
    StoreError::new(Code::TicketInvalid, description)
}

/// The claims of `ticket`, required to be of `kind`, for this Core, for `root`, and current at
/// `now`.
pub async fn verify(
    keys: &Keys,
    ticket: &str,
    core_id: &str,
    kind: &'static str,
    root: &Hash,
    now: i64,
) -> Result<Claims, StoreError> {
    let unverified = Unverified::parse(ticket).map_err(invalid)?;
    let key = keys.get(&unverified.kid).await.ok_or_else(|| {
        invalid("The ticket is signed with a key this Core does not know.")
    })?;
    let value = unverified.verify(&key).map_err(invalid)?;
    let common: Common = serde_json::from_value(value.clone())
        .map_err(|_| invalid("The ticket's claims are unreadable."))?;
    if common.iss != ISSUER {
        return Err(invalid("The ticket is from another issuer."));
    }
    if common.kind != kind {
        return Err(StoreError::new(
            Code::TicketWrongKind,
            format!("This needs a {kind} ticket."),
        ));
    }
    if !common.aud.contains(core_id) {
        return Err(StoreError::new(
            Code::TicketWrongCore,
            "The ticket is for another Core.",
        ));
    }
    if common.iat > now + LEEWAY_S {
        return Err(invalid(
            "The ticket is from the future: the clock on this Core's machine is behind.",
        ));
    }
    if now > common.exp + LEEWAY_S {
        return Err(StoreError::new(
            Code::TicketExpired,
            "The ticket expired. Get a new one.",
        ));
    }
    let claims: Claims = serde_json::from_value(value)
        .map_err(|_| invalid("The ticket's claims are unreadable."))?;
    if claims.root != *root {
        return Err(invalid("The ticket is for another version."));
    }
    let complete = match kind {
        STORE => claims.bytes.is_some(),
        PULL => claims
            .source
            .as_ref()
            .is_some_and(|source| source.ticket().is_some()),
        _ => true,
    };
    if !complete {
        return Err(invalid("The ticket is missing what its kind needs."));
    }
    Ok(claims)
}

/// Pool tickets as the backend signs them, for tests.
#[cfg(test)]
pub mod test_support {
    use serde_json::{Value, json};

    use crate::access::tickets::{ISSUER, test_support::Signer};

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
            "iss": ISSUER,
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
        signer.sign_value("EdDSA", &claims)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{test_support::ticket, *};
    use crate::access::tickets::test_support::Signer;

    const NOW: i64 = 1_800_000_000;

    async fn keys(signer: &Signer) -> (tempfile::TempDir, Keys) {
        let dir = tempfile::tempdir().unwrap();
        let keys = Keys::load_file(
            dir.path().join("keys.json"),
            reqwest::Client::new(),
            None,
        );
        keys.set(&signer.public_keys()).await;
        (dir, keys)
    }

    fn root() -> Hash {
        Hash::parse(&"a".repeat(64)).unwrap()
    }

    async fn code(
        keys: &Keys,
        ticket: &str,
        kind: &'static str,
        now: i64,
    ) -> Code {
        verify(keys, ticket, "core", kind, &root(), now)
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
            STORE,
            json!("core"),
            root.as_str(),
            NOW,
            json!({ "bytes": 10 }),
        );
        let claims = verify(&keys, &store, "core", STORE, &root, NOW)
            .await
            .unwrap();
        assert_eq!(claims.bytes, Some(10));
        assert_eq!(
            code(&keys, &store, FETCH, NOW).await,
            Code::TicketWrongKind
        );
        let fetch = ticket(
            &signer,
            FETCH,
            json!(["other", "core"]),
            root.as_str(),
            NOW,
            json!({}),
        );
        assert!(
            verify(&keys, &fetch, "core", FETCH, &root, NOW)
                .await
                .is_ok()
        );
        assert_eq!(
            code(&keys, &fetch, STORE, NOW).await,
            Code::TicketWrongKind
        );
        let session = signer.sign(
            &crate::access::tickets::test_support::claims("u", "core", NOW),
        );
        assert_eq!(
            code(&keys, &session, FETCH, NOW).await,
            Code::TicketWrongKind
        );
    }

    #[tokio::test]
    async fn core_root_time_and_contents_are_checked() {
        let signer = Signer::new("k1");
        let (_dir, keys) = keys(&signer).await;
        let root = root().to_string();
        let other_core =
            ticket(&signer, FETCH, json!(["other"]), &root, NOW, json!({}));
        assert_eq!(
            code(&keys, &other_core, FETCH, NOW).await,
            Code::TicketWrongCore
        );
        let other_root = ticket(
            &signer,
            FETCH,
            json!("core"),
            &"b".repeat(64),
            NOW,
            json!({}),
        );
        assert_eq!(
            code(&keys, &other_root, FETCH, NOW).await,
            Code::TicketInvalid
        );
        let fetch =
            ticket(&signer, FETCH, json!("core"), &root, NOW, json!({}));
        assert!(
            verify(
                &keys,
                &fetch,
                "core",
                FETCH,
                &Hash::parse(&root).unwrap(),
                NOW + 3660
            )
            .await
            .is_ok()
        );
        assert_eq!(
            code(&keys, &fetch, FETCH, NOW + 3661).await,
            Code::TicketExpired
        );
        assert_eq!(
            code(&keys, &fetch, FETCH, NOW - 61).await,
            Code::TicketInvalid
        );
        let no_bytes =
            ticket(&signer, STORE, json!("core"), &root, NOW, json!({}));
        assert_eq!(
            code(&keys, &no_bytes, STORE, NOW).await,
            Code::TicketInvalid
        );
        let no_source_ticket = ticket(
            &signer,
            PULL,
            json!("core"),
            &root,
            NOW,
            json!({ "source": { "core": { "address": "https://203.0.113.1:1", "cert_fingerprint": "00" } } }),
        );
        assert_eq!(
            code(&keys, &no_source_ticket, PULL, NOW).await,
            Code::TicketInvalid
        );
        let stranger = Signer::new("k1");
        let forged =
            ticket(&stranger, FETCH, json!("core"), &root, NOW, json!({}));
        assert_eq!(code(&keys, &forged, FETCH, NOW).await, Code::TicketInvalid);
    }

    #[test]
    fn a_source_ticket_may_sit_beside_or_inside_the_core() {
        let beside: Source = serde_json::from_value(json!({
            "core": { "id": "c", "address": "https://10.0.0.1:1", "public_address": "https://203.0.113.1:1", "cert_fingerprint": "f" },
            "ticket": "t1",
        }))
        .unwrap();
        assert_eq!(beside.ticket(), Some("t1"));
        assert_eq!(beside.address(), "https://203.0.113.1:1");
        let inside: Source = serde_json::from_value(json!({
            "core": { "address": "https://10.0.0.1:1", "public_address": null, "cert_fingerprint": "f", "ticket": "t2" },
        }))
        .unwrap();
        assert_eq!(inside.ticket(), Some("t2"));
        assert_eq!(inside.address(), "https://10.0.0.1:1");
    }
}
