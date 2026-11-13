//! The App's local HTTP API on 127.0.0.1, under a secret first path segment. It binds before Tauri
//! builds, so the native window and tabs know its address before the frontend loads. The Core
//! proxy (`crate::cores::proxy`) binds beside it, on a port of its own, behind the same secret.

mod boot;
pub mod endpoints;
pub mod events;
pub mod guard;
pub mod ops;
#[cfg(test)]
mod tests;

use std::{
    io,
    net::{Ipv4Addr, TcpListener},
    sync::Arc,
};

use axum::{
    Router,
    extract::{FromRef, Request, State},
    http::{header::UPGRADE, request::Parts},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use contracts::{
    ApiError, Empty, Json,
    server::{Req, route},
};
use engine::{
    Engine, EventSink,
    access::{Access, BoxFuture, Caller, Permission},
};
use tauri::{Runtime, plugin::TauriPlugin};
use url::Url;

use crate::{
    accounts,
    cores::{self, Cores},
    desktop::{self, AppSlot, updater::Updater},
    engine_handle::EngineHandle,
    friends, launch, library, screenshots, skins, synced_options,
};

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

    /// `/health` and `/app/boot` answer from bind; `/events` and the routes over the engine wait
    /// for it.
    pub fn router(
        &self,
        boot: Boot,
        events: events::Events,
        engine: EngineHandle,
        app: AppSlot,
        updater: Arc<Updater>,
    ) -> Router {
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
        let ops = Arc::new(ops::Ops::new(events.app.clone()));
        let state = AppState {
            engine: engine.clone(),
            access: Arc::new(AppAccess),
            ops,
            updater,
            app: app.clone(),
            events: events.engine.clone(),
        };
        let (events_path, events) = events::route(events);
        // Updates come without the engine, so a release that cannot start it can still be fixed.
        let updates = ops::routes()
            .merge(desktop::updater::routes())
            .with_state(state.clone());
        // Every other router over AppState merges here: its engine exists only behind
        // wait_for_engine.
        let waiting = Router::new()
            .route(events_path, events)
            .merge(
                engine::routes::routes()
                    .merge(library::routes::routes())
                    .merge(synced_options::routes::routes())
                    .merge(screenshots::routes::routes())
                    .merge(friends::routes::routes())
                    .merge(skins::routes::routes())
                    .merge(desktop::op_routes())
                    .merge(accounts::routes::routes())
                    .merge(launch::routes())
                    .with_state(state),
            )
            .route_layer(middleware::from_fn_with_state(
                engine.clone(),
                wait_for_engine,
            ));
        let routes = Router::new()
            .route(health_path, health)
            .route(boot_path, boot)
            .merge(waiting)
            .merge(updates)
            .merge(cores::routes(Arc::clone(&self.cores)))
            .merge(desktop::routes(engine, app, &self.secret));
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

/// What the routes over the engine read.
#[derive(Clone)]
pub struct AppState {
    engine: EngineHandle,
    access: Arc<dyn Access>,
    ops: Arc<ops::Ops>,
    updater: Arc<Updater>,
    app: AppSlot,
    events: EventSink,
}

impl FromRef<AppState> for Arc<Engine> {
    fn from_ref(state: &AppState) -> Self {
        state
            .engine
            .try_get()
            .expect("wait_for_engine holds these routes until the engine is up")
    }
}

impl FromRef<AppState> for Arc<dyn Access> {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.access)
    }
}

impl FromRef<AppState> for Arc<ops::Ops> {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.ops)
    }
}

impl FromRef<AppState> for Arc<Updater> {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.updater)
    }
}

impl FromRef<AppState> for EventSink {
    fn from_ref(state: &AppState) -> Self {
        state.events.clone()
    }
}

impl FromRef<AppState> for AppSlot {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.app)
    }
}

/// The App's one local user may do everything.
struct AppAccess;

impl Access for AppAccess {
    fn caller<'a>(
        &'a self,
        _: &'a Parts,
    ) -> BoxFuture<'a, Result<Caller, ApiError>> {
        Box::pin(async { Ok(Caller::Local) })
    }

    fn require<'a>(
        &'a self,
        _: &'a Caller,
        _: Option<&'a str>,
        _: Permission,
    ) -> BoxFuture<'a, Result<Permission, ApiError>> {
        Box::pin(async { Ok(Permission::Owner) })
    }
}

/// A page opening its event socket retries a failed start, as a reload did when the start was a
/// command. The HTTP read that asks why a socket was refused only reads the failure.
async fn wait_for_engine(
    State(engine): State<EngineHandle>,
    request: Request,
    next: Next,
) -> Response {
    if request.headers().contains_key(UPGRADE) {
        engine.retry();
    }
    match engine.ready().await {
        Ok(_) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

fn new_secret() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
