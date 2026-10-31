//! Core's settings. Every setting has a default, so Core starts with no configuration.
//!
//! Each setting is read from, last one winning: its default, the optional `config.toml` in the
//! data folder, then an environment variable. The data folder itself can only come from the
//! environment, because the file lives inside it.

use std::{
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
};

use color_eyre::eyre::{Result, WrapErr};
use serde::Deserialize;

use super::paths::Paths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `CORE_DATA_DIR`. See `default_data_dir`.
    pub data_dir: PathBuf,
    /// `PORT`. The HTTPS API port.
    pub port: u16,
    /// `BIND_HOST`. Every interface by default: the API is reachable from the internet.
    pub bind: IpAddr,
    /// `ACCOUNT_API_URL`. The Amberite backend. `None` until the hosted backend has an address;
    /// a Core without one serves its API but cannot pair or sync.
    pub backend_url: Option<String>,
    /// `COPAL_UPDATE_URL`. The list of Copal releases, in the shape of GitHub's releases API.
    /// Releases are trusted by their signature, not by where they come from.
    pub update_url: String,
    /// `COPAL_FORWARD_PORTS`. Ask the router to forward Core's port and each running server's.
    /// On by default; off leaves the router alone.
    pub forward_ports: bool,
}

/// `config.toml`. Every field is optional.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    port: Option<u16>,
    bind: Option<IpAddr>,
    backend_url: Option<String>,
    update_url: Option<String>,
    forward_ports: Option<bool>,
}

impl Config {
    /// The settings for this process: defaults, then the file, then the environment.
    pub fn load() -> Result<Self> {
        let env = |name: &str| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        };
        let data_dir = data_dir();
        let file = read_file(&Paths::new(&data_dir).config_file())?;
        let mut config = Self::with_file(data_dir, file);
        if let Some(port) = env("PORT") {
            config.port =
                port.parse().wrap_err("PORT must be a port number")?;
        }
        if let Some(bind) = env("BIND_HOST") {
            config.bind =
                bind.parse().wrap_err("BIND_HOST must be an IP address")?;
        }
        if let Some(url) = env("ACCOUNT_API_URL") {
            config.backend_url = Some(url);
        }
        if let Some(url) = env("COPAL_UPDATE_URL") {
            config.update_url = url;
        }
        if let Some(forward) = env("COPAL_FORWARD_PORTS") {
            config.forward_ports = !matches!(
                forward.to_lowercase().as_str(),
                "0" | "false" | "off" | "no"
            );
        }
        config.backend_url = config
            .backend_url
            .map(|url| url.trim_end_matches('/').to_owned());
        Ok(config)
    }

    fn with_file(data_dir: PathBuf, file: File) -> Self {
        Self {
            data_dir,
            port: file.port.unwrap_or(16662),
            bind: file.bind.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
            backend_url: file.backend_url,
            update_url: file
                .update_url
                .unwrap_or_else(|| crate::update::RELEASES.into()),
            forward_ports: file.forward_ports.unwrap_or(true),
        }
    }
}

fn read_file(path: &Path) -> Result<File> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text)
            .wrap_err_with(|| format!("{} is not valid", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(File::default())
        }
        Err(error) => Err(error)
            .wrap_err_with(|| format!("could not read {}", path.display())),
    }
}

/// The data folder alone, without reading `config.toml`. The CLI uses this: the owner may
/// reach the local socket but not read the data folder's files.
pub fn data_dir() -> PathBuf {
    std::env::var("CORE_DATA_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map_or_else(default_data_dir, PathBuf::from)
}

/// Where a system service keeps its data on each OS. `copal install` gives this folder to the
/// `copal` account.
fn default_data_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("ProgramData")
            .map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from)
            .join("Copal")
    } else if cfg!(target_os = "macos") {
        PathBuf::from("/Library/Application Support/Copal")
    } else {
        PathBuf::from("/var/lib/copal")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_file_means_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = read_file(&Paths::new(dir.path()).config_file()).unwrap();
        let config = Config::with_file(dir.path().into(), file);
        assert_eq!(
            config,
            Config {
                data_dir: dir.path().into(),
                port: 16662,
                bind: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                backend_url: None,
                update_url: crate::update::RELEASES.into(),
                forward_ports: true,
            }
        );
    }

    #[test]
    fn file_overrides_defaults_it_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = Paths::new(dir.path()).config_file();
        std::fs::write(&path, "port = 4000\n").unwrap();
        let config =
            Config::with_file(dir.path().into(), read_file(&path).unwrap());
        assert_eq!(config.port, 4000);
        assert_eq!(config.bind, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    }

    #[test]
    fn unknown_settings_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = Paths::new(dir.path()).config_file();
        std::fs::write(&path, "prot = 4000\n").unwrap();
        assert!(read_file(&path).is_err());
    }
}
