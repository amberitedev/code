//! Theseus directory information
pub(crate) mod move_app_directory;
use crate::event::EventSink;
use crate::event::emit::{emit_loading, init_loading};
use crate::state::Settings;
use crate::util::fetch::IoSemaphore;
use crate::{ErrorKind, LoadingBarType};
use anyhow::Context;
use std::path::{Path, PathBuf};
use tokio::fs;

pub const CACHES_FOLDER_NAME: &str = "caches";
pub const INSTANCES_FOLDER_NAME: &str = "profiles";
pub const METADATA_FOLDER_NAME: &str = "meta";
pub const SYNCED_OPTIONS_FOLDER_NAME: &str = "synced-options";
pub const STORE_FOLDER_NAME: &str = "store";

#[derive(Clone, Debug)]
pub struct DirectoryInfo {
    pub settings_dir: PathBuf, // Base settings directory- app database
    pub config_dir: PathBuf, // Base config directory- instances, minecraft downloads, etc. Changeable as a setting.
    instances_dir: PathBuf,
    logs_dir: Option<PathBuf>,
}

impl DirectoryInfo {
    /// Get all paths needed for Theseus to operate properly
    #[tracing::instrument]
    pub async fn init(
        db_path: &Path,
        config_dir: PathBuf,
        instances_dir: PathBuf,
        logs_dir: Option<PathBuf>,
    ) -> crate::Result<Self> {
        let settings_dir = db_path
            .parent()
            .ok_or(ErrorKind::FSError(
                "the database path has no folder".to_string(),
            ))?
            .to_path_buf();
        fs::create_dir_all(&settings_dir)
            .await
            .context(ErrorKind::FSError(
                "error creating Theseus config directory".to_string(),
            ))?;

        Ok(Self {
            settings_dir,
            config_dir,
            instances_dir,
            logs_dir,
        })
    }

    pub fn store_dir(&self) -> PathBuf {
        self.config_dir.join(STORE_FOLDER_NAME)
    }

    pub fn content_store_dir(&self) -> PathBuf {
        self.store_dir().join("content")
    }

    pub fn store_staging_dir(&self) -> PathBuf {
        self.store_dir().join("staging")
    }

    /// Get the Minecraft instance metadata directory
    #[inline]
    pub fn metadata_dir(&self) -> PathBuf {
        self.config_dir.join(METADATA_FOLDER_NAME)
    }

    pub fn install_backups_dir(&self) -> PathBuf {
        self.metadata_dir().join("install_job_backups")
    }

    /// Get the Minecraft java versions metadata directory
    #[inline]
    pub fn java_versions_dir(&self) -> PathBuf {
        self.metadata_dir().join("java_versions")
    }

    /// Get the Minecraft versions metadata directory
    #[inline]
    pub fn versions_dir(&self) -> PathBuf {
        self.metadata_dir().join("versions")
    }

    /// Get the metadata directory for a given version
    #[inline]
    pub fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir().join(version)
    }

    /// Get the Minecraft libraries metadata directory
    #[inline]
    pub fn libraries_dir(&self) -> PathBuf {
        self.metadata_dir().join("libraries")
    }

    /// Get the Minecraft assets metadata directory
    #[inline]
    pub fn assets_dir(&self) -> PathBuf {
        self.metadata_dir().join("assets")
    }

    /// Get the assets index directory
    #[inline]
    pub fn assets_index_dir(&self) -> PathBuf {
        self.assets_dir().join("indexes")
    }

    /// Get the assets objects directory
    #[inline]
    pub fn objects_dir(&self) -> PathBuf {
        self.assets_dir().join("objects")
    }

    /// Get the directory for a specific object
    #[inline]
    pub fn object_dir(&self, hash: &str) -> PathBuf {
        self.objects_dir().join(&hash[..2]).join(hash)
    }

    /// Get the Minecraft log config's directory
    #[inline]
    pub fn log_configs_dir(&self) -> PathBuf {
        self.metadata_dir().join("log_configs")
    }

    /// Get the Minecraft legacy assets metadata directory
    #[inline]
    pub fn legacy_assets_dir(&self) -> PathBuf {
        self.metadata_dir().join("resources")
    }

    /// Get the Minecraft legacy assets metadata directory
    #[inline]
    pub fn natives_dir(&self) -> PathBuf {
        self.metadata_dir().join("natives")
    }

    /// Get the natives directory for a version of Minecraft
    #[inline]
    pub fn version_natives_dir(&self, version: &str) -> PathBuf {
        self.natives_dir().join(version)
    }

    /// Get the directory containing instance icons
    #[inline]
    pub fn icon_dir(&self) -> PathBuf {
        self.config_dir.join("icons")
    }

    /// Get the instances directory
    #[inline]
    pub fn instances_dir(&self) -> PathBuf {
        self.instances_dir.clone()
    }

    #[inline]
    pub fn synced_options_dir(&self) -> PathBuf {
        self.config_dir.join(SYNCED_OPTIONS_FOLDER_NAME)
    }

    /// Gets the logs dir for a given instance path
    #[inline]
    pub fn instance_logs_dir(&self, instance_path: &str) -> PathBuf {
        self.instances_dir().join(instance_path).join("logs")
    }

    /// Gets the crash reports dir for a given instance path
    #[inline]
    pub fn crash_reports_dir(&self, instance_path: &str) -> PathBuf {
        self.instances_dir()
            .join(instance_path)
            .join("crash-reports")
    }

    /// None when the host keeps no launcher logs.
    #[inline]
    pub fn launcher_logs_dir(&self) -> Option<PathBuf> {
        self.logs_dir.clone()
    }

    /// Get the cache directory for Theseus
    #[inline]
    pub fn caches_dir(&self) -> PathBuf {
        self.config_dir.join(CACHES_FOLDER_NAME)
    }

    #[tracing::instrument(skip(settings, pool, _io_semaphore, events))]
    pub async fn move_launcher_directory(
        settings: &mut Settings,
        pool: &sqlx::SqlitePool,
        _io_semaphore: &IoSemaphore,
        settings_dir: &Path,
        events: &EventSink,
    ) -> crate::Result<()> {
        Self::try_move_launcher_directory(settings, pool, settings_dir, events)
            .await
            .context(ErrorKind::DirectoryMoveFailed)
    }

    async fn try_move_launcher_directory(
        settings: &mut Settings,
        pool: &sqlx::SqlitePool,
        initial: &Path,
        events: &EventSink,
    ) -> crate::Result<()> {
        let destination = settings
            .custom_dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| initial.to_path_buf());
        let previous = settings.prev_custom_dir.as_ref().map(PathBuf::from);
        let previous_root = match &previous {
            Some(previous) => Some(fs::canonicalize(previous).await?),
            None => None,
        };
        fs::create_dir_all(&destination).await?;
        let destination_root = fs::canonicalize(&destination).await?;
        let moving = previous_root
            .as_ref()
            .is_some_and(|root| root != &destination_root);
        let pending_move =
            super::content_store::setting(pool, "store_directory_move")
                .await?
                .filter(|checkpoint| !checkpoint.is_empty())
                .map(|checkpoint| {
                    serde_json::from_str::<(PathBuf, PathBuf)>(&checkpoint)
                })
                .transpose()?;
        let settings_root = fs::canonicalize(initial).await?;
        let mut locked_roots = std::collections::HashSet::from([settings_root]);
        let mut move_locks = Vec::new();
        let pending_roots = pending_move
            .as_ref()
            .into_iter()
            .flat_map(|(from, to)| [from, to]);
        for root in previous
            .iter()
            .chain(std::iter::once(&destination))
            .chain(pending_roots)
        {
            let root = fs::canonicalize(root).await?;
            if locked_roots.insert(root.clone()) {
                move_locks.push(
                    super::content_store::ContentStore::lock_process(&root)
                        .await?,
                );
            }
        }
        if let Some(previous) = &previous
            && moving
        {
            let loading = init_loading(
                events,
                LoadingBarType::DirectoryMove {
                    old: previous.to_string_lossy().into_owned(),
                    new: destination.to_string_lossy().into_owned(),
                },
                100.0,
                "Moving launcher directory",
            )
            .await?;
            move_app_directory::move_app_directory(
                previous,
                &destination,
                pool,
            )
            .await?;
            emit_loading(&loading, 100.0, None)?;
        }
        if !moving {
            move_app_directory::resume_completed_move(&destination, pool)
                .await?;
        }
        settings.custom_dir = Some(destination.to_string_lossy().into_owned());
        settings.prev_custom_dir.clone_from(&settings.custom_dir);
        settings.update(pool).await?;
        if let Some(previous) = &previous
            && moving
        {
            move_app_directory::finish_app_directory_move(
                previous,
                &destination,
                pool,
            )
            .await?;
        }
        drop(move_locks);
        Ok(())
    }
}
