#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
#![recursion_limit = "256"]

use engine::prelude::*;
use native_dialog::{DialogBuilder, MessageLevel};
use std::sync::Arc;
use tauri::{Listener, Manager};

mod accounts;
mod api;
mod cores;
mod data_dir;
mod desktop;
mod dev;
mod discord;
mod engine_handle;
mod friends;
mod hooks;
mod import;
mod launch;
mod library;
mod logger;
mod onboarding;
mod presence;
mod reactions;
mod screenshots;
mod server;
mod settings;
mod skins;
mod synced_options;

#[cfg(target_os = "macos")]
mod macos;

/// Starts the engine and signs a dev App in before its routes open. Setup runs it, and opening the
/// event socket runs it again after a failure.
async fn start_engine(
    app: tauri::AppHandle,
    events: engine::EventSink,
    presence: Arc<presence::Presence>,
) -> anyhow::Result<Arc<Engine>> {
    tracing::info!("Initializing app state...");
    let started = async {
        let reacting = events.subscribe();
        let config = data_dir::engine_config(
            data_dir::resolve(&app.config().identifier)?,
            events,
            dev::accounts()?,
        )
        .await?;
        let state = Engine::new(config).await?;
        accounts::self_hosted::bind_origin(state.pool(), &state.sessions)
            .await?;
        reactions::start(&state, Arc::clone(&presence), app.clone(), reacting);
        #[cfg(debug_assertions)]
        dev::sign_in(&state, &presence).await;
        anyhow::Ok(state)
    }
    .await;
    if let Err(error) = &started {
        tracing::error!("The engine could not start: {error:#}");
    }
    started
}

// Should be call once Vue has mounted the app
#[tracing::instrument(skip_all)]
#[tauri::command]
fn show_window(app: tauri::AppHandle) {
    if dev::config().is_some_and(|config| config.hide_window) {
        return;
    }
    let win = app.get_window("main").unwrap();
    if let Err(e) = win.show() {
        DialogBuilder::message()
            .set_level(MessageLevel::Error)
            .set_title("Initialization error")
            .set_text(format!(
                "Cannot display application window due to an error:\n{e}"
            ))
            .alert()
            .show()
            .unwrap();
        panic!("cannot display application window")
    } else {
        let _ = win.set_focus();
    }
}

// if Tauri app is called with arguments, then those arguments will be treated as commands
// ie: deep links or filepaths for .mrpacks
fn main() {
    dev::prepare();
    engine::install_crypto_provider();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to create app async runtime");
    tauri::async_runtime::set(runtime.handle().clone());

    /*
        tracing is set based on the environment variable RUST_LOG=xxx, depending on the amount of logs to show
            ERROR > WARN > INFO > DEBUG > TRACE
        eg. RUST_LOG=info will show info, warn, and error logs
            RUST_LOG="engine=trace" will show *all* messages but from engine only (and not dependencies using similar crates)
            RUST_LOG="engine=trace" will show *all* messages but from engine only (and not dependencies using similar crates)

        Error messages returned to Tauri will display as traced error logs if they return an error.
        This will also include an attached span trace if the error is from a tracing error, and the level is set to info, debug, or trace

        on unix:
            RUST_LOG="engine=trace" {run command}

    */

    let mut tauri_context = tauri::generate_context!();
    if let Some(dev_url) =
        dev::config().and_then(|config| config.dev_url.clone())
    {
        tauri_context.config_mut().build.dev_url = Some(dev_url);
    }

    let _log_guard =
        match data_dir::logs_dir(&tauri_context.config().identifier) {
            Ok(logs_dir) => logger::start_logger(&logs_dir),
            Err(error) => {
                eprintln!("Could not start logger: {error:#}");
                None
            }
        };

    tracing::info!("Initialized tracing subscriber. Loading Modrinth App!");

    let local = server::LocalServer::bind(
        dev::config().and_then(|config| config.local_port),
        dev::config().and_then(|config| config.local_secret.clone()),
        tauri_context.config().build.dev_url.clone(),
    )
    .expect("failed to bind the App's local server");
    let local_init = local.init_plugin();
    let events = server::events::Events::default();
    let engine = engine_handle::EngineHandle::default();
    let presence = Arc::new(
        presence::Presence::init()
            .expect("failed to set up Discord and the friends socket"),
    );
    let desktop = desktop::AppSlot::default();
    let updater = Arc::new(desktop::updater::Updater::new(desktop.clone()));
    let router = local.router(
        server::endpoints::boot::Boot::current(
            tauri_context.package_info().version.to_string(),
        ),
        events.clone(),
        engine.clone(),
        Arc::clone(&presence),
        desktop.clone(),
        Arc::clone(&updater),
        tauri_context.config().identifier.clone(),
    );
    runtime.spawn(local.serve(router));

    let mut builder = tauri::Builder::default().plugin(local_init);

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .menu(macos::menu::create)
            .on_menu_event(macos::menu::handle_event);
    }

    #[cfg(feature = "updater")]
    {
        use engine::launcher_user_agent;
        use tauri_plugin_http::reqwest::header::{HeaderValue, USER_AGENT};
        builder = builder.plugin(
            tauri_plugin_updater::Builder::new()
                .header(
                    USER_AGENT,
                    HeaderValue::from_str(&launcher_user_agent()).unwrap(),
                )
                .unwrap()
                .build(),
        );
    }

    if dev::config().is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(
            |app, args, _cwd| {
                if let Some(payload) = args.get(1) {
                    tracing::info!("Handling command-line deep link");
                    let payload = payload.clone();
                    tauri::async_runtime::spawn(api::utils::handle_command(
                        app.clone(),
                        payload,
                    ));
                }

                if let Some(win) = app.get_window("main") {
                    let _ = win.set_focus();
                }
            },
        ));
    }

    builder = builder
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        // Links open through the local server's opener route.
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        );

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            SWP_NOZORDER, SetWindowPos,
        };

        // Refresh the hidden window's frame before window-state measures its client area.
        builder = builder.plugin(
            tauri::plugin::Builder::<_, ()>::new("window-frame")
                .on_window_ready(|window| {
                    if window.label() != "main" {
                        return;
                    }

                    let hwnd = match window.hwnd() {
                        Ok(hwnd) => hwnd,
                        Err(e) => {
                            tracing::warn!("Failed to get window handle: {e}");
                            return;
                        }
                    };

                    if let Err(e) = unsafe {
                        SetWindowPos(
                            hwnd,
                            None,
                            0,
                            0,
                            0,
                            0,
                            SWP_FRAMECHANGED
                                | SWP_NOACTIVATE
                                | SWP_NOMOVE
                                | SWP_NOSIZE
                                | SWP_NOZORDER,
                        )
                    } {
                        tracing::warn!("Failed to refresh window frame: {e}");
                    }
                })
                .build(),
        );
    }

    let window_state_flags = tauri_plugin_window_state::StateFlags::POSITION
        | tauri_plugin_window_state::StateFlags::SIZE
        | tauri_plugin_window_state::StateFlags::MAXIMIZED;
    #[cfg(target_os = "windows")]
    let window_state_flags =
        window_state_flags | tauri_plugin_window_state::StateFlags::DECORATIONS;

    builder = builder
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_filename("app-window-state.json")
                .with_denylist(&["signin"])
                // Restoring VISIBLE would override the hidden startup window.
                .with_state_flags(window_state_flags)
                .build(),
        )
        .setup(move |app| {
            let _ = desktop.set(app.handle().clone());
            let sink = events.engine;
            app.manage(sink.clone());
            app.manage(events.app);
            let handle = app.handle().clone();
            app.state::<engine_handle::EngineHandle>().start(move || {
                start_engine(
                    handle.clone(),
                    sink.clone(),
                    Arc::clone(&presence),
                )
            });

            #[cfg(target_os = "macos")]
            {
                let payload = macos::deep_link::get_or_init_payload(app);

                let mtx_copy = payload.payload;
                let handle = app.handle().clone();
                app.listen("deep-link://new-url", move |url| {
                    let mtx_copy_copy = mtx_copy.clone();
                    let handle = handle.clone();
                    let request = url.payload().to_owned();

                    let actual_request =
                        serde_json::from_str::<Vec<String>>(&request)
                            .ok()
                            .map(|mut x| x.remove(0))
                            .unwrap_or(request);

                    tauri::async_runtime::spawn(async move {
                        tracing::info!("Handling macOS deep link");

                        let mut payload = mtx_copy_copy.lock().await;
                        if payload.is_none() {
                            *payload = Some(actual_request.clone());
                        }

                        let _ =
                            api::utils::handle_command(handle, actual_request)
                                .await;
                    });
                });
            };

            #[cfg(not(target_os = "macos"))]
            {
                let handle = app.handle().clone();
                app.listen("deep-link://new-url", move |url| {
                    let payload = url.payload().to_owned();
                    tracing::info!("Handling deep link");
                    tauri::async_runtime::spawn(api::utils::handle_command(
                        handle.clone(),
                        payload,
                    ));
                });
            }

            #[cfg(not(target_os = "linux"))]
            if let Some(window) = app.get_window("main")
                && let Err(e) = window.set_shadow(true)
            {
                tracing::warn!("Failed to set window shadow: {e}");
            }

            if let Some(title) = dev::config().map(|config| &config.title)
                && let Some(window) = app.get_window("main")
                && let Err(e) = window.set_title(title)
            {
                tracing::warn!("Failed to set dev window title: {e}");
            }

            Ok(())
        });

    builder = builder
        .plugin(api::logs::init())
        .manage(updater)
        .manage(engine)
        .invoke_handler(tauri::generate_handler![show_window]);

    tracing::info!("Initializing app...");
    let app = builder.build(tauri_context);

    match app {
        Ok(app) => {
            app.run(|app, event| {
                #[cfg(not(any(feature = "updater", target_os = "macos")))]
                let _ = app;

                if matches!(&event, tauri::RunEvent::ExitRequested { .. })
                    && let Some(state) =
                        app.state::<engine_handle::EngineHandle>().try_get()
                    && let Err(error) = tauri::async_runtime::block_on(
                        skins::minecraft_skins::flush_pending_skin_change(
                            &state,
                        ),
                    )
                {
                    tracing::warn!(
                        "Failed to flush pending Minecraft skin change before exit: {error}"
                    );
                }

                #[cfg(feature = "updater")]
                if matches!(&event, tauri::RunEvent::Exit) {
                    let updater =
                        app.state::<Arc<desktop::updater::Updater>>();
                    let should_restart = updater.restarts_after();
                    if let Some((update, data)) = &updater.take_downloaded() {
                        let state = app.state::<engine_handle::EngineHandle>().try_get();
                        fn set_changelog_toast(state: Option<&Engine>, version: Option<String>) {
                            let Some(state) = state else {
                                return;
                            };
                            let toast_result: engine::Result<()> = tauri::async_runtime::block_on(async move {
                                let mut settings = crate::settings::get(state).await?;
                                settings.pending_update_toast_for_version = version;
                                crate::settings::set(state, settings).await?;
                                Ok(())
                            });
                            if let Err(e) = toast_result {
                                tracing::warn!(
                                    "Failed to set pending_update_toast: {e}"
                                )
                            }
                        }

						let current_version = &app.package_info().version;
						let mut version_parts = update.version.split('.');
						let major = version_parts.next().and_then(|part| part.parse::<u64>().ok());
						let minor = version_parts.next().and_then(|part| part.parse::<u64>().ok());
						let is_major_update = major.zip(minor).is_some_and(|version| {
							version > (current_version.major, current_version.minor)
						});
						set_changelog_toast(state.as_deref(), is_major_update.then(|| update.version.clone()));
                        let update = if should_restart {
                            (**update).clone()
                        } else {
                            (**update).clone().restart_after_install(false)
                        };
                        match update.install(data) {
                            Ok(()) => {
                                if should_restart {
                                    tracing::info!(
                                        "Pending update installed successfully (version {}); restarting because user requested reload",
                                        update.version
                                    );
                                    app.restart();
                                } else {
                                    tracing::info!(
                                        "Pending update installed successfully (version {}); exiting without relaunch (user did not request reload)",
                                        update.version
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Pending update install failed (version {}): {e}",
                                    update.version
                                );
                                set_changelog_toast(state.as_deref(), None);

                                DialogBuilder::message()
                                    .set_level(MessageLevel::Error)
                                    .set_title("Update error")
                                    .set_text(format!("Failed to install update due to an error:\n{e}"))
                                    .alert()
                                    .show()
                                    .unwrap();
                            }
                        }
                    }
                }
                #[cfg(target_os = "macos")]
                if let tauri::RunEvent::Opened { urls } = event {
                    tracing::info!("Handling webview open {urls:?}");

                    let file = urls
                        .into_iter()
                        .find_map(|url| url.to_file_path().ok());

                    if let Some(file) = file {
                        let payload =
                            macos::deep_link::get_or_init_payload(app);

                        let mtx_copy = payload.payload;
                        let request = file.to_string_lossy().to_string();
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let mut payload = mtx_copy.lock().await;
                            if payload.is_none() {
                                *payload = Some(request.clone());
                            }

                            let _ =
                                api::utils::handle_command(handle, request)
                                    .await;
                        });
                    }
                }
            });
        }
        Err(e) => {
            tracing::error!("Error while running tauri application: {:?}", e);

            #[cfg(target_os = "windows")]
            {
                // tauri doesn't expose runtime errors, so matching a string representation seems like the only solution
                if format!("{e:?}").contains(
                    "Runtime(CreateWebview(WebView2Error(WindowsError",
                ) {
                    DialogBuilder::message()
                        .set_level(MessageLevel::Error)
                        .set_title("Initialization error")
                        .set_text("Your Microsoft Edge WebView2 installation is corrupt.\n\nMicrosoft Edge WebView2 is required to run Modrinth App.\n\nLearn how to repair it at https://support.modrinth.com/en/articles/8797765-corrupted-microsoft-edge-webview2-installation")
                        .alert()
                        .show()
                        .unwrap();

                    panic!("webview2 initialization failed")
                }
            }

            DialogBuilder::message()
                .set_level(MessageLevel::Error)
                .set_title("Initialization error")
                .set_text(format!(
                    "Cannot initialize application due to an error:\n{e:?}"
                ))
                .alert()
                .show()
                .unwrap();

            panic!("{1}: {:?}", e, "error while running tauri application")
        }
    }
}

#[cfg(test)]
mod tests {
    /// Builds a TLS config the way crates relying on rustls's process-level
    /// provider do (e.g. async-tungstenite for the friends socket), with the
    /// app's full dependency graph. This only fails here, not in app-lib's
    /// own tests, because a conflicting provider feature can come from
    /// unifying app-lib's dependencies with the Tauri plugins'.
    #[test]
    fn rustls_process_provider_is_resolvable() {
        let _ = rustls::ClientConfig::builder();
    }
}
