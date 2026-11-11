//! Who may use the local server. Any program on this machine can reach 127.0.0.1, and any website
//! can through the user's browser, so a request needs the secret path segment and, from a browser,
//! one of the App's own origins.

use std::sync::Arc;
#[cfg(debug_assertions)]
use std::time::Instant;

use axum::{
    extract::{Request, State},
    http::{
        HeaderMap, HeaderValue, Method, StatusCode,
        header::{
            ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
            ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_MAX_AGE,
            ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD,
            HOST, ORIGIN, UPGRADE, VARY,
        },
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use contracts::{ApiError, ErrorCode};
use url::Url;

/// The origins the App's native window loads from in a release build.
pub const APP_ORIGINS: [&str; 3] = [
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
];

pub struct Guard {
    pub secret: String,
    pub port: u16,
    /// The Vite dev server's port, in a debug build that loads its frontend from it.
    pub dev_port: Option<u16>,
}

impl Guard {
    pub fn new(secret: String, port: u16, dev_url: Option<&Url>) -> Self {
        Self {
            secret,
            port,
            dev_port: dev_url.and_then(Url::port_or_known_default),
        }
    }

    /// Compared in constant time, so the answer's timing says nothing about how much of a guess
    /// was right.
    fn has_secret(&self, path: &str) -> bool {
        let Some(rest) = path.strip_prefix('/') else {
            return false;
        };
        let given = rest.split('/').next().unwrap_or_default().as_bytes();
        let expected = self.secret.as_bytes();
        given
            .iter()
            .zip(expected)
            .fold(given.len() ^ expected.len(), |differs, (a, b)| {
                differs | usize::from(a ^ b)
            })
            == 0
    }

    /// A name other than loopback is DNS rebinding: a website's own name pointed at 127.0.0.1.
    fn has_host(&self, headers: &HeaderMap) -> bool {
        headers
            .get(HOST)
            .and_then(|host| host.to_str().ok())
            .and_then(|host| host.rsplit_once(':'))
            .is_some_and(|(name, port)| {
                matches!(name, "127.0.0.1" | "localhost")
                    && port.parse() == Ok(self.port)
            })
    }

    /// Never another loopback port in a release build, so a Core's content served by the App's
    /// proxy cannot call this server.
    pub fn allows(&self, origin: &str) -> bool {
        if APP_ORIGINS.contains(&origin) {
            return true;
        }
        let Ok(url) = Url::parse(origin) else {
            return false;
        };
        if url.scheme() != "http" {
            return false;
        }
        let loopback =
            matches!(url.host(), Some(url::Host::Ipv4(ip)) if ip.is_loopback());
        let vite = self.dev_port.is_some_and(|port| {
            url.port() == Some(port)
                && (loopback
                    || url.host_str().is_some_and(|host| {
                        host == "localhost" || host.ends_with(".localhost")
                    }))
        });
        // The browser bridge serves the frontend from a port of its own the App is not told.
        vite || (cfg!(all(debug_assertions, feature = "browser-bridge"))
            && loopback)
    }
}

pub async fn guard(
    State(guard): State<Arc<Guard>>,
    request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();
    if !guard.has_secret(request.uri().path()) {
        return refuse(
            StatusCode::NOT_FOUND,
            ErrorCode::NotFound,
            "Not found.",
        );
    }
    if !guard.has_host(headers) {
        return refuse(
            StatusCode::MISDIRECTED_REQUEST,
            ErrorCode::Forbidden,
            "Only 127.0.0.1 and localhost reach the App.",
        );
    }
    // No caller of this server shares its origin; a document that does could read local files.
    if headers
        .get("sec-fetch-site")
        .is_some_and(|site| site == "same-origin")
    {
        return refuse(
            StatusCode::FORBIDDEN,
            ErrorCode::Forbidden,
            "Same-origin requests are refused.",
        );
    }
    let origin = headers.get(ORIGIN).cloned();
    match &origin {
        Some(origin)
            if !origin.to_str().is_ok_and(|origin| guard.allows(origin)) =>
        {
            return refuse(
                StatusCode::FORBIDDEN,
                ErrorCode::Forbidden,
                "Only the Amberite App can use this address.",
            );
        }
        None if headers.contains_key(UPGRADE) => {
            return refuse(
                StatusCode::FORBIDDEN,
                ErrorCode::Forbidden,
                "A socket needs the App's origin.",
            );
        }
        _ => {}
    }
    if let Some(origin) = &origin
        && request.method() == Method::OPTIONS
        && headers.contains_key(ACCESS_CONTROL_REQUEST_METHOD)
    {
        return preflight(headers, origin.clone());
    }

    #[cfg(debug_assertions)]
    let started = Instant::now();
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    // Also without an Origin, so an `<img>` load's cached copy never answers a later fetch.
    headers.append(VARY, HeaderValue::from_static("Origin"));
    let Some(origin) = origin else {
        return response;
    };
    #[cfg(debug_assertions)]
    if let Ok(timing) = HeaderValue::try_from(format!(
        "app;dur={:.2}",
        started.elapsed().as_secs_f64() * 1000.0
    )) {
        headers.insert("server-timing", timing);
        // Every caller is cross-origin, and a page reads Server-Timing only when this allows it.
        headers.insert("timing-allow-origin", origin.clone());
    }
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    response
}

fn preflight(headers: &HeaderMap, origin: HeaderValue) -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    let allowed = response.headers_mut();
    allowed.insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    allowed.insert(VARY, HeaderValue::from_static("Origin"));
    allowed.insert(
        ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE"),
    );
    allowed.insert(ACCESS_CONTROL_MAX_AGE, HeaderValue::from_static("600"));
    if let Some(requested) = headers.get(ACCESS_CONTROL_REQUEST_HEADERS) {
        allowed.insert(ACCESS_CONTROL_ALLOW_HEADERS, requested.clone());
    }
    // Chromium's Private Network Access asks before a page reaches loopback.
    if headers.contains_key("access-control-request-private-network") {
        allowed.insert(
            "access-control-allow-private-network",
            HeaderValue::from_static("true"),
        );
    }
    response
}

fn refuse(
    status: StatusCode,
    code: ErrorCode,
    description: &'static str,
) -> Response {
    (status, ApiError::new(code, description)).into_response()
}
