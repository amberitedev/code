//! Serves the dialog routes of `crate::server::endpoints::desktop`.

use std::path::PathBuf;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Json,
    server::{Req, route},
};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};
use tokio::sync::oneshot;

use super::{AppSlot, internal};
use crate::server::endpoints::desktop::{
    DialogFilter, PickFiles, PickSavePath,
};

pub fn routes(app: AppSlot) -> Router {
    let (open_path, open) = route::<PickFiles, _, _, _>(pick_files);
    let (save_path, save) = route::<PickSavePath, _, _, _>(pick_save_path);
    Router::new()
        .route(open_path, open)
        .route(save_path, save)
        .with_state(app)
}

/// The dialog runs on its own thread and answers through the channel, so a person taking their
/// time holds up no other request.
async fn pick_files(
    State(app): State<AppSlot>,
    Req { body, .. }: Req<PickFiles>,
) -> Result<Json<Option<Vec<PathBuf>>>, ApiError> {
    let dialog = file_dialog(
        app.wait().await,
        body.title,
        body.default_path,
        body.filters,
    );
    let (send, picked) = oneshot::channel::<Option<Vec<FilePath>>>();
    match (body.directory, body.multiple) {
        (false, false) => dialog.pick_file(|path| {
            let _ = send.send(path.map(|path| vec![path]));
        }),
        (true, false) => dialog.pick_folder(|path| {
            let _ = send.send(path.map(|path| vec![path]));
        }),
        (false, true) => dialog.pick_files(|paths| {
            let _ = send.send(paths);
        }),
        (true, true) => dialog.pick_folders(|paths| {
            let _ = send.send(paths);
        }),
    }
    let Some(picked) = picked.await.ok().flatten() else {
        return Ok(Json(None));
    };
    picked
        .into_iter()
        .map(|path| path.into_path().map_err(internal))
        .collect::<Result<_, _>>()
        .map(|paths| Json(Some(paths)))
}

async fn pick_save_path(
    State(app): State<AppSlot>,
    Req { body, .. }: Req<PickSavePath>,
) -> Result<Json<Option<PathBuf>>, ApiError> {
    let dialog = file_dialog(
        app.wait().await,
        body.title,
        body.default_path,
        body.filters,
    );
    let (send, picked) = oneshot::channel();
    dialog.save_file(|path| {
        let _ = send.send(path);
    });
    picked
        .await
        .ok()
        .flatten()
        .map(|path| path.into_path().map_err(internal))
        .transpose()
        .map(Json)
}

fn file_dialog(
    app: &AppHandle,
    title: Option<String>,
    default_path: Option<PathBuf>,
    filters: Vec<DialogFilter>,
) -> FileDialogBuilder<tauri::Wry> {
    let mut dialog = app.dialog().file();
    // A dialog parented to the hidden window of a dev run never shows.
    #[cfg(any(windows, target_os = "macos"))]
    if let Some(window) = app
        .get_window("main")
        .filter(|window| window.is_visible().unwrap_or(false))
    {
        dialog = dialog.set_parent(&window);
    }
    if let Some(title) = title {
        dialog = dialog.set_title(title);
    }
    if let Some(path) = default_path {
        // A Windows dialog does not read forward slashes.
        let path: PathBuf = path.components().collect();
        if path.is_dir() {
            dialog = dialog.set_directory(path);
        } else {
            if let Some(folder) = path
                .parent()
                .filter(|folder| folder.components().next().is_some())
            {
                dialog = dialog.set_directory(folder);
            }
            if let Some(name) = path.file_name() {
                dialog = dialog.set_file_name(name.to_string_lossy());
            }
        }
    }
    for filter in &filters {
        let extensions: Vec<&str> =
            filter.extensions.iter().map(String::as_str).collect();
        dialog = dialog.add_filter(&filter.name, &extensions);
    }
    dialog
}
