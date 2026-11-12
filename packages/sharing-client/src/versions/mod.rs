//! A share version (`contracts::backend::sharing::InstanceVersion`) as the App and Core read it.

mod removed_file;

use contracts::backend::sharing::{ExternalFile, InstanceVersion};
pub use removed_file::{InvalidRemovedFile, RemovedFileRules, removed_path};

/// The manifest's config files: single files the owner picked, by their path.
pub fn configs(
    version: &InstanceVersion,
) -> impl Iterator<Item = &ExternalFile> {
    version
        .manifest
        .external_files
        .iter()
        .filter(|file| file.file_type == crate::rules::CONFIG_FILE_TYPE)
}

/// The modpack the version is based on, when it has one.
pub fn modpack_id(version: &InstanceVersion) -> Option<&str> {
    version
        .manifest
        .content
        .modpack_id
        .as_deref()
        .filter(|id| !id.is_empty())
}

/// Checks the removed files, which only a version based on a modpack may list.
pub fn check_removed_files(
    version: &InstanceVersion,
) -> Result<(), InvalidRemovedFile> {
    let removed = &version.manifest.content.removed_files;
    if modpack_id(version).is_none() && !removed.is_empty() {
        return Err(InvalidRemovedFile);
    }
    removed.iter().try_for_each(RemovedFileRules::validate)
}
