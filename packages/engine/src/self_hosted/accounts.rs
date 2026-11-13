use crate::state::{Sessions, User};
use crate::util::fetch::{FetchSemaphore, fetch_json};
use reqwest::{Method, Url};
use std::borrow::Cow;

/// The account service the host routes Modrinth account calls to.
#[derive(Debug)]
pub struct AccountEndpoints {
    pub(crate) api: Url,
    web: String,
    socket: String,
}

impl AccountEndpoints {
    pub fn new(api: &str, web: Option<&str>) -> crate::Result<Self> {
        let api = Url::parse(api)?;
        if !matches!(api.scheme(), "http" | "https") || api.cannot_be_a_base() {
            return Err(crate::ErrorKind::InputError(
                "Invalid account service URL".into(),
            )
            .into());
        }
        let mut socket = api.clone();
        socket
            .set_scheme(if api.scheme() == "https" { "wss" } else { "ws" })
            .map_err(|_| {
                crate::ErrorKind::InputError("Invalid socket URL".into())
            })?;
        let api = format!("{}/", api.as_str().trim_end_matches('/'));
        let web = format!("{}/", web.unwrap_or(&api).trim_end_matches('/'));
        let socket = format!("{}/", socket.as_str().trim_end_matches('/'));
        Ok(Self {
            api: Url::parse(&api)?,
            web,
            socket,
        })
    }
}

pub fn enabled(sessions: &Sessions) -> bool {
    sessions.accounts.is_some()
}

/// The account service's API, when the host routes accounts to one.
pub fn api(sessions: &Sessions) -> Option<&Url> {
    sessions.accounts.as_ref().map(|config| &config.api)
}

pub fn login_url(sessions: &Sessions, sign_up: bool) -> Option<String> {
    sessions.accounts.as_ref().map(|config| {
        format!(
            "{}auth/{}",
            config.web,
            if sign_up { "sign-up" } else { "sign-in" }
        )
    })
}

pub fn socket_base(sessions: &Sessions) -> &str {
    sessions
        .accounts
        .as_ref()
        .map_or(env!("MODRINTH_SOCKET_URL"), |config| config.socket.as_str())
}

/// Only account routes move. Projects, versions, teams and CDN bytes stay public.
pub(crate) fn route<'a>(
    sessions: &Sessions,
    url: &'a str,
    path: Option<&str>,
) -> Cow<'a, str> {
    let Some(config) = &sessions.accounts else {
        return Cow::Borrowed(url);
    };
    let Some(path) = path else {
        return Cow::Borrowed(url);
    };
    let private = path.starts_with("/v2/user")
        || path.starts_with("/v3/user")
        || path.starts_with("/v2/session")
        || path.starts_with("/v3/friend")
        || path.starts_with("/v3/block")
        || path.starts_with("/v2/notification")
        || path.starts_with("/v3/notification");
    if private {
        for (base, version) in [
            (env!("MODRINTH_API_URL"), "v2/"),
            (env!("MODRINTH_API_URL_V3"), "v3/"),
        ] {
            if let Some(suffix) = url.strip_prefix(base) {
                return Cow::Owned(format!("{}{version}{suffix}", config.api));
            }
        }
    }
    Cow::Borrowed(url)
}

pub(crate) fn may_send_credentials(sessions: &Sessions, url: &str) -> bool {
    if let Some(config) = &sessions.accounts {
        return Url::parse(url).is_ok_and(|url| {
            url.origin() == config.api.origin()
                && url.path().starts_with(config.api.path())
        });
    }
    url.starts_with(env!("MODRINTH_API_URL"))
        || url.starts_with(env!("MODRINTH_API_URL_V3"))
        || url.starts_with("https://cdn.modrinth.com/")
}

/// Local accounts bypass the public author cache, where the same ID could refer
/// to a completely different person embedded in a Modrinth project team.
pub async fn users(
    ids: &[&str],
    semaphore: &FetchSemaphore,
    sessions: &Sessions,
) -> crate::Result<Vec<User>> {
    let ids = urlencoding::encode(&serde_json::to_string(ids)?).into_owned();
    fetch_json(
        Method::GET,
        &format!("{}users?ids={ids}", env!("MODRINTH_API_URL")),
        None,
        None,
        Some("/v2/users"),
        semaphore,
        sessions,
    )
    .await
}

pub fn origin(sessions: &Sessions) -> &str {
    sessions
        .accounts
        .as_ref()
        .map_or(env!("MODRINTH_API_URL"), |config| config.api.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ModrinthSession;

    fn signed_in(accounts: Option<&str>) -> Sessions {
        let sessions = Sessions::new(
            accounts.map(|api| AccountEndpoints::new(api, None).unwrap()),
        );
        sessions.set_modrinth_session(Some(ModrinthSession {
            token: "imported-session".into(),
        }));
        sessions
    }

    #[test]
    fn the_session_reaches_only_urls_that_may_receive_credentials() {
        let modrinth = concat!(env!("MODRINTH_API_URL"), "project/x");
        let modrinth_v3 = concat!(env!("MODRINTH_API_URL_V3"), "project/x");
        let cases = [
            (None, modrinth, true),
            (None, modrinth_v3, true),
            (None, "https://cdn.modrinth.com/data/x.jar", true),
            (None, "https://accounts.example/v2/user", false),
            (
                Some("https://accounts.example/api"),
                "https://accounts.example/api/v2/user",
                true,
            ),
            (
                Some("https://accounts.example/api"),
                "https://accounts.example/other",
                false,
            ),
            (
                Some("https://accounts.example/api"),
                "http://accounts.example/api/v2/user",
                false,
            ),
            (Some("https://accounts.example/api"), modrinth, false),
            (
                Some("https://accounts.example/api"),
                "https://cdn.modrinth.com/data/x.jar",
                false,
            ),
        ];
        for (accounts, url, allowed) in cases {
            let sessions = signed_in(accounts);
            assert_eq!(
                sessions.modrinth_token(url).as_deref(),
                allowed.then_some("imported-session"),
                "{url}"
            );
            sessions.set_modrinth_session(None);
            assert_eq!(sessions.modrinth_token(url), None, "{url}");
        }
    }
}
