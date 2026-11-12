//! Theseus state management system
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::install::InstallJobSnapshot;

pub mod emit;

/// Log lines are an event each, so the buffer absorbs a burst before a slow subscriber lags.
const EVENT_CAPACITY: usize = 4096;

/// Where an engine sends its events. The host creates it and subscribes before the engine
/// starts, so startup events reach it; with no subscriber, events are dropped.
#[derive(Clone)]
pub struct EventSink {
    tx: broadcast::Sender<Event>,
    loading_bars: Arc<DashMap<Uuid, LoadingBar>>,
    #[cfg(feature = "tauri")]
    pub(crate) app: Option<tauri::AppHandle>,
}

impl EventSink {
    pub fn new() -> Self {
        Self {
            tx: broadcast::Sender::new(EVENT_CAPACITY),
            loading_bars: Arc::new(DashMap::new()),
            #[cfg(feature = "tauri")]
            app: None,
        }
    }

    /// The same sink, with the App's handle for the main window.
    #[cfg(feature = "tauri")]
    pub fn with_app(mut self, app: tauri::AppHandle) -> Self {
        self.app = Some(app);
        self
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    pub(crate) fn send(&self, event: Event) {
        let _ = self.tx.send(event);
    }

    // Values provided should not be used directly, as they are clones and are not guaranteed to be up-to-date
    pub fn loading_bars(&self) -> DashMap<Uuid, LoadingBar> {
        (*self.loading_bars).clone()
    }

    /// The ads child webview makes the main window a multi-webview window,
    /// so Tauri's single-webview window lookup no longer returns it.
    #[cfg(feature = "tauri")]
    pub fn get_main_window(&self) -> Option<tauri::Window> {
        use tauri::Manager;
        self.app.as_ref()?.get_window("main")
    }
}

impl Default for EventSink {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for EventSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventSink").finish_non_exhaustive()
    }
}

#[derive(Clone, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
#[cfg_attr(
    feature = "export-ts",
    ts(
        rename = "EngineEvent",
        tag = "type",
        content = "payload",
        rename_all = "snake_case"
    )
)]
pub enum Event {
    Loading(LoadingPayload),
    Process(ProcessPayload),
    Instance(InstancePayload),
    InstallJob(std::sync::Arc<InstallJobSnapshot>),
    Warning(WarningPayload),
    Log(LogPayload),
    AdsConsentRequired(bool),
    /// Raised by desktop code that still lives in the engine; the App sends it as its own event.
    /// It does not serialize, so every socket loop maps or drops it. It goes once the desktop code
    /// raising it lives in apps/app.
    #[serde(skip)]
    Desktop(DesktopEvent),
}

#[derive(Clone)]
pub enum DesktopEvent {
    Friend(FriendPayload),
    Notification(serde_json::Value),
    OnboardingChecklist(crate::state::OnboardingChecklist),
    InstanceGroupsChanged(InstanceGroupsChangedPayload),
    Command(CommandPayload),
    GameOptionLocalesUpdated,
}

#[derive(Serialize, Debug, Clone)]
pub struct LoadingBar {
    // loading_bar_uuid not be used directly by external functions as it may not reflect the current state of the loading bar/hashmap
    pub loading_bar_uuid: Uuid,
    pub message: String,
    pub total: f64,
    pub current: f64,
    #[serde(skip)]
    pub last_sent: f64,
    pub bar_type: LoadingBarType,
}

#[derive(Debug)]
pub struct LoadingBarId(Uuid, EventSink);

// When Loading bar id is dropped, we should remove it from the hashmap
impl Drop for LoadingBarId {
    fn drop(&mut self) {
        let Self(loader_uuid, events) = self;
        if let Some((_, bar)) = events.loading_bars.remove(loader_uuid) {
            let fraction = bar.current / bar.total;
            events.send(Event::Loading(LoadingPayload {
                fraction: None,
                message: "Completed".to_string(),
                event: bar.bar_type,
                loader_uuid: loader_uuid.to_string(),
            }));
            tracing::trace!(
                "Exited at {fraction} for loading bar: {:?}",
                loader_uuid
            );
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-ts", ts(tag = "type", rename_all = "snake_case"))]
pub enum LoadingBarType {
    LegacyDataMigration,
    DirectoryMove {
        old: String,
        new: String,
    },
    JavaDownload {
        version: u32,
    },
    PackFileDownload {
        instance_id: String,
        pack_name: String,
        icon: Option<String>,
        pack_version: String,
    },
    PackDownload {
        instance_id: String,
        pack_name: String,
        icon: Option<String>,
        pack_id: Option<String>,
        pack_version: Option<String>,
    },
    PackImport {
        pack_name: String,
    },
    MinecraftDownload {
        instance_id: String,
        instance_name: String,
    },
    ZipExtract {
        instance_id: String,
        instance_name: String,
    },
    PackExport {
        instance_id: String,
        instance_name: String,
    },
    ConfigChange {
        new_path: String,
    },
    CopyInstance {
        import_location: String,
        instance_name: String,
    },
    LauncherUpdate {
        version: String,
        current_version: String,
    },
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct LoadingPayload {
    pub event: LoadingBarType,
    pub loader_uuid: String,
    pub fraction: Option<f64>, // by convention, if optional, it means the loading is done
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct WarningPayload {
    pub message: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "event")]
#[cfg_attr(feature = "export-ts", ts(tag = "event"))]
pub enum CommandPayload {
    InstallMod {
        id: String,
    },
    InstallVersion {
        id: String,
    },
    InstallModpack {
        id: String,
    },
    InstallServer {
        id: String,
    },
    LaunchInstance {
        id: String,
        server: Option<String>,
        singleplayer_world: Option<String>,
    },
    InstallSharedInstanceInvite {
        invite_id: String,
    },
    RunMRPack {
        // run or install .mrpack
        path: String,
    },
    PlayHostingServer {
        server_id: String,
        world_id: String,
    },
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ProcessPayload {
    pub instance_id: String,
    pub uuid: String,
    pub event: ProcessPayloadType,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ProcessPayloadType {
    Launched,
    Finished,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstancePayload {
    pub instance_id: String,
    #[serde(flatten)]
    #[cfg_attr(feature = "export-ts", ts(flatten))]
    pub event: InstancePayloadType,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceGroupsChangedPayload {
    pub instance_ids: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "event", rename_all = "snake_case")]
#[cfg_attr(feature = "export-ts", ts(tag = "event", rename_all = "snake_case"))]
pub enum InstancePayloadType {
    Created,
    Synced,
    ServersUpdated,
    ScreenshotsUpdated,
    WorldUpdated {
        world: String,
    },
    ServerJoined {
        host: String,
        port: u16,
        timestamp: String,
    },
    Edited,
    ContentInstallFinished {
        project_ids: Vec<String>,
    },
    ContentInstallFailed {
        project_ids: Vec<String>,
        message: String,
    },
    Removed,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
#[serde(tag = "event")]
#[cfg_attr(feature = "export-ts", ts(tag = "event", rename_all = "snake_case"))]
pub enum FriendPayload {
    FriendRequest { from: String },
    UserOffline { id: String },
    StatusUpdate { user_status: FriendStatusPayload },
    StatusSync,
    FriendRequestRejected { from: String },
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct FriendStatusPayload {
    pub user_id: String,
    pub profile_name: Option<String>,
    pub last_update: String,
}

impl From<ariadne::users::UserStatus> for FriendStatusPayload {
    fn from(status: ariadne::users::UserStatus) -> Self {
        Self {
            user_id: status.user_id.to_string(),
            profile_name: status.profile_name,
            last_update: status.last_update.to_rfc3339(),
        }
    }
}

pub use self::log_types::*;

mod log_types {
    use crate::state::Log4jEvent;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
    #[serde(tag = "type", rename_all = "snake_case")]
    #[cfg_attr(
        feature = "export-ts",
        ts(tag = "type", rename_all = "snake_case")
    )]
    pub enum LogEvent {
        Log4j(Log4jEvent),
        Legacy { message: String },
    }

    #[derive(Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
    pub struct LogPayload {
        pub instance_id: String,
        #[serde(flatten)]
        #[cfg_attr(feature = "export-ts", ts(flatten))]
        pub event: LogEvent,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("Non-existent loading bar of key: {0}")]
    NoLoadingBar(Uuid),
}

#[cfg(test)]
mod tests {
    use super::emit::{emit_instance, emit_loading, init_loading};
    use super::*;

    #[tokio::test]
    async fn loading_bars_need_no_subscriber_and_events_reach_one() {
        let events = EventSink::new();
        let bar = init_loading(
            &events,
            LoadingBarType::LegacyDataMigration,
            1.0,
            "Loading",
        )
        .await
        .unwrap();
        emit_loading(&bar, 1.0, None).unwrap();
        drop(bar);
        assert!(events.loading_bars().is_empty());

        let mut received = events.subscribe();
        emit_instance(&events, "a", InstancePayloadType::Edited)
            .await
            .unwrap();
        assert!(matches!(
            received.try_recv(),
            Ok(Event::Instance(InstancePayload { instance_id, .. })) if instance_id == "a"
        ));
    }
}
