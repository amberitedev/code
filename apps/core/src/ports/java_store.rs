use std::path::PathBuf;

use async_trait::async_trait;

use crate::domain::java::JavaInstall;

/// Port for persisting and querying Java installations.
#[async_trait]
pub trait JavaStore: Send + Sync + 'static {
    /// Upsert all detected Java installations into the DB.
    async fn sync_all(&self, installs: &[JavaInstall]);

    /// Find a Java binary path by major version number.
    async fn find_by_version(&self, version: u32) -> Option<PathBuf>;

    /// List all known Java installations.
    async fn list_all(&self) -> Vec<JavaInstall>;
}
