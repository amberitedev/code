use std::path::PathBuf;

use sqlx::SqlitePool;

/// Registered Java installation.
#[derive(Debug, Clone)]
pub struct JavaInstall {
    pub version: u32,
    pub path: PathBuf,
}

/// Find the best Java installation for a given Minecraft version.
/// Minecraft 1.17+ requires Java 17; 1.20.5+ requires Java 21.
pub fn required_java_version(game_version: &str) -> u32 {
    // Parse major.minor from versions like "1.21.4" or "1.17"
    let parts: Vec<u32> = game_version
        .split('.')
        .take(2)
        .filter_map(|s| s.parse().ok())
        .collect();
    let minor = parts.get(1).copied().unwrap_or(0);
    if minor >= 20 {
        21
    } else if minor >= 17 {
        17
    } else {
        8
    }
}

/// Detect Java installations on the system using `which`.
pub fn detect_java_installations() -> Vec<JavaInstall> {
    let mut installs = Vec::new();
    for (version, name) in [(21, "java21"), (17, "java17"), (8, "java")] {
        if let Ok(path) = which::which(name) {
            installs.push(JavaInstall { version, path });
        }
    }
    // Fallback: plain `java`
    if installs.is_empty() {
        if let Ok(path) = which::which("java") {
            installs.push(JavaInstall { version: 8, path });
        }
    }
    installs
}

/// Persist detected Java installations to DB on startup.
pub async fn sync_java_to_db(pool: &SqlitePool, installs: &[JavaInstall]) {
    for install in installs {
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO java_installations (version, path) VALUES (?, ?)"
        )
        .bind(install.version as i64)
        .bind(install.path.display().to_string())
        .execute(pool)
        .await;
    }
}

/// Look up a java binary path from DB.
pub async fn find_java(pool: &SqlitePool, version: u32) -> Option<PathBuf> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT path FROM java_installations WHERE version = ?"
    )
    .bind(version as i64)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    row.map(|(p,)| PathBuf::from(p))
}
