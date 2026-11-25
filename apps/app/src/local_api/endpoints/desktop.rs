//! What the App does on its own desktop: dialogs, links, folders and shortcuts. A call from a
//! tab acts on the App's desktop too.

use std::path::PathBuf;

use contracts::{Empty, Json};
use engine::data::JavaVersion;
use serde::{Deserialize, Serialize};

use crate::local_api::events::Command;

contracts::endpoints! {
    group desktop;

    /// Asks the person to pick files or folders. `None` when they cancel.
    PickFiles: Post "/app/dialog/open", None, (), (), Json<OpenDialog> => Json<Option<Vec<PathBuf>>>;
    /// Asks the person where to save a file. `None` when they cancel.
    PickSavePath: Post "/app/dialog/save", None, (), (), Json<SaveDialog> => Json<Option<PathBuf>>;
    /// Opens a web, mail or phone link in the default program.
    OpenUrl: Post "/app/opener/url", None, (), (), Json<ExternalLink> => Empty;
    /// Opens a file or folder in its default program.
    OpenPath: Post "/app/opener/path", None, (), (), Json<LocalPath> => Empty;
    /// Shows a file selected in the file manager.
    RevealPath: Post "/app/opener/reveal", None, (), (), Json<LocalPath> => Empty;
    /// Opens one of the App's own folders in the file manager.
    OpenFolder: Post "/app/folders/open", None, (), (), Json<AppFolder> => Empty;
    /// Writes a desktop shortcut that launches an instance, and returns where it went: the path
    /// gains the platform's shortcut extension.
    CreateShortcut: Post "/app/shortcuts", None, (), (), Json<NewShortcut> => Json<PathBuf>;
    /// Whether the network connection is metered, so large downloads can wait.
    CheckNetworkMetered: Get "/app/network/metered", None, (), (), Empty => Json<bool>;
    /// Lines from the native window's console, in order, written to the App's log.
    LogFromWebview: Post "/app/webview-log", None, (), (), Json<Vec<WebviewLog>> => Empty;
    /// The file or link the App was opened with, or on macOS the first one it was asked to open.
    /// Read it once, when the page is ready to act on it.
    GetOpeningCommand: Get "/app/opening-command", None, (), (), Empty => Json<Option<Command>>;
    /// The Java at a path the person gave, which fails when it is not one.
    GetJavaAt: Get "/app/java", None, (), LocalPath, Empty => Json<JavaAtPath>;
    /// Whether the path is a Java of this major version.
    TestJavaAt: Get "/app/java/test", None, (), JavaTest, Empty => Json<bool>;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct OpenDialog {
    pub title: Option<String>,
    /// A folder to start in, or a file name, with or without its folder.
    pub default_path: Option<PathBuf>,
    pub directory: bool,
    pub multiple: bool,
    pub filters: Vec<DialogFilter>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SaveDialog {
    pub title: Option<String>,
    pub default_path: Option<PathBuf>,
    pub filters: Vec<DialogFilter>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct DialogFilter {
    pub name: String,
    /// Without the dot.
    pub extensions: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ExternalLink {
    pub url: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct LocalPath {
    pub path: PathBuf,
}

/// A screenshot opens its folder with the file selected.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
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
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewShortcut {
    pub instance_name: String,
    pub instance_id: String,
    pub output_path: PathBuf,
    pub server: Option<String>,
    pub singleplayer_world: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct WebviewLog {
    pub level: WebviewLogLevel,
    pub message: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum WebviewLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct JavaTest {
    pub path: PathBuf,
    pub major_version: u32,
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct JavaAtPath(
    #[cfg_attr(feature = "export-ts", ts(type = "JavaVersion"))] pub JavaVersion,
);
