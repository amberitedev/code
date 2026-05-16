//! App settings — persisted to disk as JSON

use crate::error::{AmberiteError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

/// Persistent app settings stored in `{config_dir}/amberite/settings.json`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
	/// Base URL of the connected Amberite Core (e.g. "http://localhost:25585")
	pub core_url: Option<String>,
	/// Display name of the logged-in user
	pub display_name: Option<String>,
	/// Whether to auto-launch Core on app start
	pub auto_launch_core: bool,
}

impl Default for AppSettings {
	fn default() -> Self {
		Self {
			core_url: None,
			display_name: None,
			auto_launch_core: false,
		}
	}
}

impl AppSettings {
	fn settings_path() -> Result<PathBuf> {
		let config = dirs::config_dir()
			.ok_or_else(|| AmberiteError::Config("Cannot find config directory".into()))?;
		Ok(config.join("amberite").join("settings.json"))
	}

	/// Load settings from disk, returning defaults if the file doesn't exist
	pub async fn load() -> Result<Self> {
		let path = Self::settings_path()?;
		if !path.exists() {
			return Ok(Self::default());
		}
		let data = fs::read_to_string(&path).await?;
		let settings = serde_json::from_str(&data)?;
		Ok(settings)
	}

	/// Persist settings to disk
	pub async fn save(&self) -> Result<()> {
		let path = Self::settings_path()?;
		if let Some(parent) = path.parent() {
			fs::create_dir_all(parent).await?;
		}
		let data = serde_json::to_string_pretty(self)?;
		fs::write(&path, data).await?;
		Ok(())
	}
}
