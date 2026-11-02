//! A share version as a member installs it: what the version puts where beyond what its modpack
//! places ([`version_files`]), which config files an update writes or deletes
//! ([`config_update`]), and the plan of both ([`install_plan`]). The App and a Core following a
//! share both plan their install here; applying the plan stays with each side, which writes into
//! its own folder.

mod configs;
mod modpack;
mod plan;

pub use configs::{ConfigUpdate, config_update};
pub use modpack::{
    ModpackFile, VersionFiles, content_path, split_content, version_files,
};
pub use plan::{InstallPlan, Side, Source, Step, install_plan};
