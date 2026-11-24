use std::collections::HashMap;

use contracts::{Empty, Json};
use modrinth_content_management::ResolveContentPlan;
use path_util::SafeRelativeUtf8UnixPathBuf;
use serde::{Deserialize, Serialize};

use super::{cache::CacheRead, instances::InstanceRef};
use crate::{
    DownloadReason,
    install::InstallJobSnapshot,
    instance::{
        ContentSide, InstallProjectWithDependenciesRequest, PackExportCandidate,
    },
    state::{
        CacheBehaviour, ContentFile, ContentItem, Dependency,
        InstanceInstallCandidate, InstanceInstallTarget, LinkedModpackInfo,
        ProjectType,
    },
};

contracts::endpoints! {
    group instanceContent;

    /// The instance's content files, keyed by their path in the instance.
    ListInstanceProjects: Get "/instances/{id}/projects", Session, InstanceRef, CacheRead, Empty => Json<HashMap<String, ContentFile>>, client_only;
    ListInstalledProjectIds: Get "/instances/{id}/projects/ids", Session, InstanceRef, (), Empty => Json<Vec<String>>, client_only;
    CheckProjectInstalled: Get "/instances/{id}/projects/installed", Session, InstanceRef, ProjectIdQuery, Empty => Json<bool>, client_only;
    /// Which instances could take the project, among the targets.
    ListInstallCandidates: Post "/install-candidates", Session, (), (), Json<InstallCandidatesQuery> => Json<Vec<InstanceInstallCandidate>>, client_only;
    /// The content outside the linked modpack, sorted by name.
    ListContentItems: Get "/instances/{id}/content", Session, InstanceRef, CacheRead, Empty => Json<Vec<ContentItem>>, shared;
    RefreshContentUpdates: Post "/instances/{id}/content/refresh-updates", Session, InstanceRef, (), Empty => Empty, shared;
    /// Indexes content files added or removed on disk.
    SyncContentFiles: Post "/instances/{id}/content/sync", Session, InstanceRef, (), Empty => Empty, client_only;
    ListDependencyContentItems: Post "/content-items/dependencies", Session, (), (), Json<DependencyContentQuery> => Json<Vec<ContentItem>>, client_only;
    /// `null` when the instance follows no modpack.
    GetLinkedModpack: Get "/instances/{id}/modpack", Session, InstanceRef, CacheRead, Empty => Json<Option<LinkedModpackInfo>>, shared;
    ListLinkedModpackContent: Get "/instances/{id}/modpack/content", Session, InstanceRef, CacheRead, Empty => Json<Vec<ContentItem>>, shared;
    /// Answers the new file's path.
    AddProjectFromVersion: Post "/instances/{id}/projects", Session, InstanceRef, (), Json<NewProject> => Json<String>, shared;
    InstallProjectWithDependencies: Post "/instances/{id}/projects/with-dependencies", Session, InstanceRef, (), Json<InstallProjectWithDependenciesRequest> => Json<ResolveContentPlan>, shared;
    /// Answers the new file's path.
    UpdateProject: Post "/instances/{id}/projects/update", Session, InstanceRef, (), Json<ProjectPath> => Json<String>, shared;
    /// Answers the new file's path.
    SwitchProjectVersion: Post "/instances/{id}/projects/switch-version", Session, InstanceRef, (), Json<ProjectVersionSwitch> => Json<String>, shared;
    /// Answers the file's path, which disabling renames.
    ToggleProject: Post "/instances/{id}/projects/toggle", Session, InstanceRef, (), Json<ProjectToggle> => Json<String>, shared;
    SetProjectLocked: Put "/instances/{id}/projects/locked", Session, InstanceRef, (), Json<ProjectLock> => Empty, client_only;
    RemoveProject: Post "/instances/{id}/projects/remove", Session, InstanceRef, (), Json<ProjectPath> => Empty, shared;
    /// Installs a Modrinth modpack version over the instance, which then follows it.
    InstallModpack: Post "/instances/{id}/modpack", Session, InstanceRef, (), Json<ModpackProject> => Json<InstallJobSnapshot>, shared;
    UpdateModpackVersion: Post "/instances/{id}/modpack/update", Session, InstanceRef, (), Json<ModpackVersion> => Json<InstallJobSnapshot>, shared;
    /// Stops following the modpack. Its files stay.
    UnlinkModpack: Post "/instances/{id}/modpack/unlink", Session, InstanceRef, (), Empty => Empty, shared;
    RepairModpack: Post "/instances/{id}/modpack/repair", Session, InstanceRef, (), Empty => Json<InstallJobSnapshot>, shared;
    /// The direct children of `parent` (the instance's folder when absent) an mrpack export can
    /// include.
    ListPackExportCandidates: Get "/instances/{id}/export-candidates", Session, InstanceRef, ExportCandidatesQuery, Empty => Json<Vec<PackExportCandidate>>, client_only;
    /// Turns an addon on or off for the server or for players. Server engines only, as are the
    /// routes below.
    SetSide: Post "/instances/{id}/content/side", Session, InstanceRef, (), Json<SideChange> => Empty, shared;
    SetSidesUnlocked: Post "/instances/{id}/content/sides-unlocked", Session, InstanceRef, (), Json<SidesUnlocked> => Empty, shared;
    CompleteSideReview: Post "/instances/{id}/content/side-review", Session, InstanceRef, (), Empty => Empty, shared;
    /// Installs the followed pack again, placing every file of it that is missing or changed.
    RepairContent: Post "/instances/{id}/content/repair", Session, InstanceRef, (), Empty => Empty, shared;
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
pub struct ModpackProject {
    pub project_id: String,
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
