//! The history of the server-first shares this Core pushed, as Copal keeps it
//! (`sharing_client::history`): entries in Core's database, files retained in the content store
//! clone (`bedrock::upstream::store`). The App keeps its own the same way in app-lib's store.

use std::path::{Path, PathBuf};

use bedrock::upstream::store::{self, ContentStore};
use sharing_client::history::{HistoryStore, OWNER_KIND};
use sqlx::SqlitePool;

use crate::state::servers::Error;

/// The history over Core's database and content store.
pub struct History<'a> {
    pub store: &'a ContentStore,
    pub db: &'a SqlitePool,
}

fn store_error(error: bedrock::Error) -> Error {
    Error::Other(color_eyre::eyre::eyre!("{error:#}"))
}

impl HistoryStore for History<'_> {
    type Error = Error;

    fn db(&self) -> &SqlitePool {
        self.db
    }

    async fn store_file(&self, path: &Path) -> Result<String, Error> {
        let stored = store::store_file(self.store, path)
            .await
            .map_err(store_error)?;
        Ok(stored.sha512().to_owned())
    }

    async fn blob_path(&self, blob: &str) -> Result<Option<PathBuf>, Error> {
        Ok(store::lookup(self.store, blob)
            .await
            .map_err(store_error)?
            .map(|stored| stored.path().to_path_buf()))
    }

    async fn retain(&self, owner: &str, blobs: &[String]) -> Result<(), Error> {
        store::retain(self.store, OWNER_KIND, owner, blobs)
            .await
            .map_err(store_error)
    }

    async fn release(&self, owner: &str) -> Result<(), Error> {
        store::release(self.store, OWNER_KIND, owner)
            .await
            .map_err(store_error)
    }
}

#[cfg(test)]
mod tests {
    use sharing_client::{history, push::Pushed, versions::ManifestFile};

    use super::*;

    #[tokio::test]
    async fn a_push_is_kept_in_the_content_store_and_restored_from_it() {
        let db = crate::db::memory().await;
        let folder = tempfile::tempdir().unwrap();
        let dirs = store::DirectoryInfo {
            settings_dir: folder.path().join("content-store"),
            config_dir: folder.path().join("content-store"),
        };
        let content_store = store::open(&dirs, db.clone()).await.unwrap();
        let source = folder.path().join("a.toml");
        std::fs::write(&source, "x = 1").unwrap();
        let history = History {
            store: &content_store,
            db: &db,
        };
        let content = sharing_client::versions::VersionContent {
            modrinth_ids: Vec::new(),
            modpack_id: None,
            removed_files: Vec::new(),
            game_version: "1.21.1".into(),
            loader: "fabric".into(),
            loader_version: "0.16.0".into(),
        };
        let pushed = Pushed {
            version: 1,
            previous: None,
            external_files: vec![ManifestFile {
                file_name: "config/a.toml".into(),
                file_type: "config".into(),
                file_size: 5,
                sha1: "s".into(),
                sha256: [1; 32],
                object: [2; 32],
                object_size: 21,
                key: Some([3; 32]),
            }],
            paths: vec![source.clone()],
        };

        history::record(&history, "share", &content, &pushed, Vec::new(), None)
            .await
            .unwrap();
        std::fs::remove_file(&source).unwrap();

        let plan = history::restore_plan(&history, "share", 1).await.unwrap();
        assert_eq!(plan.entry.content, content);
        assert_eq!(std::fs::read_to_string(&plan.files[0].1).unwrap(), "x = 1");
        assert_eq!(
            history::latest(&history, "share")
                .await
                .unwrap()
                .unwrap()
                .version,
            1
        );

        // Cleanup keeps what the history retains, and removes it once the history lets go.
        let blob = plan.files[0].1.clone();
        sqlx::query("UPDATE store_blobs SET last_used_at = 0")
            .execute(&db)
            .await
            .unwrap();
        store::cleanup(&content_store, db.clone()).await.unwrap();
        assert!(blob.exists());
        history::prune(&history, "share", Some(0)).await.unwrap();
        store::cleanup(&content_store, db.clone()).await.unwrap();
        assert!(!blob.exists());
    }
}
