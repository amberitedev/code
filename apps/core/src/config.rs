use std::path::PathBuf;

use color_eyre::eyre::{eyre, Result, WrapErr};

/// Runtime configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory where all instance data is stored.
    pub data_dir: PathBuf,
    /// Convex deployment URL.
    pub convex_url: String,
    /// Public URL clients should use to reach this Core.
    pub public_url: String,
    /// HTTP port for the Core API.
    pub port: u16,
    /// Host/IP the Core API binds to.
    pub bind_host: String,
    /// Allowed CORS origin.
    pub allowed_origin: String,
    /// Enables local development conveniences that do not bypass route auth.
    pub dev_mode: bool,
    /// Number of sync snapshot archives retained per profile.
    pub sync_retain_count: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        load_local_env();
        dotenvy::dotenv().ok();
        let dev_mode = required_env("AMBERITE_DEV")?;
        let sync_retain_count: usize =
            required_env("AMBERITE_SYNC_RETAIN_COUNT")?
                .parse()
                .wrap_err(
                "AMBERITE_SYNC_RETAIN_COUNT must be a positive whole number",
            )?;
        if sync_retain_count == 0 {
            return Err(eyre!(
                "AMBERITE_SYNC_RETAIN_COUNT must be a positive whole number"
            ));
        }
        Ok(Self {
            data_dir: PathBuf::from(required_env("AMBERITE_DATA_DIR")?),
            convex_url: required_env("CONVEX_URL")?,
            public_url: required_env("AMBERITE_PUBLIC_URL")?,
            port: required_env("PORT")?
                .parse()
                .wrap_err("PORT must be a valid port number")?,
            bind_host: required_env("AMBERITE_BIND_HOST")?,
            allowed_origin: required_env("ALLOWED_ORIGIN")?,
            dev_mode: match dev_mode.as_str() {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => {
                    return Err(eyre!(
                        "AMBERITE_DEV must be true, false, 1, or 0"
                    ))
                }
            },
            sync_retain_count,
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

/// Load the workspace's development environment when Core runs from `apps/core`.
///
/// `dotenvy::dotenv` only looks for `.env`, while the desktop app and Convex use
/// the repository-level `.env.local` for `CONVEX_URL`.
fn load_local_env() {
    let Ok(current_dir) = std::env::current_dir() else {
        return;
    };

    for directory in current_dir.ancestors() {
        let path = directory.join(".env.local");
        if path.is_file() {
            dotenvy::from_path(path).ok();
            return;
        }
    }
}
