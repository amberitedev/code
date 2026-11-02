//! A share version as a member installs it: what the version puts where beyond what its modpack
//! places ([`version_files`]), and which config files an update writes or deletes
//! ([`config_update`]). The App and a Core following a share both plan their install from it;
//! applying stays with each side, which writes into its own folder.

mod configs;
mod modpack;

pub use configs::{ConfigUpdate, config_update};
pub use modpack::{
    ModpackFile, VersionFiles, content_path, split_content, version_files,
};
