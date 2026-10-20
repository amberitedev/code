//! The backend's public ticket keys. Stored in the database so tickets verify while the backend is
//! down, refreshed once a day, and refetched when a ticket names a key Core does not know, at most
//! once every ten minutes.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sqlx::SqlitePool;
use tokio::sync::Mutex;
use tracing::{debug, warn};

use super::now;
use crate::api::backend::PublicKeys;

const REFRESH_EVERY_S: i64 = 24 * 3600;
/// How long a failed daily refresh waits before trying again.
const RETRY: Duration = Duration::from_secs(3600);
const UNKNOWN_KID_EVERY: Duration = Duration::from_secs(600);

pub struct Keys {
    db: SqlitePool,
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
    /// When an unknown key id last made Core refetch.
    refetched: Option<Instant>,
}

impl Keys {
    /// The keys stored in the database.
    pub async fn load(
        db: SqlitePool,
        http: reqwest::Client,
        backend_url: Option<&str>,
    ) -> sqlx::Result<Self> {
        let (stored, fetched_at): (Option<String>, Option<i64>) =
            sqlx::query_as(
                "SELECT ticket_keys, ticket_keys_at FROM core WHERE id = 1",
            )
            .fetch_optional(&db)
            .await?
            .unwrap_or_default();
        let keys = stored
            .and_then(|json| serde_json::from_str(&json).ok())
            .map(|keys| parse(&keys))
            .unwrap_or_default();
        Ok(Self {
            db,
            http,
            url: backend_url.map(|url| format!("{url}/core/keys.json")),
            state: Mutex::new(State {
                keys,
                fetched_at: fetched_at.unwrap_or(0),
                refetched: None,
            }),
        })
    }

    /// The key named `kid`. An unknown id refetches the keys first, unless that happened in the
    /// last ten minutes.
    pub async fn get(&self, kid: &str) -> Option<[u8; 32]> {
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

    /// Refetch the keys whenever they are a day old, for as long as Core runs.
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
        self.store(&mut state, &keys)
            .await
            .map_err(|error| error.to_string())
    }

    async fn store(
        &self,
        state: &mut State,
        keys: &PublicKeys,
    ) -> sqlx::Result<()> {
        let fetched_at = now();
        sqlx::query(
            "UPDATE core SET ticket_keys = ?, ticket_keys_at = ? WHERE id = 1",
        )
        .bind(serde_json::to_string(keys).expect("keys serialize"))
        .bind(fetched_at)
        .execute(&self.db)
        .await?;
        state.keys = parse(keys);
        state.fetched_at = fetched_at;
        debug!(count = state.keys.len(), "stored the ticket keys");
        Ok(())
    }

    /// Use `keys` as if they were just fetched.
    #[cfg(test)]
    pub async fn set(&self, keys: &PublicKeys) {
        let mut state = self.state.lock().await;
        self.store(&mut state, keys).await.unwrap();
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{api::backend::PublicKey, db};

    #[tokio::test]
    async fn stored_keys_survive_a_restart() {
        let db = db::memory().await;
        db::core_id(&db).await.unwrap();
        let http = reqwest::Client::new();
        let keys = Keys::load(db.clone(), http.clone(), None).await.unwrap();
        keys.set(&PublicKeys {
            keys: vec![PublicKey {
                kty: "OKP".into(),
                crv: "Ed25519".into(),
                kid: "one".into(),
                x: URL_SAFE_NO_PAD.encode([7; 32]),
            }],
        })
        .await;
        let again = Keys::load(db, http, None).await.unwrap();
        assert_eq!(again.get("one").await, Some([7; 32]));
        assert_eq!(again.get("two").await, None);
    }
}
