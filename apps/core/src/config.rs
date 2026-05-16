use std::path::PathBuf;

/// Runtime configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory where all instance data is stored.
    pub data_dir: PathBuf,
    /// Convex deployment URL (also persisted after first-run pairing).
    pub convex_url: Option<String>,
    /// Public URL clients should use to reach this Core, if known.
    pub public_url: Option<String>,
    /// HTTP port for the Core API.
    pub port: u16,
    /// Host/IP the Core API binds to.
    pub bind_host: String,
    /// Allowed CORS origin.
    pub allowed_origin: String,
    /// When true, all auth checks are bypassed (dev mode only).
    pub dev_mode: bool,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        Self {
            data_dir: std::env::var("AMBERITE_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home_dir().join(".amberite")),
            convex_url: std::env::var("CONVEX_URL").ok(),
            public_url: std::env::var("AMBERITE_PUBLIC_URL").ok(),
            port: std::env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(16662),
            bind_host: std::env::var("AMBERITE_BIND_HOST")
                .unwrap_or_else(|_| "127.0.0.1".to_string()),
            allowed_origin: std::env::var("ALLOWED_ORIGIN")
                .unwrap_or_else(|_| "https://amberite.dev".to_string()),
            dev_mode: std::env::var("AMBERITE_DEV")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(cfg!(debug_assertions)),
        }
    }
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}
