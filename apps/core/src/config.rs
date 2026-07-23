use std::path::PathBuf;

use color_eyre::eyre::{eyre, Result, WrapErr};

/// Runtime configuration loaded from the active Core environment profile.
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory where all instance data is stored.
    pub data_dir: PathBuf,
    /// Convex deployment URL.
    pub convex_url: String,
    /// Convex HTTP actions URL.
    pub convex_site_url: String,
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
    /// Disables HTTP auth and permission checks for local dashboard development.
    pub no_auth: bool,
    /// Number of sync snapshot archives retained per profile.
    pub sync_retain_count: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Self::from_env_with_no_auth(false)
    }

    pub fn from_env_with_no_auth(no_auth_override: bool) -> Result<Self> {
        load_environment_profile()?;
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
        let dev_mode = cfg!(debug_assertions);
        let no_auth =
            no_auth_override || optional_bool_env("AMBERITE_NO_AUTH")?;
        if no_auth && !dev_mode {
            return Err(eyre!(
                "No-auth Core mode is only available in debug builds"
            ));
        }
        Ok(Self {
            data_dir: PathBuf::from(required_env("CORE_DATA_DIR")?),
            convex_url: convex_deployment_url(
                &required_env("CONVEX_URL")?,
                dev_mode,
            )?,
            convex_site_url: convex_site_url(
                &required_env("CONVEX_SITE_URL")?,
                dev_mode,
            )?,
            public_url: required_env("AMBERITE_PUBLIC_URL")?,
            port: required_env("PORT")?
                .parse()
                .wrap_err("PORT must be a valid port number")?,
            bind_host: required_env("AMBERITE_BIND_HOST")?,
            allowed_origin: required_env("ALLOWED_ORIGIN")?,
            dev_mode,
            no_auth,
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

fn optional_bool_env(name: &str) -> Result<bool> {
    let value = match std::env::var(name) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(false),
        Err(error) => {
            return Err(eyre!("Unable to read {name}: {error}"));
        }
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(eyre!(
            "{name} must be one of true/false, yes/no, on/off, or 1/0"
        )),
    }
}

fn convex_deployment_url(
    value: &str,
    allow_local_http: bool,
) -> Result<String> {
    let mut url = url::Url::parse(value.trim())
        .wrap_err("CONVEX_URL must be a valid URL")?;
    if !valid_convex_scheme(&url, allow_local_http) {
        return Err(eyre!(
            "CONVEX_URL must use HTTPS or local HTTP in debug builds"
        ));
    }
    if url.host_str() == Some("test.convex.cloud") {
        return Err(eyre!(
            "CONVEX_URL points at the placeholder https://test.convex.cloud"
        ));
    }
    let Some(host) = url.host_str().map(ToOwned::to_owned) else {
        return Err(eyre!("CONVEX_URL must include a host"));
    };
    if host.ends_with(".convex.site") {
        let cloud_host = host
            .strip_suffix(".convex.site")
            .expect("host suffix was checked");
        url.set_host(Some(&format!("{cloud_host}.convex.cloud")))
            .map_err(|_| eyre!("CONVEX_URL must include a valid host"))?;
    }
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn convex_site_url(value: &str, allow_local_http: bool) -> Result<String> {
    let mut url = url::Url::parse(value.trim())
        .wrap_err("CONVEX_SITE_URL must be a valid URL")?;
    if !valid_convex_scheme(&url, allow_local_http) {
        return Err(eyre!(
            "CONVEX_SITE_URL must use HTTPS or local HTTP in debug builds"
        ));
    }
    if url.host_str().is_none() {
        return Err(eyre!("CONVEX_SITE_URL must include a host"));
    }
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn valid_convex_scheme(url: &url::Url, allow_local_http: bool) -> bool {
    url.scheme() == "https"
        || (allow_local_http
            && url.scheme() == "http"
            && matches!(
                url.host_str(),
                Some("localhost" | "127.0.0.1" | "::1")
            ))
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

#[cfg(test)]
mod tests {
    use super::{convex_deployment_url, convex_site_url};

    #[test]
    fn local_convex_http_is_debug_only() {
        assert!(convex_deployment_url("http://127.0.0.1:3210", true).is_ok());
        assert!(convex_site_url("http://127.0.0.1:3211", true).is_ok());
        assert!(convex_deployment_url("http://127.0.0.1:3210", false).is_err());
        assert!(convex_site_url("http://example.com:3211", true).is_err());
    }
}
