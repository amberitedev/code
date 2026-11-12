//! Instances' screenshots and the groups they are sorted into.

use std::path::PathBuf;

use contracts::{Empty, Json, Raw};
use engine::instance::{
    ScreenshotEditSaveMode, ScreenshotGroup, ScreenshotGroupImport,
    ScreenshotGroupMembershipUpdate, ScreenshotKey,
};
use serde::{Deserialize, Serialize};

use super::IdParams;

endpoints! {
    ListInstanceScreenshots: Get "/app/instances/{id}/screenshots", IdParams,
        (), Empty => Json<Vec<ScreenshotFile>>;
    ListScreenshots: Get "/app/screenshots", (), (), Empty
        => Json<Vec<ScreenshotFile>>;
    /// The screenshots in the shared screenshots folder.
    ListSyncedScreenshots: Get "/app/screenshots/synced", (), (), Empty
        => Json<Vec<ScreenshotFile>>;
    /// Saves an edit of a screenshot; the body is the edited PNG.
    SaveEditedScreenshot: Post "/app/screenshots/edits", (), ScreenshotEdit,
        Raw => Json<ScreenshotFile>;
    DeleteScreenshots: Delete "/app/screenshots", (), (),
        Json<Vec<ScreenshotKey>> => Empty;
    /// Copies the screenshots into a folder.
    ExportScreenshots: Post "/app/screenshots/export", (), (),
        Json<ScreenshotExport> => Empty;
    /// Moves the screenshots to another instance and returns their new keys.
    MoveScreenshots: Post "/app/screenshots/move", (), (), Json<ScreenshotMove>
        => Json<Vec<ScreenshotKey>>;
    ListScreenshotGroups: Get "/app/screenshot-groups", (), (), Empty
        => Json<Vec<ScreenshotGroup>>;
    CreateScreenshotGroup: Post "/app/screenshot-groups", (), (),
        Json<NewScreenshotGroup> => Json<ScreenshotGroup>;
    RenameScreenshotGroup: Patch "/app/screenshot-groups/{id}", IdParams, (),
        Json<ScreenshotGroupUpdate> => Json<ScreenshotGroup>;
    DeleteScreenshotGroup: Delete "/app/screenshot-groups/{id}", IdParams, (),
        Empty => Empty;
    SetScreenshotGroupMemberships: Put "/app/screenshot-groups/memberships",
        (), (), Json<Vec<ScreenshotGroupMembershipUpdate>> => Empty;
    ImportScreenshotGroups: Post "/app/screenshot-groups/import", (), (),
        Json<Vec<ScreenshotGroupImport>> => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ScreenshotFile {
    pub id: String,
    pub instance_id: String,
    pub instance_name: String,
    pub file_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub modified_at: i64,
    pub group_id: Option<String>,
    pub path: PathBuf,
}

impl From<engine::instance::InstanceScreenshot> for ScreenshotFile {
    fn from(screenshot: engine::instance::InstanceScreenshot) -> Self {
        Self {
            id: screenshot.id,
            instance_id: screenshot.instance_id,
            instance_name: screenshot.instance_name,
            file_name: screenshot.file_name,
            created_at: screenshot.created_at,
            modified_at: screenshot.modified_at,
            group_id: screenshot.group_id,
            path: screenshot.path,
        }
    }
}

/// The screenshot that was edited, and whether the edit replaces it or is saved beside it.
#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ScreenshotEdit {
    pub instance_id: String,
    pub file_name: String,
    pub mode: ScreenshotEditSaveMode,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ScreenshotExport {
    pub keys: Vec<ScreenshotKey>,
    pub export_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ScreenshotMove {
    pub keys: Vec<ScreenshotKey>,
    pub target_instance_id: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct NewScreenshotGroup {
    pub name: String,
    pub screenshot_ids: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ScreenshotGroupUpdate {
    pub name: String,
}
