use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevAppConfig {
    pub app_id: String,
    pub username: String,
    pub branch: String,
    pub title: String,
    pub data_dir: String,
    pub convex_url: String,
    pub convex_site_url: String,
    pub control_port: u16,
}

#[cfg(debug_assertions)]
mod runtime {
    use super::DevAppConfig;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::env;
    use std::sync::{LazyLock, Mutex, OnceLock};
    use std::time::{Duration, Instant};
    use tauri::{Emitter, Manager};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::{TcpListener, TcpStream};

    static CONFIG: OnceLock<Option<DevAppConfig>> = OnceLock::new();
    static ACCOUNT_RESULTS: LazyLock<Mutex<HashMap<String, Option<String>>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    #[derive(Deserialize)]
    struct ControlRequest {
        action: String,
        username: Option<String>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct AccountSwitchRequest {
        request_id: String,
        username: String,
    }

    #[derive(Serialize)]
    struct ControlResponse {
        ok: bool,
        error: Option<String>,
        pid: u32,
    }

    pub fn prepare() {
        let mut args = env::args();
        let mut config = None;
        while let Some(arg) = args.next() {
            if arg != "--amberite-dev-config" {
                continue;
            }
            let raw = args.next().unwrap_or_else(|| {
                panic!("--amberite-dev-config requires JSON")
            });
            let parsed: DevAppConfig = serde_json::from_str(&raw)
                .unwrap_or_else(|error| {
                    panic!("invalid Amberite dev config: {error}")
                });
            assert!(
                !parsed.app_id.trim().is_empty(),
                "dev app ID cannot be empty"
            );
            assert!(parsed.control_port > 0, "dev control port cannot be zero");
            unsafe {
                env::set_var("THESEUS_CONFIG_DIR", &parsed.data_dir);
                env::set_var(
                    "WEBVIEW2_USER_DATA_FOLDER",
                    format!("{}/webview2", parsed.data_dir),
                );
            }
            config = Some(parsed);
            break;
        }
        let _ = CONFIG.set(config);
    }

    pub fn config() -> Option<&'static DevAppConfig> {
        CONFIG.get().and_then(Option::as_ref)
    }

    pub fn start_control(app: tauri::AppHandle) {
        let Some(config) = config().cloned() else {
            return;
        };
        tauri::async_runtime::spawn(async move {
            let address = ("127.0.0.1", config.control_port);
            let listener = match TcpListener::bind(address).await {
                Ok(listener) => listener,
                Err(error) => {
                    tracing::error!(
                        "Could not bind Amberite dev control port: {error}"
                    );
                    return;
                }
            };
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = handle_connection(app, stream).await {
                        tracing::warn!(
                            "Amberite dev control request failed: {error}"
                        );
                    }
                });
            }
        });
    }

    pub fn complete_account_switch(request_id: String, error: Option<String>) {
        ACCOUNT_RESULTS.lock().unwrap().insert(request_id, error);
    }

    async fn handle_connection(
        app: tauri::AppHandle,
        stream: TcpStream,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (reader, mut writer) = stream.into_split();
        let mut raw = String::new();
        BufReader::new(reader).read_line(&mut raw).await?;
        let request: ControlRequest = serde_json::from_str(raw.trim())?;
        let result = handle_request(&app, request).await;
        let response = ControlResponse {
            ok: result.is_ok(),
            error: result.err(),
            pid: std::process::id(),
        };
        writer
            .write_all(
                format!("{}\n", serde_json::to_string(&response)?).as_bytes(),
            )
            .await?;
        Ok(())
    }

    async fn handle_request(
        app: &tauri::AppHandle,
        request: ControlRequest,
    ) -> Result<(), String> {
        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "main window is unavailable".to_string())?;
        match request.action.as_str() {
            "status" => Ok(()),
            "focus" => {
                window.show().map_err(|error| error.to_string())?;
                window.set_focus().map_err(|error| error.to_string())
            }
            "hide" => window.hide().map_err(|error| error.to_string()),
            "reload" => window
                .eval("window.location.reload()")
                .map_err(|error| error.to_string()),
            "close" => {
                app.exit(0);
                Ok(())
            }
            "account" => {
                let username = request
                    .username
                    .ok_or_else(|| "username is required".to_string())?;
                let request_id = uuid::Uuid::new_v4().to_string();
                app.emit(
                    "amberite://dev-account",
                    AccountSwitchRequest {
                        request_id: request_id.clone(),
                        username: username.clone(),
                    },
                )
                .map_err(|error| error.to_string())?;
                wait_for_account_result(&request_id).await?;
                if let Some(config) = config() {
                    window
                        .set_title(&format!(
                            "{} · {} · {}",
                            config.branch, config.app_id, username
                        ))
                        .map_err(|error| error.to_string())?;
                }
                Ok(())
            }
            _ => Err(format!("unknown action {}", request.action)),
        }
    }

    async fn wait_for_account_result(request_id: &str) -> Result<(), String> {
        let started_at = Instant::now();
        loop {
            if let Some(error) =
                ACCOUNT_RESULTS.lock().unwrap().remove(request_id)
            {
                return error.map_or(Ok(()), Err);
            }
            if started_at.elapsed() >= Duration::from_secs(10) {
                return Err("account switch timed out".to_string());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

#[cfg(not(debug_assertions))]
mod runtime {
    use super::DevAppConfig;

    pub fn prepare() {}

    pub fn config() -> Option<&'static DevAppConfig> {
        None
    }

    pub fn start_control(_app: tauri::AppHandle) {}

    pub fn complete_account_switch(
        _request_id: String,
        _error: Option<String>,
    ) {
    }
}

pub use runtime::{complete_account_switch, config, prepare, start_control};
