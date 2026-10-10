use std::path::PathBuf;

use color_eyre::eyre::{eyre, Result, WrapErr};

/// Runtime configuration loaded from the active Core environment profile.
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory where all instance data is stored.
    pub data_dir: PathBuf,
    /// Backend that owns accounts; Core asks it who an account token belongs to.
    pub account_api_url: String,
    /// Public URL clients should use to reach this Core.
    pub public_url: String,
    /// HTTP port for the Core API.
    pub port: u16,
    /// Host/IP the Core API binds to.
    pub bind_host: String,
    /// Allowed CORS origin.
    pub allowed_origin: String,
    /// Debug build: relaxes CORS for local development.
    pub dev_mode: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        load_environment_profile()?;
        Ok(Self {
            data_dir: PathBuf::from(required_env("CORE_DATA_DIR")?),
            account_api_url: required_env("ACCOUNT_API_URL")?
                .trim_end_matches('/')
                .to_string(),
            public_url: required_env("AMBERITE_PUBLIC_URL")?,
            port: required_env("PORT")?
                .parse()
                .wrap_err("PORT must be a valid port number")?,
            bind_host: required_env("AMBERITE_BIND_HOST")?,
            allowed_origin: required_env("ALLOWED_ORIGIN")?,
            dev_mode: cfg!(debug_assertions),
        })
    }
}

fn required_env(name: &str) -> Result<String> {
    let value = std::env::var(name)
        .map_err(|_| eyre!("Missing required environment variable: {name}"))?;
    if value.trim().is_empty() {
        return Err(eyre!("Required environment variable is empty: {name}"));
    }
    Ok(value)
}

fn load_environment_profile() -> Result<()> {
    let filename = if cfg!(debug_assertions) {
        ".env.local"
    } else {
        ".env.prod"
    };
    let path = std::env::current_dir()
        .map_err(|error| {
            eyre!("Unable to resolve the Core working directory: {error}")
        })?
        .join(filename);
    if path.is_file() {
        dotenvy::from_path(path).map_err(|error| {
            eyre!("Unable to load the Core environment profile: {error}")
        })?;
    }
    Ok(())
}
