use std::path::PathBuf;

/// A Java installation detected on the host system.
#[derive(Debug, Clone)]
pub struct JavaInstall {
    /// Java major version (8, 17, 21, …).
    pub version: u32,
    /// Absolute path to the `java` binary.
    pub path: PathBuf,
}
