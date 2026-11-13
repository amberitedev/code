//! Installs that read a file on this desktop, a modpack or an icon, so a Core never serves them.

use std::path::PathBuf;

use contracts::Json;
use engine::{
    data::{IconConfig, ModLoader},
    endpoints::instances::InstanceLink,
    install::InstallJobSnapshot,
    pack::install_from::{CreatePackInstance, CreatePackLocation},
};
use serde::{Deserialize, Serialize};

use super::IdParams;

endpoints! {
    /// What installing the modpack would make, without installing it.
    PreviewModpack: Post "/app/install/modpack-preview", (), (),
        Json<CreatePackLocation> => Json<InstallModpackPreview>;
    CreateInstance: Post "/app/install/instances", (), (),
        Json<InstallCreateInstanceRequest> => Json<InstallJobSnapshot>;
    CreateModpackInstance: Post "/app/install/modpack-instances", (), (),
        Json<PackInstall> => Json<InstallJobSnapshot>;
    /// Replaces the instance's loader and content with the modpack's.
    InstallPackToInstance: Post "/app/install/instances/{id}/pack", IdParams,
        (), Json<PackInstall> => Json<InstallJobSnapshot>;
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct InstallModpackPreview {
    pub name: String,
    pub game_version: String,
    pub modloader: ModLoader,
    pub loader_version: Option<String>,
    pub icon: Option<PathBuf>,
    pub icon_url: Option<String>,
    pub link: Option<InstanceLink>,
    /// Modrinth does not know the file.
    pub unknown_file: bool,
    pub external_files_in_modpack: Vec<String>,
}

impl From<CreatePackInstance> for InstallModpackPreview {
    fn from(pack: CreatePackInstance) -> Self {
        Self {
            name: pack.name,
            game_version: pack.game_version,
            modloader: pack.modloader,
            loader_version: pack.loader_version,
            icon: pack.icon,
            icon_url: pack.icon_url,
            link: pack.link.and_then(InstanceLink::from_core),
            unknown_file: pack.unknown_file,
            external_files_in_modpack: pack.external_files_in_modpack,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct InstallCreateInstanceRequest {
    pub name: String,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub icon_path: Option<String>,
    #[cfg_attr(test, ts(optional = nullable))]
    pub icon_config: Option<IconConfig>,
    #[cfg_attr(test, ts(optional = nullable))]
    pub link: Option<InstanceLink>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PackInstall {
    pub location: CreatePackLocation,
    #[cfg_attr(test, ts(optional = nullable))]
    pub post_install_edit: Option<InstallPostInstallEdit>,
}

/// Applied to the instance once the modpack is installed.
#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct InstallPostInstallEdit {
    #[cfg_attr(test, ts(optional = nullable))]
    pub name: Option<String>,
    /// Absent keeps the pack's icon; `null` removes it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    #[cfg_attr(test, ts(as = "Option<Option<String>>", optional))]
    pub icon_path: Option<Option<String>>,
    #[cfg_attr(test, ts(optional = nullable))]
    pub link: Option<InstanceLink>,
}

impl InstallPostInstallEdit {
    pub fn into_core(
        self,
    ) -> engine::Result<engine::install::InstallPostInstallEdit> {
        Ok(engine::install::InstallPostInstallEdit {
            name: self.name,
            icon_path: self.icon_path,
            link: self.link.map(InstanceLink::into_core).transpose()?,
        })
    }
}
