use std::collections::BTreeMap;

use crate::rules::CONFIG_FILE_TYPE;
use contracts::backend::sharing::ExternalFile;

/// The config files an update from `applied` to `latest` touches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigUpdate {
    /// Configs of `latest` that are new or changed since `applied`, by path. Every version lists
    /// all configs it shares, so the rest stay as the member has them.
    pub write: Vec<ExternalFile>,
    /// Configs `applied` shared that `latest` no longer does, as `applied` sent them. A member
    /// deletes each one whose copy is still byte for byte what the share sent (its SHA-1 is
    /// this entry's), and keeps one they edited.
    pub left: Vec<ExternalFile>,
}

/// What an update to `latest` does to the configs of a member who applied `applied`. Without an
/// applied version (a first install, or one the backend no longer keeps) every config is
/// written and none is known to have left.
pub fn config_update(
    latest: &[ExternalFile],
    applied: Option<&[ExternalFile]>,
) -> ConfigUpdate {
    let configs = |files: &[ExternalFile]| {
        files
            .iter()
            .filter(|file| file.file_type == CONFIG_FILE_TYPE)
            .map(|file| (file.file_name.clone(), file.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    let latest = configs(latest);
    let applied = configs(applied.unwrap_or_default());
    ConfigUpdate {
        write: latest
            .iter()
            .filter(|(path, file)| {
                applied.get(*path).map(|applied| applied.sha256)
                    != Some(file.sha256)
            })
            .map(|(_, file)| file.clone())
            .collect(),
        left: applied
            .into_iter()
            .filter(|(path, _)| !latest.contains_key(path))
            .map(|(_, file)| file)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use contracts::Hash;

    use super::*;

    fn file(file_type: &str, file_name: &str, sha256: u8) -> ExternalFile {
        ExternalFile {
            file_name: file_name.to_string(),
            file_type: file_type.to_string(),
            url: None,
            file_size: 1,
            sha1: String::new(),
            sha256: Hash([sha256; 32]),
            object: Hash([0; 32]),
            object_size: 17,
            key: None,
        }
    }

    fn names(files: &[ExternalFile]) -> Vec<&str> {
        files.iter().map(|file| file.file_name.as_str()).collect()
    }

    #[test]
    fn an_update_writes_changed_configs_and_lists_the_ones_that_left() {
        let applied = [
            file(CONFIG_FILE_TYPE, "config/same.toml", 1),
            file(CONFIG_FILE_TYPE, "config/changed.toml", 1),
            file(CONFIG_FILE_TYPE, "config/gone.toml", 1),
            file("mod", "gone.jar", 1),
        ];
        let latest = [
            file("mod", "extra.jar", 2),
            file(CONFIG_FILE_TYPE, "config/same.toml", 1),
            file(CONFIG_FILE_TYPE, "config/changed.toml", 2),
            file(CONFIG_FILE_TYPE, "config/new.toml", 2),
        ];

        let update = config_update(&latest, Some(&applied));

        assert_eq!(
            names(&update.write),
            ["config/changed.toml", "config/new.toml"]
        );
        assert_eq!(names(&update.left), ["config/gone.toml"]);
    }

    #[test]
    fn a_first_install_writes_every_config() {
        let latest = [
            file(CONFIG_FILE_TYPE, "config/b.toml", 1),
            file("mod", "a.jar", 1),
            file(CONFIG_FILE_TYPE, "config/a.toml", 1),
        ];

        let update = config_update(&latest, None);

        assert_eq!(names(&update.write), ["config/a.toml", "config/b.toml"]);
        assert!(update.left.is_empty());
    }
}
