use std::collections::HashMap;

use contracts::{Empty, Json};
use modrinth_content_management::ResolveContentPlan;
use path_util::SafeRelativeUtf8UnixPathBuf;
use serde::{Deserialize, Serialize};

use super::{cache::CacheRead, endpoints, instances::InstanceRef};
use crate::{
    DownloadReason,
    install::InstallJobSnapshot,
    instance::{
        ContentSide, EditedFile, InstallProjectWithDependenciesRequest,
        PackExportCandidate,
    },
    state::{
        CacheBehaviour, ContentFile, ContentItem, Dependency,
        InstanceInstallCandidate, InstanceInstallTarget, LinkedModpackInfo,
        ProjectType,
    },
};

endpoints! {
    /// The instance's content files, keyed by their path in the instance.
    ListInstanceProjects: Get "/instances/{id}/projects", InstanceRef, CacheRead, Empty => Json<HashMap<String, ContentFile>>;
    ListInstalledProjectIds: Get "/instances/{id}/projects/ids", InstanceRef, (), Empty => Json<Vec<String>>;
    CheckProjectInstalled: Get "/instances/{id}/projects/installed", InstanceRef, ProjectIdQuery, Empty => Json<bool>;
    /// Which instances could take the project, among the targets.
    ListInstallCandidates: Post "/install-candidates", (), (), Json<InstallCandidatesQuery> => Json<Vec<InstanceInstallCandidate>>;
    /// The content outside the linked modpack, sorted by name.
    ListContentItems: Get "/instances/{id}/content", InstanceRef, CacheRead, Empty => Json<Vec<ContentItem>>;
    RefreshContentUpdates: Post "/instances/{id}/content/refresh-updates", InstanceRef, (), Empty => Empty;
    /// Indexes content files added or removed on disk.
    SyncContentFiles: Post "/instances/{id}/content/sync", InstanceRef, (), Empty => Empty;
    ListDependencyContentItems: Post "/content-items/dependencies", (), (), Json<DependencyContentQuery> => Json<Vec<ContentItem>>;
    /// `null` when the instance follows no modpack.
    GetLinkedModpack: Get "/instances/{id}/modpack", InstanceRef, CacheRead, Empty => Json<Option<LinkedModpackInfo>>;
    ListLinkedModpackContent: Get "/instances/{id}/modpack/content", InstanceRef, CacheRead, Empty => Json<Vec<ContentItem>>;
    /// Answers the new file's path.
    AddProjectFromVersion: Post "/instances/{id}/projects", InstanceRef, (), Json<NewProject> => Json<String>;
    InstallProjectWithDependencies: Post "/instances/{id}/projects/with-dependencies", InstanceRef, (), Json<InstallProjectWithDependenciesRequest> => Json<ResolveContentPlan>;
    /// Answers the new file's path.
    UpdateProject: Post "/instances/{id}/projects/update", InstanceRef, (), Json<ProjectPath> => Json<String>;
    /// Answers the new file's path.
    SwitchProjectVersion: Post "/instances/{id}/projects/switch-version", InstanceRef, (), Json<ProjectVersionSwitch> => Json<String>;
    /// Answers the file's path, which disabling renames.
    ToggleProject: Post "/instances/{id}/projects/toggle", InstanceRef, (), Json<ProjectToggle> => Json<String>;
    SetProjectLocked: Put "/instances/{id}/projects/locked", InstanceRef, (), Json<ProjectLock> => Empty;
    RemoveProject: Post "/instances/{id}/projects/remove", InstanceRef, (), Json<ProjectPath> => Empty;
    UpdateModpackVersion: Post "/instances/{id}/modpack/update", InstanceRef, (), Json<ModpackVersion> => Json<InstallJobSnapshot>;
    RepairModpack: Post "/instances/{id}/modpack/repair", InstanceRef, (), Empty => Json<InstallJobSnapshot>;
    /// The direct children of `parent` (the instance's folder when absent) an mrpack export can
    /// include.
    ListPackExportCandidates: Get "/instances/{id}/export-candidates", InstanceRef, ExportCandidatesQuery, Empty => Json<Vec<PackExportCandidate>>;
    /// Turns an addon on or off for the server or for players. Server engines only, as are the
    /// routes below.
    SetSide: Post "/instances/{id}/content/side", InstanceRef, (), Json<SideChange> => Empty;
    SetSidesUnlocked: Post "/instances/{id}/content/sides-unlocked", InstanceRef, (), Json<SidesUnlocked> => Empty;
    CompleteSideReview: Post "/instances/{id}/content/side-review", InstanceRef, (), Empty => Empty;
    /// Places every file of the followed pack that is missing.
    RepairContent: Post "/instances/{id}/content/repair", InstanceRef, (), Empty => Empty;
    /// The pack files the owner edited or deleted, and what the last pack update wants for each.
    EditedFiles: Get "/instances/{id}/content/edited", InstanceRef, (), Empty => Json<Vec<EditedFile>>;
    DecideEdited: Post "/instances/{id}/content/edited", InstanceRef, (), Json<EditedDecision> => Empty;
    /// Places a client-only file of the pack on the server.
    PlaceClientOnly: Post "/instances/{id}/content/client-only", InstanceRef, (), Json<ProjectPath> => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ProjectIdQuery {
    pub project_id: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstallCandidatesQuery {
    pub project_id: String,
    pub project_type: ProjectType,
    pub targets: Vec<InstanceInstallTarget>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DependencyContentQuery {
    pub dependencies: Vec<Dependency>,
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub cache_behaviour: Option<CacheBehaviour>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewProject {
    pub version_id: String,
    pub reason: DownloadReason,
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub dependent_on_version_id: Option<String>,
}

/// A content file's path in the instance, such as `mods/sodium.jar`.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ProjectPath {
    pub project_path: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ProjectVersionSwitch {
    pub project_path: String,
    pub version_id: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ProjectToggle {
    pub project_path: String,
    /// Flips the current state when absent.
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub desired_enabled: Option<bool>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ProjectLock {
    pub project_path: String,
    pub locked: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ModpackVersion {
    pub version_id: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ExportCandidatesQuery {
    #[cfg_attr(feature = "export-ts", ts(as = "Option<String>", optional))]
    pub parent: Option<SafeRelativeUtf8UnixPathBuf>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SideChange {
    pub project_path: String,
    pub side: ContentSide,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SidesUnlocked {
    pub project_path: String,
    pub unlocked: bool,
}

/// Keep the owner's version of an edited file, or `replace` it with the pack's.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct EditedDecision {
    pub path: String,
    pub replace: bool,
}
