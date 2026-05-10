pub use crate::domain::java::JavaInstall;

/// Determine the required Java major version for a given Minecraft version.
/// Minecraft 1.17+ needs Java 17; 1.20.5+ needs Java 21.
pub fn required_java_version(game_version: &str) -> u32 {
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

/// Detect Java installations on the system by probing well-known binary names.
pub fn detect_java_installations() -> Vec<JavaInstall> {
    let mut installs = Vec::new();
    for (version, name) in [(21, "java21"), (17, "java17"), (8, "java")] {
        if let Ok(path) = which::which(name) {
            installs.push(JavaInstall { version, path });
        }
    }
    // Fallback: plain `java` if none of the versioned names exist.
    if installs.is_empty() {
        if let Ok(path) = which::which("java") {
            installs.push(JavaInstall { version: 8, path });
        }
    }
    installs
}
