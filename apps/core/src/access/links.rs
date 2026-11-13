//! Download links: `POST /links` turns a session into a token for one GET of one exact path and
//! query, for a download that cannot send the session. The GET sends it as `?link=`, and
//! [`CoreAccess`](super::CoreAccess) redeems it as the caller who made it.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{extract::State, http::Uri};
use contracts::{
    ApiError, ErrorCode, Json,
    core::{CreateDownloadLink, DownloadLink},
    server::Req,
};

use super::{Caller, CoreAccess, Permission};
use crate::api::CoreState;

const LIFETIME: Duration = Duration::from_secs(300);
const MAX_PATH: usize = 4096;

/// Links made and not yet used. Each works once, for [`LIFETIME`].
#[derive(Default)]
pub struct Links(Mutex<HashMap<String, Link>>);

struct Link {
    caller: Caller,
    path: String,
    expires: Instant,
}

impl Links {
    fn issue(&self, caller: Caller, path: String) -> String {
        let mut bytes = [0u8; 32];
        ring::rand::SecureRandom::fill(
            &ring::rand::SystemRandom::new(),
            &mut bytes,
        )
        .expect("the system has randomness");
        let token = hex::encode(bytes);
        let mut links = self.0.lock().expect("not poisoned");
        links.retain(|_, link| link.expires > Instant::now());
        links.insert(
            token.clone(),
            Link {
                caller,
                path,
                expires: Instant::now() + LIFETIME,
            },
        );
        token
    }

    /// Use up `token`, whatever the path: a token sent for another path is spent too.
    fn redeem(&self, token: &str, path: &str) -> Option<Caller> {
        self.0
            .lock()
            .expect("not poisoned")
            .remove(token)
            .filter(|link| link.expires > Instant::now() && link.path == path)
            .map(|link| link.caller)
    }
}

fn expired() -> ApiError {
    ApiError::new(
        ErrorCode::InvalidCredentials,
        "This download link has expired or was used.",
    )
}

/// What a link may be made for, and is checked against again when used: a path of a server the
/// caller is an editor of, as every download is.
async fn allowed(
    access: &CoreAccess,
    caller: &Caller,
    path: &str,
) -> Result<(), ApiError> {
    let server = path
        .strip_prefix("/servers/")
        .and_then(|rest| rest.split_once('/'))
        .map(|(server, _)| server)
        .filter(|server| !server.is_empty())
        .ok_or_else(|| {
            ApiError::new(
                ErrorCode::InvalidInput,
                "A link is for a path under /servers/{id}/.",
            )
        })?;
    access.require(caller, server, Permission::Edit).await?;
    Ok(())
}

pub async fn create(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { body, .. }: Req<CreateDownloadLink>,
) -> Result<Json<DownloadLink>, ApiError> {
    if body.path.len() > MAX_PATH {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "The path is too long.",
        ));
    }
    allowed(&state.access, &caller, &body.path).await?;
    let token = state.access.links.issue(caller, body.path);
    Ok(Json(DownloadLink { token }))
}

/// The `link` token of `uri`, and the path and query it was made for: `uri` without it.
pub(super) fn split(uri: &Uri) -> Option<(&str, String)> {
    let query = uri.query()?;
    let mut token = None;
    let mut rest = Vec::new();
    for pair in query.split('&') {
        match pair.strip_prefix("link=") {
            Some(value) if token.is_none() => token = Some(value),
            _ => rest.push(pair),
        }
    }
    let token = token?;
    let path = if rest.is_empty() {
        uri.path().to_owned()
    } else {
        format!("{}?{}", uri.path(), rest.join("&"))
    };
    Some((token, path))
}

impl CoreAccess {
    /// The caller who made the link `token` for `path` (see [`split`]), unless it is spent,
    /// expired, for another path, or no longer allowed.
    pub(super) async fn redeem(
        &self,
        token: &str,
        path: &str,
    ) -> Result<Caller, ApiError> {
        let caller = self.links.redeem(token, path).ok_or_else(expired)?;
        if !self.revalidate(&caller).await {
            return Err(expired());
        }
        allowed(self, &caller, path).await?;
        Ok(caller)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_is_its_paths_without_itself() {
        let uri: Uri = "/servers/a/f?path=x%2Fy&link=t&b=1".parse().unwrap();
        assert_eq!(
            split(&uri),
            Some(("t", "/servers/a/f?path=x%2Fy&b=1".to_owned()))
        );
        let uri: Uri = "/servers/a/download?link=t".parse().unwrap();
        assert_eq!(split(&uri), Some(("t", "/servers/a/download".to_owned())));
        let uri: Uri = "/servers/a/f?link=t&link=u".parse().unwrap();
        assert_eq!(split(&uri), Some(("t", "/servers/a/f?link=u".to_owned())));
        let uri: Uri = "/servers/a/f?path=x".parse().unwrap();
        assert_eq!(split(&uri), None);
    }

    #[test]
    fn an_expired_link_is_refused() {
        let links = Links::default();
        let token = links.issue(Caller::Local, "/servers/a/f".to_owned());
        links.0.lock().unwrap().get_mut(&token).unwrap().expires =
            Instant::now() - Duration::from_secs(1);
        assert_eq!(links.redeem(&token, "/servers/a/f"), None);
    }
}
