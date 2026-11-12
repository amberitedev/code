//! The App's local HTTP API on 127.0.0.1, under a secret first path segment. It binds before Tauri
//! builds, so the native window and tabs know its address before the frontend loads. The Core
//! proxy (`crate::cores::proxy`) binds beside it, on a port of its own, behind the same secret.

mod boot;
pub mod endpoints;
pub mod events;
pub mod guard;
#[cfg(test)]
mod tests;

use std::{
    io,
    net::{Ipv4Addr, TcpListener},
    sync::Arc,
};

use axum::{Router, middleware};
use contracts::{
    Empty, Json,
    server::{Req, route},
};
use tauri::{Runtime, plugin::TauriPlugin};
use url::Url;

use crate::cores::{self, Cores};

use endpoints::{
    boot::{Boot, GetBoot},
    health::CheckHealth,
};
use guard::{APP_ORIGINS, Guard};

pub struct LocalServer {
    pub listener: TcpListener,
    pub secret: String,
    /// Where the frontend loads from in a debug build; its origin may call the server.
    dev_url: Option<Url>,
    proxy: TcpListener,
    cores: Arc<Cores>,
}

impl LocalServer {
    /// `port` and `secret` come from the dev runner; otherwise any free port and a new secret.
    /// `dev_url` is Tauri's configured one, which a release build's config still names.
    pub fn bind(
        port: Option<u16>,
        secret: Option<String>,
        dev_url: Option<Url>,
    ) -> io::Result<Self> {
        let listener =
            TcpListener::bind((Ipv4Addr::LOCALHOST, port.unwrap_or(0)))?;
        listener.set_nonblocking(true)?;
        let proxy = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        proxy.set_nonblocking(true)?;
        let secret = secret.unwrap_or_else(new_secret);
        let cores = Arc::new(Cores::new(format!(
            "http://127.0.0.1:{}/{secret}",
            proxy.local_addr()?.port()
        )));
        Ok(Self {
            listener,
            secret,
            dev_url: dev_url.filter(|_| cfg!(debug_assertions)),
            proxy,
            cores,
        })
    }

    pub fn port(&self) -> u16 {
        self.listener
            .local_addr()
            .expect("a bound listener has an address")
            .port()
    }

    /// Carries the secret: never log it.
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/{}", self.port(), self.secret)
    }

    pub fn router(&self, boot: Boot, events: events::Events) -> Router {
        let guard = Arc::new(Guard::new(
            self.secret.clone(),
            self.port(),
            self.dev_url.as_ref(),
        ));
        let (health_path, health) =
            route::<CheckHealth, _, _, _>(|_: Req<CheckHealth>| async {
                Empty
            });
        let (boot_path, boot) =
            route::<GetBoot, _, _, _>(move |_: Req<GetBoot>| {
                let boot = boot.clone();
                async move { Json(boot) }
            });
        let (events_path, events) = events::route(events);
        let routes = Router::new()
            .route(health_path, health)
            .route(boot_path, boot)
            .route(events_path, events)
            .merge(cores::routes(Arc::clone(&self.cores)));
        Router::new()
            .nest(&format!("/{}", self.secret), routes)
            .layer(middleware::from_fn_with_state(guard, guard::guard))
    }

    /// Hands the main window the base URL before its scripts run. Every webview runs a plugin's
    /// init script, the `signin` window on Microsoft's page included, so the script checks the
    /// webview's label, frame and origin.
    pub fn init_plugin<R: Runtime>(&self) -> TauriPlugin<R> {
        tauri::plugin::Builder::new("local-server")
            .js_init_script(self.init_script())
            .build()
    }

    fn init_script(&self) -> String {
        let origins: Vec<String> = APP_ORIGINS
            .iter()
            .map(ToString::to_string)
            .chain(
                self.dev_url
                    .as_ref()
                    .map(|url| url.origin().ascii_serialization()),
            )
            .collect();
        format!(
            "{{\n{}\nlocalInit({}, {});\n}}",
            include_str!("local-init.js").trim_end(),
            serde_json::Value::from(self.base_url()),
            serde_json::Value::from(origins),
        )
    }

    /// Serves the router and the Core proxy.
    pub async fn serve(self, router: Router) {
        let proxy = tokio::net::TcpListener::from_std(self.proxy)
            .expect("the listener is non-blocking");
        let guard = Guard::new(
            self.secret,
            proxy.local_addr().expect("a bound listener").port(),
            self.dev_url.as_ref(),
        );
        tokio::spawn(cores::proxy::serve(proxy, Arc::new(guard), self.cores));
        let listener = tokio::net::TcpListener::from_std(self.listener)
            .expect("the listener is non-blocking");
        if let Err(error) = axum::serve(listener, router).await {
            tracing::error!(%error, "The App's local server stopped");
        }
    }
}

fn new_secret() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
