use std::path::PathBuf;

use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::{domain::java::JavaInstall, ports::java_store::JavaStore};

/// SQLite-backed implementation of [`JavaStore`].
pub struct JavaRepo {
    pool: SqlitePool,
}

impl JavaRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl JavaStore for JavaRepo {
    async fn sync_all(&self, installs: &[JavaInstall]) {
        for install in installs {
            let _ = sqlx::query(
                "INSERT OR REPLACE INTO java_installations (version, path) VALUES (?, ?)",
            )
            .bind(install.version as i64)
            .bind(install.path.display().to_string())
            .execute(&self.pool)
            .await;
        }
    }

    async fn find_by_version(&self, version: u32) -> Option<PathBuf> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT path FROM java_installations WHERE version = ?")
                .bind(version as i64)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten();
        row.map(|(p,)| PathBuf::from(p))
    }

    async fn list_all(&self) -> Vec<JavaInstall> {
        let rows: Vec<(i64, String)> =
            sqlx::query_as("SELECT version, path FROM java_installations ORDER BY version DESC")
                .fetch_all(&self.pool)
                .await
                .unwrap_or_default();
        rows.into_iter()
            .map(|(v, p)| JavaInstall { version: v as u32, path: PathBuf::from(p) })
            .collect()
    }
}
