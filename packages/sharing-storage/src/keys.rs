//! The backend's public ticket keys, and the check every ticket starts with. Core's sign-in
//! (`access::tickets` in Copal) and the store's pool tickets ([`crate::tickets`]) both verify
//! with [`Keys::verify`] and differ only in the claims they then require.
//!
//! The keys are kept so tickets verify while the backend is down, refreshed once a day, and
//! refetched when a ticket names a key that is not known, at most once every ten minutes. Core
//! keeps them in its database; the store, which has none, in a file ([`Keys::load_file`]).

use std::{
    collections::HashMap,
    future::Future,
    path::PathBuf,
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::sync::Mutex;
use tracing::{debug, warn};
use ts_rs::TS;

/// The `iss` of every ticket the backend signs.
pub const ISSUER: &str = "amberite";
/// How far apart this machine's clock and the backend's may be.
pub const LEEWAY_S: i64 = 60;

const REFRESH_EVERY_S: i64 = 24 * 3600;
/// How long a failed daily refresh waits before trying again.
const RETRY: Duration = Duration::from_secs(3600);
const UNKNOWN_KID_EVERY: Duration = Duration::from_secs(600);

/// The backend's ticket keys as a JWK set. A key stays listed for a day after it is replaced.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct PublicKeys {
    pub keys: Vec<PublicKey>,
}

/// One Ed25519 public key as a JWK.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PublicKey {
    /// Always `OKP`.
    pub kty: String,
    /// Always `Ed25519`.
    pub crv: String,
    /// The id a ticket names in its `kid` header.
    pub kid: String,
    /// The key, base64url.
    pub x: String,
}

/// Where [`Keys`] keeps the key set between runs.
pub trait Persist: Send + Sync {
    /// Keep `keys`, fetched at `fetched_at` (unix seconds). The error is a description.
    fn save(
        &self,
        keys: &PublicKeys,
        fetched_at: i64,
    ) -> impl Future<Output = Result<(), String>> + Send;
}

/// The store's `keys.json`.
pub struct JsonFile(PathBuf);

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    fetched_at: i64,
    #[serde(flatten)]
    keys: PublicKeys,
}

impl Persist for JsonFile {
    async fn save(
        &self,
        keys: &PublicKeys,
        fetched_at: i64,
    ) -> Result<(), String> {
        let saved = Saved {
            fetched_at,
            keys: keys.clone(),
        };
        let json = serde_json::to_vec(&saved).expect("keys serialize");
        // Written aside and renamed, so a crash never leaves half a file.
        let part = self.0.with_extension("json.part");
        tokio::fs::write(&part, json)
            .await
            .map_err(|error| error.to_string())?;
        tokio::fs::rename(&part, &self.0)
            .await
            .map_err(|error| error.to_string())
    }
}

pub struct Keys<P> {
    persist: P,
    http: reqwest::Client,
    /// `{backend}/core/keys.json`. `None` without a backend: only stored keys are used.
    url: Option<String>,
    state: Mutex<State>,
}

struct State {
    /// Ed25519 public keys by key id.
    keys: HashMap<String, [u8; 32]>,
    /// Unix seconds; 0 before the first fetch.
    fetched_at: i64,
    /// When an unknown key id last caused a refetch.
    refetched: Option<Instant>,
}

impl Keys<JsonFile> {
    /// The keys stored in `path`, a JSON file that need not exist yet.
    pub fn load_file(
        path: PathBuf,
        http: reqwest::Client,
        backend_url: Option<&str>,
    ) -> Self {
        let saved: Saved = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self::new(
            JsonFile(path),
            &saved.keys,
            saved.fetched_at,
            http,
            backend_url,
        )
    }
}

impl<P: Persist> Keys<P> {
    /// Keys that start from `keys`, last fetched at `fetched_at`, and save through `persist`.
    pub fn new(
        persist: P,
        keys: &PublicKeys,
        fetched_at: i64,
        http: reqwest::Client,
        backend_url: Option<&str>,
    ) -> Self {
        Self {
            persist,
            http,
            url: backend_url.map(|url| format!("{url}/core/keys.json")),
            state: Mutex::new(State {
                keys: parse(keys),
                fetched_at,
                refetched: None,
            }),
        }
    }

    /// The claims of `ticket`, an EdDSA JWT, once its signature checks out against a known key.
    /// Nothing else about them is checked. The error is a description.
    pub async fn verify(
        &self,
        ticket: &str,
    ) -> Result<serde_json::Value, &'static str> {
        let unverified = Unverified::parse(ticket)?;
        let key = self.get(&unverified.kid).await.ok_or(
            "The ticket is signed with a key this Core does not know.",
        )?;
        unverified.verify(&key)
    }

    /// The key named `kid`. An unknown id refetches the keys first, unless that happened in the
    /// last ten minutes.
    async fn get(&self, kid: &str) -> Option<[u8; 32]> {
        let mut state = self.state.lock().await;
        if let Some(key) = state.keys.get(kid) {
            return Some(*key);
        }
        if state
            .refetched
            .is_some_and(|at| at.elapsed() < UNKNOWN_KID_EVERY)
        {
            return None;
        }
        state.refetched = Some(Instant::now());
        // Unlocked while the backend answers, so tickets with known keys never wait on it.
        drop(state);
        if let Err(error) = self.fetch().await {
            warn!(%error, "could not refetch the ticket keys");
        }
        self.state.lock().await.keys.get(kid).copied()
    }

    /// Refetch the keys whenever they are a day old, for as long as the process runs.
    pub async fn refresh_daily(&self) {
        if self.url.is_none() {
            return;
        }
        loop {
            let due =
                self.state.lock().await.fetched_at + REFRESH_EVERY_S - now();
            let wait = if due > 0 {
                Duration::from_secs(due as u64)
            } else {
                match self.fetch().await {
                    Ok(()) => Duration::from_secs(REFRESH_EVERY_S as u64),
                    Err(error) => {
                        warn!(%error, "could not refresh the ticket keys");
                        RETRY
                    }
                }
            };
            tokio::time::sleep(wait).await;
        }
    }

    /// Download the keys, then take the lock only to store them.
    async fn fetch(&self) -> Result<(), String> {
        let Some(url) = &self.url else {
            return Err("this Core has no backend".into());
        };
        let keys: PublicKeys = self
            .http
            .get(url)
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|error| error.to_string())?
            .json()
            .await
            .map_err(|error| error.to_string())?;
        let mut state = self.state.lock().await;
        self.store(&mut state, &keys).await
    }

    async fn store(
        &self,
        state: &mut State,
        keys: &PublicKeys,
    ) -> Result<(), String> {
        let fetched_at = now();
        self.persist.save(keys, fetched_at).await?;
        state.keys = parse(keys);
        state.fetched_at = fetched_at;
        debug!(count = state.keys.len(), "stored the ticket keys");
        Ok(())
    }

    /// Use `keys` as if they were just fetched.
    #[cfg(any(test, feature = "testing"))]
    pub async fn set(&self, keys: &PublicKeys) {
        let mut state = self.state.lock().await;
        self.store(&mut state, keys).await.unwrap();
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// The usable Ed25519 keys of a key set. Anything else in it is ignored.
fn parse(keys: &PublicKeys) -> HashMap<String, [u8; 32]> {
    keys.keys
        .iter()
        .filter(|key| key.kty == "OKP" && key.crv == "Ed25519")
        .filter_map(|key| {
            let bytes = URL_SAFE_NO_PAD.decode(&key.x).ok()?;
            Some((key.kid.clone(), bytes.try_into().ok()?))
        })
        .collect()
}

#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: String,
}

fn decode<T: DeserializeOwned>(part: &str) -> Option<T> {
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(part).ok()?).ok()
}

/// A ticket split into its parts, its signature not yet checked.
struct Unverified<'a> {
    /// The key id the header names.
    kid: String,
    signed: &'a str,
    claims: &'a str,
    signature: &'a str,
}

impl<'a> Unverified<'a> {
    /// Split `ticket`, refusing anything that is not an EdDSA JWT.
    fn parse(ticket: &'a str) -> Result<Self, &'static str> {
        let mut parts = ticket.split('.');
        let (Some(header), Some(claims), Some(signature), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err("That is not a ticket.");
        };
        let Header { alg, kid } =
            decode(header).ok_or("That is not a ticket.")?;
        // Pinned: a ticket names its algorithm, and only EdDSA is ever accepted.
        if alg != "EdDSA" {
            return Err("Tickets are signed with EdDSA only.");
        }
        Ok(Self {
            kid,
            signed: &ticket[..header.len() + 1 + claims.len()],
            claims,
            signature,
        })
    }

    /// The claims, once the signature checks out against `key`.
    fn verify(
        &self,
        key: &[u8; 32],
    ) -> Result<serde_json::Value, &'static str> {
        let signature = URL_SAFE_NO_PAD
            .decode(self.signature)
            .map_err(|_| "The ticket's signature is unreadable.")?;
        UnparsedPublicKey::new(&ED25519, key)
            .verify(self.signed.as_bytes(), &signature)
            .map_err(|_| "The ticket's signature is wrong.")?;
        decode(self.claims).ok_or("The ticket's claims are unreadable.")
    }
}

/// Signing tickets the way the backend does, for tests here and in Core.
#[cfg(any(test, feature = "testing"))]
pub mod test_support {
    use ring::{
        rand::SystemRandom,
        signature::{Ed25519KeyPair, KeyPair},
    };
    use serde::Serialize;

    use super::*;

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
        pub fn sign_as(&self, alg: &str, claims: &impl Serialize) -> String {
            let encode = |value: &serde_json::Value| {
                URL_SAFE_NO_PAD.encode(value.to_string())
            };
            let signed = format!(
                "{}.{}",
                encode(
                    &serde_json::json!({ "alg": alg, "typ": "JWT", "kid": self.kid })
                ),
                encode(&serde_json::to_value(claims).unwrap())
            );
            let signature = self.pair.sign(signed.as_bytes());
            format!("{signed}.{}", URL_SAFE_NO_PAD.encode(signature))
        }

        pub fn sign(&self, claims: &impl Serialize) -> String {
            self.sign_as("EdDSA", claims)
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{test_support::Signer, *};

    fn key(kid: &str) -> PublicKeys {
        PublicKeys {
            keys: vec![PublicKey {
                kty: "OKP".into(),
                crv: "Ed25519".into(),
                kid: kid.into(),
                x: URL_SAFE_NO_PAD.encode([7; 32]),
            }],
        }
    }

    #[tokio::test]
    async fn file_keys_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let http = crate::http().build().unwrap();
        let keys = Keys::load_file(path.clone(), http.clone(), None);
        assert_eq!(keys.get("one").await, None);
        keys.set(&key("one")).await;
        let again = Keys::load_file(path, http, None);
        assert_eq!(again.get("one").await, Some([7; 32]));
    }

    #[tokio::test]
    async fn only_eddsa_signed_by_a_known_key_verifies() {
        let dir = tempfile::tempdir().unwrap();
        let signer = Signer::new("k1");
        let keys = Keys::load_file(
            dir.path().join("keys.json"),
            crate::http().build().unwrap(),
            None,
        );
        keys.set(&signer.public_keys()).await;
        let claims = json!({ "sub": "u1" });
        assert_eq!(keys.verify(&signer.sign(&claims)).await.unwrap(), claims);
        for alg in ["none", "HS256", "RS256", "eddsa"] {
            assert!(keys.verify(&signer.sign_as(alg, &claims)).await.is_err());
        }
        let signed = signer.sign(&claims);
        let unsigned = signed.rsplit_once('.').unwrap().0;
        assert!(keys.verify(&format!("{unsigned}.")).await.is_err());
        let stranger = Signer::new("k1");
        assert!(
            keys.verify(&stranger.sign(&claims)).await.is_err(),
            "same key id, another key"
        );
        assert!(keys.verify(&Signer::new("k2").sign(&claims)).await.is_err());
    }
}
