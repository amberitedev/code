//! A share version as a member installs it: what the version puts where beyond what its modpack
//! places ([`version_files`]), which config files an update writes or deletes
//! ([`config_update`]), the plan of both ([`install_plan`]), and how content on disk changes into
//! a version's without a reinstall ([`content_changes`]). The App and a Core plan here; applying
//! the plan stays with each side, which writes into its own folder.

mod changes;
mod configs;
mod modpack;
mod plan;

pub use changes::{
    ContentChanges, ContentProject, CurrentFile, DesiredFile, content_changes,
};
pub use configs::{ConfigUpdate, config_update};
pub use modpack::{
    ModpackFile, VersionFiles, content_path, split_content, version_files,
};
pub use plan::{InstallPlan, Source, Step, install_plan};
