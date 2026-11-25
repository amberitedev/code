//! The store's ticket keys ([`sharing_client::keys`]), kept in `keys.json` since the store has no
//! database.

use std::path::PathBuf;

use contracts::tickets::PublicKeys;
use serde::{Deserialize, Serialize};
use sharing_client::keys::Persist;

pub type Keys = sharing_client::keys::Keys<JsonFile>;

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

/// The keys stored in `path`, a JSON file that need not exist yet.
pub fn load_file(
    path: PathBuf,
    http: reqwest::Client,
    backend_url: Option<&str>,
) -> Keys {
    let saved: Saved = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    Keys::new(
        JsonFile(path),
        &saved.keys,
        saved.fetched_at,
        http,
        backend_url,
    )
}

#[cfg(test)]
mod tests {
    use sharing_client::keys::test_support::Signer;

    use super::*;

    #[tokio::test]
    async fn file_keys_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let http = crate::http().build().unwrap();
        let signer = Signer::new("one");
        let ticket = signer.sign(&serde_json::json!({}));
        let keys = load_file(path.clone(), http.clone(), None);
        assert!(keys.verify(&ticket).await.is_err());
        keys.set(&signer.public_keys()).await;
        let again = load_file(path, http, None);
        assert!(again.verify(&ticket).await.is_ok());
    }
}
