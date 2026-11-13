use anyhow::Context;
use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use super::endpoints;
use crate::{
    ErrorKind,
    state::{
        AppliedContentSetPatch, ContentSetSyncStatus,
        EditInstance as CoreEditInstance, Hooks, IconConfig,
        InstanceInstallStage, InstanceLaunchOverridesPatch,
        InstanceLink as CoreInstanceLink, InstanceMetadata,
        InstanceSyncedOptions, InstanceTabVisibility, JavaVersion,
        MemorySettings, ModLoader, ReleaseChannel,
        SharedInstanceAttachment as CoreSharedInstanceAttachment,
        SharedInstanceRole, WindowSize,
    },
};

endpoints! {
    /// Every instance the caller may view.
    ListInstances: Get "/instances", (), (), Empty => Json<Vec<Instance>>;
    /// The instances among `ids` that exist and the caller may view.
    GetInstances: Get "/instances/many", (), InstanceIds, Empty => Json<Vec<Instance>>;
    /// `null` when there is no such instance.
    GetInstance: Get "/instances/{id}", InstanceRef, (), Empty => Json<Option<Instance>>;
    /// Deletes the instance and its folder.
    RemoveInstance: Delete "/instances/{id}", InstanceRef, (), Empty => Empty;
    EditInstance: Patch "/instances/{id}", InstanceRef, (), Json<InstanceUpdate> => Empty;
    /// Stops the instance's running game.
    KillInstance: Post "/instances/{id}/kill", InstanceRef, (), Empty => Empty;
    /// The installed Java that suits the instance's game version, if any.
    GetOptimalJava: Get "/instances/{id}/java", InstanceRef, (), Empty => Json<Option<JavaVersion>>;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceRef {
    pub id: String,
}

/// The fields to change; an absent field stays. On a field that can be unset, `null` unsets it.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceUpdate {
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub name: Option<String>,
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub game_version: Option<String>,
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub loader: Option<ModLoader>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    #[cfg_attr(
        feature = "export-ts",
        ts(as = "Option<Option<String>>", optional)
    )]
    pub loader_version: Option<Option<String>>,
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub group_ids: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    #[cfg_attr(
        feature = "export-ts",
        ts(as = "Option<Option<InstanceLink>>", optional)
    )]
    pub link: Option<Option<InstanceLink>>,
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub update_channel: Option<ReleaseChannel>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    #[cfg_attr(
        feature = "export-ts",
        ts(as = "Option<Option<String>>", optional)
    )]
    pub java_path: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    #[cfg_attr(
        feature = "export-ts",
        ts(as = "Option<Option<Vec<String>>>", optional)
    )]
    pub extra_launch_args: Option<Option<Vec<String>>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    #[cfg_attr(
        feature = "export-ts",
        ts(as = "Option<Option<Vec<(String, String)>>>", optional)
    )]
    pub custom_env_vars: Option<Option<Vec<(String, String)>>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    #[cfg_attr(
        feature = "export-ts",
        ts(as = "Option<Option<MemorySettings>>", optional)
    )]
    pub memory: Option<Option<MemorySettings>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    #[cfg_attr(
        feature = "export-ts",
        ts(as = "Option<Option<bool>>", optional)
    )]
    pub force_fullscreen: Option<Option<bool>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    #[cfg_attr(
        feature = "export-ts",
        ts(as = "Option<Option<WindowSize>>", optional)
    )]
    pub game_resolution: Option<Option<WindowSize>>,
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub hooks: Option<Hooks>,
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub visible_tabs: Option<InstanceTabVisibility>,
}

impl InstanceUpdate {
    pub fn into_core(self) -> crate::Result<CoreEditInstance> {
        Ok(CoreEditInstance {
            install_stage: None,
            launcher_feature_version: None,
            name: self.name,
            icon_path: None,
            icon_config: None,
            update_channel: self.update_channel,
            group_ids: self.group_ids,
            link: self
                .link
                .map(|link| match link {
                    Some(link) => link.into_core(),
                    None => Ok(CoreInstanceLink::Unmanaged),
                })
                .transpose()?,
            launch_overrides: Some(InstanceLaunchOverridesPatch {
                java_path: self.java_path,
                extra_launch_args: self.extra_launch_args,
                custom_env_vars: self.custom_env_vars,
                memory: self.memory,
                force_fullscreen: self.force_fullscreen,
                game_resolution: self.game_resolution,
                hooks: self.hooks,
                visible_tabs: self.visible_tabs,
            }),
            content_set_patch: Some(AppliedContentSetPatch {
                source_kind: None,
                game_version: self.game_version,
                protocol_version: Some(None),
                loader: self.loader,
                loader_version: self.loader_version,
            }),
            last_played: None,
            submitted_time_played: None,
            recent_time_played: None,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceIds {
    /// A JSON array of ids.
    pub ids: String,
}

/// An instance as the frontend reads it.
#[derive(Serialize, Debug, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(rename = "GameInstance"))]
pub struct Instance {
    pub id: String,
    pub path: String,
    pub install_stage: InstanceInstallStage,
    pub launcher_feature_version: String,
    pub name: String,
    pub icon_path: Option<String>,
    pub icon_config: Option<IconConfig>,
    pub game_version: String,
    pub protocol_version: Option<u32>,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub group_ids: Vec<String>,
    pub synced_options: InstanceSyncedOptions,
    pub link: Option<InstanceLink>,
    pub shared_instance: Option<SharedInstanceAttachment>,
    pub quarantined: bool,
    pub update_channel: ReleaseChannel,
    pub created: chrono::DateTime<chrono::Utc>,
    pub modified: chrono::DateTime<chrono::Utc>,
    pub last_played: Option<chrono::DateTime<chrono::Utc>>,
    pub submitted_time_played: u64,
    pub recent_time_played: u64,
    pub java_path: Option<String>,
    pub extra_launch_args: Option<Vec<String>>,
    pub custom_env_vars: Option<Vec<(String, String)>>,
    pub memory: Option<MemorySettings>,
    pub force_fullscreen: Option<bool>,
    pub game_resolution: Option<WindowSize>,
    pub hooks: Hooks,
    pub visible_tabs: InstanceTabVisibility,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstanceLink {
    ModrinthModpack {
        project_id: String,
        version_id: String,
    },
    ServerProject {
        project_id: String,
    },
    ServerProjectModpack {
        server_project_id: String,
        content_project_id: Option<String>,
        content_version_id: String,
        project_id: Option<String>,
        version_id: Option<String>,
    },
    ImportedModpack {
        project_id: Option<String>,
        version_id: Option<String>,
        name: Option<String>,
        version_number: Option<String>,
        filename: Option<String>,
    },
    ModrinthHosting {
        server_id: String,
        instance_ids: Vec<String>,
        active_instance_id: Option<String>,
    },
    SharedInstance {
        modpack_project_id: Option<String>,
        modpack_version_id: Option<String>,
    },
}

#[derive(Serialize, Debug, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SharedInstanceAttachment {
    pub id: String,
    pub role: SharedInstanceRole,
    pub manager_id: Option<String>,
    pub server_manager_name: Option<String>,
    pub server_manager_icon_url: Option<String>,
    pub linked_user_id: Option<String>,
    pub status: ContentSetSyncStatus,
    pub applied_version: Option<i32>,
    pub latest_version: Option<i32>,
    pub restored_from_remote: bool,
}

impl From<CoreSharedInstanceAttachment> for SharedInstanceAttachment {
    fn from(attachment: CoreSharedInstanceAttachment) -> Self {
        Self {
            id: attachment.id.to_string(),
            role: attachment.role,
            manager_id: attachment.manager_id,
            server_manager_name: attachment.server_manager_name,
            server_manager_icon_url: attachment.server_manager_icon_url,
            linked_user_id: attachment.linked_user_id,
            status: attachment.status,
            applied_version: attachment.applied_version,
            latest_version: attachment.latest_version,
            restored_from_remote: attachment.restored_from_remote,
        }
    }
}

impl From<InstanceMetadata> for Instance {
    fn from(metadata: InstanceMetadata) -> Self {
        Self {
            id: metadata.instance.id,
            path: metadata.instance.path,
            install_stage: metadata.instance.install_stage,
            launcher_feature_version: metadata
                .instance
                .launcher_feature_version
                .as_str()
                .to_string(),
            name: metadata.instance.name,
            icon_path: metadata.instance.icon_path,
            icon_config: metadata.icon_config,
            game_version: metadata.applied_content_set.game_version,
            protocol_version: metadata.applied_content_set.protocol_version,
            loader: metadata.applied_content_set.loader,
            loader_version: metadata.applied_content_set.loader_version,
            group_ids: metadata.group_ids,
            synced_options: metadata.synced_options,
            link: InstanceLink::from_core(metadata.link),
            shared_instance: metadata.shared_instance.map(Into::into),
            quarantined: metadata.quarantined,
            update_channel: metadata.instance.update_channel,
            created: metadata.instance.created,
            modified: metadata.instance.modified,
            last_played: metadata.instance.last_played,
            submitted_time_played: metadata.instance.submitted_time_played,
            recent_time_played: metadata.instance.recent_time_played,
            java_path: metadata.launch_overrides.java_path,
            extra_launch_args: metadata.launch_overrides.extra_launch_args,
            custom_env_vars: metadata.launch_overrides.custom_env_vars,
            memory: metadata.launch_overrides.memory,
            force_fullscreen: metadata.launch_overrides.force_fullscreen,
            game_resolution: metadata.launch_overrides.game_resolution,
            hooks: metadata.launch_overrides.hooks,
            visible_tabs: metadata.launch_overrides.visible_tabs,
        }
    }
}

impl InstanceLink {
    pub fn from_core(link: CoreInstanceLink) -> Option<Self> {
        match link {
            CoreInstanceLink::Unmanaged => None,
            CoreInstanceLink::ModrinthModpack {
                project_id,
                version_id,
            } => Some(Self::ModrinthModpack {
                project_id,
                version_id,
            }),
            CoreInstanceLink::ServerProject { project_id } => {
                Some(Self::ServerProject { project_id })
            }
            CoreInstanceLink::ServerProjectModpack {
                server_project_id,
                content_project_id,
                content_version_id,
            } => Some(Self::ServerProjectModpack {
                project_id: Some(server_project_id.clone()),
                version_id: Some(content_version_id.clone()),
                server_project_id,
                content_project_id: Some(content_project_id),
                content_version_id,
            }),
            CoreInstanceLink::ImportedModpack {
                project_id,
                version_id,
                name,
                version_number,
                filename,
            } => Some(Self::ImportedModpack {
                project_id,
                version_id,
                name,
                version_number,
                filename,
            }),
            CoreInstanceLink::ModrinthHosting {
                server_id,
                instance_ids,
                active_instance_id,
            } => Some(Self::ModrinthHosting {
                server_id: server_id.to_string(),
                instance_ids: instance_ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
                active_instance_id: active_instance_id.map(|id| id.to_string()),
            }),
            CoreInstanceLink::SharedInstance {
                modpack_project_id,
                modpack_version_id,
            } => Some(Self::SharedInstance {
                modpack_project_id,
                modpack_version_id,
            }),
        }
    }

    pub fn into_core(self) -> crate::Result<CoreInstanceLink> {
        match self {
            Self::ModrinthModpack {
                project_id,
                version_id,
            } => Ok(CoreInstanceLink::ModrinthModpack {
                project_id,
                version_id,
            }),
            Self::ServerProject { project_id } => {
                Ok(CoreInstanceLink::ServerProject { project_id })
            }
            Self::ServerProjectModpack {
                server_project_id,
                content_project_id,
                content_version_id,
                ..
            } => Ok(CoreInstanceLink::ServerProjectModpack {
                server_project_id,
                content_project_id: content_project_id.unwrap_or_default(),
                content_version_id,
            }),
            Self::ImportedModpack {
                project_id,
                version_id,
                name,
                version_number,
                filename,
            } => Ok(CoreInstanceLink::ImportedModpack {
                project_id,
                version_id,
                name,
                version_number,
                filename,
            }),
            Self::ModrinthHosting {
                server_id,
                instance_ids,
                active_instance_id,
            } => Ok(CoreInstanceLink::ModrinthHosting {
                server_id: server_id.parse().context(ErrorKind::InputError(
                    "invalid server id".to_string(),
                ))?,
                instance_ids: instance_ids
                    .into_iter()
                    .map(|id| {
                        id.parse().context(ErrorKind::InputError(
                            "invalid hosted instance id".to_string(),
                        ))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?,
                active_instance_id: active_instance_id
                    .map(|id| {
                        id.parse().context(ErrorKind::InputError(
                            "invalid active instance id".to_string(),
                        ))
                    })
                    .transpose()?,
            }),
            Self::SharedInstance {
                modpack_project_id,
                modpack_version_id,
            } => Ok(CoreInstanceLink::SharedInstance {
                modpack_project_id,
                modpack_version_id,
            }),
        }
    }
}
