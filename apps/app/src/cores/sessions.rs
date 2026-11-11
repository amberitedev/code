//! This device's Core sessions, in the OS keychain: one entry per account and Core. Only the
//! proxy reads them; no command returns a token, so the WebView never sees one.
//!
//! Entries are per account as well as per Core because the App can switch accounts, and because
//! dev scenarios share one OS keychain.

use std::{collections::HashMap, sync::Mutex};

use keyring::Entry;

const SERVICE: &str = "dev.amberite.app";

/// Whose session: the Amberite account and the Core id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key {
    pub account: String,
    pub core_id: String,
}

impl Key {
    fn entry(&self) -> keyring::Result<Entry> {
        Entry::new(
            SERVICE,
            &format!("core-session:{}:{}", self.account, self.core_id),
        )
    }
}

/// Session tokens, read from the keychain once and kept in memory after that. A keychain that
/// cannot be read or written leaves the token in memory only: the App signs in again with a new
/// ticket after a restart, which is all a lost token costs.
#[derive(Default)]
pub struct Sessions(Mutex<HashMap<Key, Option<String>>>);

impl Sessions {
    pub async fn token(&self, key: &Key) -> Option<String> {
        if let Some(token) = self.0.lock().expect("not poisoned").get(key) {
            return token.clone();
        }
        let entry = key.clone();
        let stored = tokio::task::spawn_blocking(move || {
            entry.entry().and_then(|entry| entry.get_password())
        })
        .await
        .ok()
        .and_then(|read| match read {
            Ok(token) => Some(token),
            Err(keyring::Error::NoEntry) => None,
            Err(error) => {
                tracing::warn!(%error, "could not read a Core session from the keychain");
                None
            }
        });
        self.0
            .lock()
            .expect("not poisoned")
            .entry(key.clone())
            .or_insert(stored)
            .clone()
    }

    pub async fn store(&self, key: &Key, token: String) {
        self.0
            .lock()
            .expect("not poisoned")
            .insert(key.clone(), Some(token.clone()));
        let entry = key.clone();
        let written = tokio::task::spawn_blocking(move || {
            entry.entry().and_then(|entry| entry.set_password(&token))
        })
        .await;
        if let Ok(Err(error)) = written {
            tracing::warn!(%error, "could not keep a Core session in the keychain");
        }
    }

    pub async fn forget(&self, key: &Key) {
        self.0
            .lock()
            .expect("not poisoned")
            .insert(key.clone(), None);
        let entry = key.clone();
        let deleted = tokio::task::spawn_blocking(move || {
            entry.entry().and_then(|entry| entry.delete_credential())
        })
        .await;
        if let Ok(Err(error)) = deleted
            && !matches!(error, keyring::Error::NoEntry)
        {
            tracing::warn!(%error, "could not delete a Core session from the keychain");
        }
    }

    /// A session held in memory only, for tests.
    #[cfg(test)]
    pub fn cached(key: &Key, token: Option<&str>) -> Self {
        Self(Mutex::new(HashMap::from([(
            key.clone(),
            token.map(str::to_owned),
        )])))
    }
}
