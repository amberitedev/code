//! What the App does on its own desktop: dialogs, links, folders and shortcuts. A call from a
//! tab acts on the App's desktop too.

use std::path::PathBuf;

use contracts::{Auth, Empty, Endpoint, Json, Method};
use serde::{Deserialize, Serialize};

use crate::server::events::Command;

/// Asks the person to pick files or folders. `None` when they cancel.
pub struct PickFiles;

impl Endpoint for PickFiles {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/dialog/open";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<OpenDialog>;
    type Response = Json<Option<Vec<PathBuf>>>;
}

/// Asks the person where to save a file. `None` when they cancel.
pub struct PickSavePath;

impl Endpoint for PickSavePath {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/dialog/save";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<SaveDialog>;
    type Response = Json<Option<PathBuf>>;
}

/// Opens a web, mail or phone link in the default program.
pub struct OpenUrl;

impl Endpoint for OpenUrl {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/opener/url";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<ExternalLink>;
    type Response = Empty;
}

/// Opens a file or folder in its default program.
pub struct OpenPath;

impl Endpoint for OpenPath {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/opener/path";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<LocalPath>;
    type Response = Empty;
}

/// Shows a file selected in the file manager.
pub struct RevealPath;

impl Endpoint for RevealPath {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/opener/reveal";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<LocalPath>;
    type Response = Empty;
}

/// Opens one of the App's own folders in the file manager.
pub struct OpenFolder;

impl Endpoint for OpenFolder {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/folders/open";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<AppFolder>;
    type Response = Empty;
}

/// Writes a desktop shortcut that launches an instance, and returns where it went: the path
/// gains the platform's shortcut extension.
pub struct CreateShortcut;

impl Endpoint for CreateShortcut {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/shortcuts";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<NewShortcut>;
    type Response = Json<PathBuf>;
}

/// Whether the network connection is metered, so large downloads can wait.
pub struct CheckNetworkMetered;

impl Endpoint for CheckNetworkMetered {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/network/metered";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Json<bool>;
}

/// Lines from the native window's console, in order, written to the App's log.
pub struct LogFromWebview;

impl Endpoint for LogFromWebview {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/webview-log";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<Vec<WebviewLog>>;
    type Response = Empty;
}

/// The file or link the App was opened with, or on macOS the first one it was asked to open.
/// Read it once, when the page is ready to act on it.
pub struct GetOpeningCommand;

impl Endpoint for GetOpeningCommand {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/opening-command";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Json<Option<Command>>;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct OpenDialog {
    pub title: Option<String>,
    /// A folder to start in, or a file name, with or without its folder.
    pub default_path: Option<PathBuf>,
    pub directory: bool,
    pub multiple: bool,
    pub filters: Vec<DialogFilter>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SaveDialog {
    pub title: Option<String>,
    pub default_path: Option<PathBuf>,
    pub filters: Vec<DialogFilter>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct DialogFilter {
    pub name: String,
    /// Without the dot.
    pub extensions: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ExternalLink {
    pub url: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LocalPath {
    pub path: PathBuf,
}

/// A screenshot opens its folder with the file selected.
#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "folder", rename_all = "snake_case")]
pub enum AppFolder {
    LauncherLogs,
    AppDbBackups,
    SyncedOptions,
    Screenshot {
        instance_id: String,
        file_name: String,
    },
}

/// Launches into `server` or `singleplayer_world` when one is given.
#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct NewShortcut {
    pub instance_name: String,
    pub instance_id: String,
    pub output_path: PathBuf,
    pub server: Option<String>,
    pub singleplayer_world: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct WebviewLog {
    pub level: WebviewLogLevel,
    pub message: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum WebviewLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}
