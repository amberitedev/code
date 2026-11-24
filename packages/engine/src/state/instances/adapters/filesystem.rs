use crate::state::{ProjectType, file_hash_cache_key, file_modified_at_ns};
use crate::util::io::{self, IOError};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ScannedContentFile {
    pub relative_path: String,
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub hash_cache_key: String,
    pub is_symlink: bool,
    pub has_linked_parent: bool,
}

pub fn scan_content_files(
    instances_dir: &Path,
    instance_path: &str,
) -> crate::Result<Vec<ScannedContentFile>> {
    crate::state::content_store::validate_instance_path(instance_path)?;
    let instance_full_path = instances_dir.join(instance_path);
    let instance_dir = io::canonicalize(instance_full_path)?;
    let linked_instance =
        instance_dir != io::canonicalize(instances_dir)?.join(instance_path);
    let mut files = Vec::new();

    for project_type in ProjectType::iterator() {
        let folder = project_type.get_folder();
        let folder_path = instance_dir.join(folder);
        let has_linked_parent = linked_instance
            || std::fs::symlink_metadata(&folder_path)
                .is_ok_and(|metadata| metadata.file_type().is_symlink());

        if !folder_path.exists() {
            continue;
        }

        for entry in std::fs::read_dir(&folder_path)
            .map_err(|err| IOError::with_path(err, &folder_path))?
        {
            let path = entry.map_err(IOError::from)?.path();
            if !path.is_file() {
                continue;
            }

            let Some(file_name) =
                path.file_name().and_then(|value| value.to_str())
            else {
                continue;
            };

            if !is_scannable_project_file(project_type, file_name) {
                continue;
            }

            let metadata = path.metadata().map_err(IOError::from)?;
            let size = metadata.len();
            let modified_at_ns =
                file_modified_at_ns(&metadata).map_err(IOError::from)?;
            let relative_path = format!("{folder}/{file_name}");
            let hash_cache_key = file_hash_cache_key(
                size,
                modified_at_ns,
                &format!("{instance_path}/{relative_path}"),
            );

            files.push(ScannedContentFile {
                has_linked_parent,
                relative_path,
                file_name: file_name.to_string(),
                enabled: !file_name.ends_with(".disabled"),
                size,
                hash_cache_key,
                is_symlink: path
                    .symlink_metadata()
                    .map_err(IOError::from)?
                    .file_type()
                    .is_symlink(),
            });
        }
    }

    Ok(files)
}

pub(crate) fn project_type_from_relative_path(
    relative_path: &str,
) -> Option<ProjectType> {
    ProjectType::get_from_parent_folder(PathBuf::from(relative_path))
}

/// The project type of the file at `relative_path` on a server, from its folder: directly inside
/// `mods/`, `plugins/`, `resourcepacks/`, `shaderpacks/` or `<level>/datapacks/`. `None` for
/// anything else, such as a config.
pub(crate) fn server_addon_type(relative_path: &str) -> Option<ProjectType> {
    let (folder, name) = relative_path.rsplit_once('/')?;
    if name.is_empty() {
        return None;
    }
    if folder.matches('/').count() == 1 {
        return folder
            .ends_with("/datapacks")
            .then_some(ProjectType::DataPack);
    }
    match ProjectType::from_name(folder)? {
        ProjectType::DataPack => None,
        project_type => Some(project_type),
    }
}

fn is_scannable_project_file(
    project_type: ProjectType,
    file_name: &str,
) -> bool {
    let Some(extension) = Path::new(file_name.trim_end_matches(".disabled"))
        .extension()
        .and_then(|ext| ext.to_str())
    else {
        return false;
    };

    match project_type {
        ProjectType::Mod | ProjectType::Plugin => {
            extension.eq_ignore_ascii_case("jar")
        }
        ProjectType::DataPack
        | ProjectType::ResourcePack
        | ProjectType::ShaderPack => {
            extension.eq_ignore_ascii_case("zip")
                || extension.eq_ignore_ascii_case("jar")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_addon_s_type_is_its_content_folder_s() {
        let of = server_addon_type;
        assert_eq!(of("mods/a.jar"), Some(ProjectType::Mod));
        assert_eq!(of("plugins/a.jar"), Some(ProjectType::Plugin));
        assert_eq!(of("world/datapacks/a.zip"), Some(ProjectType::DataPack));
        assert_eq!(of("datapacks/a.zip"), None);
        assert_eq!(of("config/a.toml"), None);
        assert_eq!(of("mods/sub/a.jar"), None);
        assert_eq!(of("a/b/datapacks/a.zip"), None);
    }
}
