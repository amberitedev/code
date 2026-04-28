use std::path::PathBuf;

/// Runtime configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory where all instance data is stored.
    pub data_dir: PathBuf,
    /// Supabase project URL (populated after first-run pairing).
    pub supabase_url: Option<String>,
    /// HTTP port for the Core API.
    pub port: u16,
    /// Allowed CORS origin.
    pub allowed_origin: String,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        Self {
            data_dir: std::env::var("AMBERITE_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home_dir().join(".amberite")),
            supabase_url: std::env::var("SUPABASE_URL").ok(),
            port: std::env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(16662),
            allowed_origin: std::env::var("ALLOWED_ORIGIN")
                .unwrap_or_else(|_| "https://amberite.dev".to_string()),
        }
    }
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}
