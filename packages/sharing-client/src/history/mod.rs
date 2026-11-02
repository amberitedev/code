//! A share's local history: every version its owner pushed, kept by the App or Core that pushed
//! it, so the owner can look back and restore one without the pool keeping old versions
//! (`docs/internals/pool.md`). The newest entry is also the last push, which Core compares a
//! server with to tell whether a push would change anything.
//!
//! An entry is the version's content, its manifest and what it changed over the version before
//! it. Entries are rows of `share_history` ([`SCHEMA`]) in the side's SQLite database. The files
//! an entry carries (configs and files outside Modrinth) are copied into the side's content store
//! and retained there under [`OWNER_KIND`] and [`owner_id`], so restoring reads them locally.
//! Each side implements [`HistoryStore`] over its store and database.

mod adapters;
mod model;

use std::fmt::Display;
use std::future::Future;
use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

pub use adapters::counts;
pub use model::{HistoryEntry, HistoryFile};

use crate::diff::SharedInstanceUpdateDiff;
use crate::push::Pushed;
use crate::versions::{ManifestFile, VersionContent};

/// The `share_history` table. The App and Core apply it after their own migrations; it only
/// creates what is missing.
pub const SCHEMA: &str = include_str!("schema.sql");
/// The content store's owner kind for the files of history entries.
pub const OWNER_KIND: &str = "share_history";
/// Entries kept per share until the owner picks another number. Placeholder, decide later.
pub const DEFAULT_KEPT: usize = 10;

/// The content store's owner of the files of `version` of `share_id`.
pub fn owner_id(share_id: &str, version: i32) -> String {
    format!("{share_id}:{version}")
}

/// Where a side keeps its history: entries in its database, files in its content store.
pub trait HistoryStore: Sync {
    type Error: Send + From<sqlx::Error>;

    /// The database with [`SCHEMA`] that holds the entries.
    fn db(&self) -> &SqlitePool;

    /// How many entries each share keeps, `None` for all of them.
    fn kept(
        &self,
    ) -> impl Future<Output = Result<Option<usize>, Self::Error>> + Send {
        async { Ok(Some(DEFAULT_KEPT)) }
    }

    /// Copies the file at `path` into the store and returns the key of the copy.
    fn store_file(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<String, Self::Error>> + Send;

    /// Where the copy with `blob` is, while the store has it intact.
    fn blob_path(
        &self,
        blob: &str,
    ) -> impl Future<Output = Result<Option<PathBuf>, Self::Error>> + Send;

    /// Keeps `blobs` through the store's cleanup for `owner` (within [`OWNER_KIND`]).
    fn retain(
        &self,
        owner: &str,
        blobs: &[String],
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Lets the store clean up what `owner` kept.
    fn release(
        &self,
        owner: &str,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

#[derive(Debug, thiserror::Error)]
pub enum HistoryError<E> {
    #[error("version {0} is not in this share's history")]
    MissingVersion(i32),
    #[error("the saved copy of {0} is gone")]
    MissingFile(String),
    #[error(transparent)]
    Store(E),
}

/// What restoring an entry writes back: its content, and each of its files with where its copy is.
#[derive(Debug, Clone)]
pub struct RestorePlan {
    pub entry: HistoryEntry,
    pub files: Vec<(ManifestFile, PathBuf)>,
}

/// Records `pushed`, a version of `share_id` with `content`, which changed `diffs` over the
/// version before it, then keeps the newest `kept` entries (all of them when `None`).
pub async fn record<S: HistoryStore>(
    store: &S,
    share_id: &str,
    content: &VersionContent,
    pushed: &Pushed,
    diffs: Vec<SharedInstanceUpdateDiff>,
    kept: Option<usize>,
) -> Result<HistoryEntry, S::Error> {
    let mut files = Vec::with_capacity(pushed.external_files.len());
    for (manifest, path) in pushed.external_files.iter().zip(&pushed.paths) {
        files.push(HistoryFile {
            manifest: manifest.clone(),
            blob: store.store_file(path).await?,
        });
    }
    let blobs = files
        .iter()
        .map(|file| file.blob.clone())
        .collect::<Vec<_>>();
    store
        .retain(&owner_id(share_id, pushed.version), &blobs)
        .await?;
    let entry = HistoryEntry {
        version: pushed.version,
        created: now_ms(),
        content: content.clone(),
        files,
        diffs,
    };
    adapters::save_entry(store.db(), share_id, &entry).await?;
    prune(store, share_id, kept).await?;
    Ok(entry)
}

/// Records `pushed`, a version of `share_id` with `content`, keeping as many entries as the
/// side keeps. `diffs` names what it changed over the version before it. The push already
/// succeeded, so a failure is logged and the version is only missing from the history.
pub async fn record_push<S: HistoryStore, E: Display>(
    store: &S,
    share_id: &str,
    content: &VersionContent,
    pushed: &Pushed,
    diffs: impl Future<Output = Result<Vec<SharedInstanceUpdateDiff>, E>>,
) where
    S::Error: Display,
{
    let diffs = diffs.await.unwrap_or_else(|error| {
        tracing::warn!(share_id, %error, "Could not name what a push changed");
        Vec::new()
    });
    let result = async {
        let kept = store.kept().await?;
        record(store, share_id, content, pushed, diffs, kept).await
    }
    .await;
    if let Err(error) = result {
        tracing::warn!(share_id, %error, "Could not record a push in the share's history");
    }
}

/// The entries of `share_id`, newest first.
pub async fn list<S: HistoryStore>(
    store: &S,
    share_id: &str,
) -> Result<Vec<HistoryEntry>, S::Error> {
    let mut entries = adapters::entries(store.db(), share_id).await?;
    entries.sort_by(|a, b| b.version.cmp(&a.version));
    Ok(entries)
}

/// The newest entry of `share_id`: its last push.
pub async fn latest<S: HistoryStore>(
    store: &S,
    share_id: &str,
) -> Result<Option<HistoryEntry>, S::Error> {
    Ok(list(store, share_id).await?.into_iter().next())
}

pub async fn get<S: HistoryStore>(
    store: &S,
    share_id: &str,
    version: i32,
) -> Result<Option<HistoryEntry>, S::Error> {
    Ok(adapters::entries(store.db(), share_id)
        .await?
        .into_iter()
        .find(|entry| entry.version == version))
}

/// What restoring `version` of `share_id` writes back, read from the store alone.
pub async fn restore_plan<S: HistoryStore>(
    store: &S,
    share_id: &str,
    version: i32,
) -> Result<RestorePlan, HistoryError<S::Error>> {
    let entry = get(store, share_id, version)
        .await
        .map_err(HistoryError::Store)?
        .ok_or(HistoryError::MissingVersion(version))?;
    let mut files = Vec::with_capacity(entry.files.len());
    for file in &entry.files {
        let path = store
            .blob_path(&file.blob)
            .await
            .map_err(HistoryError::Store)?
            .ok_or_else(|| {
                HistoryError::MissingFile(file.manifest.file_name.clone())
            })?;
        files.push((file.manifest.clone(), path));
    }
    Ok(RestorePlan { entry, files })
}

/// Deletes `version` of `share_id` and lets the store clean up its files.
pub async fn delete<S: HistoryStore>(
    store: &S,
    share_id: &str,
    version: i32,
) -> Result<(), S::Error> {
    adapters::delete_entry(store.db(), share_id, version).await?;
    store.release(&owner_id(share_id, version)).await
}

/// Keeps the newest `kept` entries of `share_id` (all of them when `None`) and deletes the rest.
pub async fn prune<S: HistoryStore>(
    store: &S,
    share_id: &str,
    kept: Option<usize>,
) -> Result<(), S::Error> {
    let Some(kept) = kept else {
        return Ok(());
    };
    for entry in list(store, share_id).await?.into_iter().skip(kept) {
        delete(store, share_id, entry.version).await?;
    }
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Mutex;

    use super::*;

    /// A store with blobs in memory, by the file's path, and entries in an in-memory database.
    struct Memory {
        db: SqlitePool,
        blobs: Mutex<BTreeSet<String>>,
        retained: Mutex<BTreeMap<String, Vec<String>>>,
    }

    impl Memory {
        async fn new() -> Self {
            // One connection, because every in-memory connection is its own database.
            let db = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap();
            sqlx::raw_sql(SCHEMA).execute(&db).await.unwrap();
            Self {
                db,
                blobs: Mutex::default(),
                retained: Mutex::default(),
            }
        }
    }

    impl HistoryStore for Memory {
        type Error = sqlx::Error;

        fn db(&self) -> &SqlitePool {
            &self.db
        }

        async fn store_file(&self, path: &Path) -> Result<String, Self::Error> {
            let blob = path.to_string_lossy().into_owned();
            self.blobs.lock().unwrap().insert(blob.clone());
            Ok(blob)
        }

        async fn blob_path(
            &self,
            blob: &str,
        ) -> Result<Option<PathBuf>, Self::Error> {
            Ok(self
                .blobs
                .lock()
                .unwrap()
                .contains(blob)
                .then(|| PathBuf::from(blob)))
        }

        async fn retain(
            &self,
            owner: &str,
            blobs: &[String],
        ) -> Result<(), Self::Error> {
            self.retained
                .lock()
                .unwrap()
                .insert(owner.to_owned(), blobs.to_vec());
            Ok(())
        }

        async fn release(&self, owner: &str) -> Result<(), Self::Error> {
            self.retained.lock().unwrap().remove(owner);
            Ok(())
        }
    }

    fn content() -> VersionContent {
        VersionContent {
            modrinth_ids: vec!["a".into()],
            modpack_id: None,
            removed_files: Vec::new(),
            game_version: "1.21.1".into(),
            loader: "fabric".into(),
            loader_version: "0.16.0".into(),
        }
    }

    fn pushed(version: i32, files: &[&str]) -> Pushed {
        Pushed {
            version,
            previous: None,
            external_files: files
                .iter()
                .map(|name| ManifestFile {
                    file_name: (*name).into(),
                    file_type: "config".into(),
                    file_size: 1,
                    sha1: "s".into(),
                    sha256: [1; 32],
                    object: [2; 32],
                    object_size: 17,
                    key: Some([3; 32]),
                })
                .collect(),
            paths: files
                .iter()
                .map(|name| PathBuf::from(format!("/source/{version}/{name}")))
                .collect(),
        }
    }

    #[tokio::test]
    async fn a_push_is_recorded_with_its_files_and_restored_from_them() {
        let store = Memory::new().await;
        record(
            &store,
            "s",
            &content(),
            &pushed(1, &["config/a.toml"]),
            Vec::new(),
            None,
        )
        .await
        .unwrap();
        record(
            &store,
            "s",
            &content(),
            &pushed(2, &["config/b.toml"]),
            Vec::new(),
            None,
        )
        .await
        .unwrap();

        let versions = list(&store, "s")
            .await
            .unwrap()
            .iter()
            .map(|entry| entry.version)
            .collect::<Vec<_>>();
        assert_eq!(versions, [2, 1]);
        assert_eq!(latest(&store, "s").await.unwrap().unwrap().version, 2);

        let plan = restore_plan(&store, "s", 1).await.unwrap();
        assert_eq!(plan.entry.content, content());
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].0.file_name, "config/a.toml");
        assert_eq!(plan.files[0].1, PathBuf::from("/source/1/config/a.toml"));
        assert!(matches!(
            restore_plan(&store, "s", 3).await,
            Err(HistoryError::MissingVersion(3))
        ));
    }

    #[tokio::test]
    async fn pruning_keeps_the_newest_and_releases_the_rest() {
        let store = Memory::new().await;
        for version in 1..=3 {
            record(
                &store,
                "s",
                &content(),
                &pushed(version, &["config/a.toml"]),
                Vec::new(),
                Some(2),
            )
            .await
            .unwrap();
        }

        let versions = list(&store, "s")
            .await
            .unwrap()
            .iter()
            .map(|entry| entry.version)
            .collect::<Vec<_>>();
        assert_eq!(versions, [3, 2]);
        let owners = store
            .retained
            .lock()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(owners, ["s:2", "s:3"]);
        assert_eq!(counts(&store.db).await.unwrap().get("s"), Some(&2));

        prune(&store, "s", Some(0)).await.unwrap();
        assert!(list(&store, "s").await.unwrap().is_empty());
        assert!(counts(&store.db).await.unwrap().is_empty());
        assert!(store.retained.lock().unwrap().is_empty());
    }

    #[test]
    fn an_entry_reads_back_as_it_is_saved() {
        let pushed = pushed(1, &["config/a.toml"]);
        let entry = HistoryEntry {
            version: 1,
            created: 5,
            content: content(),
            files: vec![HistoryFile {
                manifest: pushed.external_files[0].clone(),
                blob: "b".into(),
            }],
            diffs: Vec::new(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert_eq!(serde_json::from_str::<HistoryEntry>(&json).unwrap(), entry);
    }
}
