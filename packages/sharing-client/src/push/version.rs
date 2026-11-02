//! What goes into a new version: the one builder the App's publish and Core's server share use.
//! Each side reads what it has (its content rows, its folder) into a [`VersionInput`]; the
//! builder applies the share's rules and returns the content and files [`super::push`] sends.

use std::collections::HashSet;

use super::PushFile;
use crate::rules::{CONFIG_FILE_TYPE, ShareScope};
use crate::versions::{InvalidRemovedFile, RemovedFile, VersionContent};

/// A new version as one side reads it from its own state.
pub struct VersionInput {
    /// The Modrinth versions members install.
    pub modrinth_ids: Vec<String>,
    /// The Modrinth version of the modpack the version is based on.
    pub modpack_id: Option<String>,
    /// Files of the modpack that members do not get.
    pub removed_files: Vec<RemovedFile>,
    pub game_version: String,
    /// The loader's name as the backend takes it (see [`VersionContent::loader`]).
    pub loader: String,
    pub loader_version: Option<String>,
    /// Content files without a Modrinth version, and the config files the owner picked, staged
    /// where the push reads them.
    pub files: Vec<PushFile>,
}

/// Why a version cannot be built.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error(transparent)]
    RemovedFile(#[from] InvalidRemovedFile),
    #[error("{0} cannot be shared.")]
    NotShareable(String),
}

/// The content and files of the version `input` describes, for a share of `scope`. The modpack
/// is not listed among its own Modrinth versions, ids and files appear once, and only a version
/// on a modpack lists removed files. A config the scope does not carry is refused.
pub fn build_version(
    scope: ShareScope,
    input: VersionInput,
) -> Result<(VersionContent, Vec<PushFile>), BuildError> {
    let modpack_id = input.modpack_id.filter(|id| !id.is_empty());
    let mut modrinth_ids = input
        .modrinth_ids
        .into_iter()
        .filter(|id| modpack_id.as_ref() != Some(id))
        .collect::<Vec<_>>();
    modrinth_ids.sort_unstable();
    modrinth_ids.dedup();
    let mut removed_files = Vec::new();
    if modpack_id.is_some() {
        for removed in input.removed_files {
            removed.validate()?;
            if !removed_files.contains(&removed) {
                removed_files.push(removed);
            }
        }
    } else if !input.removed_files.is_empty() {
        return Err(InvalidRemovedFile.into());
    }
    let mut seen = HashSet::new();
    let mut files = Vec::with_capacity(input.files.len());
    for file in input.files {
        if file.file_type == CONFIG_FILE_TYPE && !scope.carries(&file.file_name)
        {
            return Err(BuildError::NotShareable(file.file_name));
        }
        if seen.insert((file.file_type.clone(), file.file_name.clone())) {
            files.push(file);
        }
    }
    Ok((
        VersionContent {
            modrinth_ids,
            modpack_id,
            removed_files,
            game_version: input.game_version,
            loader: input.loader,
            loader_version: input.loader_version.unwrap_or_default(),
        },
        files,
    ))
}

/// Whether a push sends again the config at `path` the latest version shares: while the scope
/// carries it and the owner did not stop sharing it (`removed`). The caller then looks for its
/// copy, which is left out when it is gone.
pub fn keeps_config(scope: ShareScope, removed: &[String], path: &str) -> bool {
    scope.carries(path) && !removed.iter().any(|removed| removed == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(modpack_id: Option<&str>) -> VersionInput {
        VersionInput {
            modrinth_ids: vec![
                "b".into(),
                "pack".into(),
                "a".into(),
                "b".into(),
            ],
            modpack_id: modpack_id.map(str::to_owned),
            removed_files: vec![RemovedFile::Version {
                version_id: "x".into(),
            }],
            game_version: "1.21.1".into(),
            loader: "fabric".into(),
            loader_version: None,
            files: vec![
                PushFile {
                    file_name: "config/a.toml".into(),
                    file_type: CONFIG_FILE_TYPE.into(),
                    path: "a".into(),
                },
                PushFile {
                    file_name: "config/a.toml".into(),
                    file_type: CONFIG_FILE_TYPE.into(),
                    path: "b".into(),
                },
            ],
        }
    }

    #[test]
    fn a_version_lists_each_thing_once_and_never_its_modpack() {
        let (content, files) =
            build_version(ShareScope::Server, input(Some("pack"))).unwrap();

        assert_eq!(content.modrinth_ids, ["a", "b"]);
        assert_eq!(content.removed_files.len(), 1);
        assert_eq!(content.loader_version, "");
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn only_a_version_on_a_modpack_removes_files() {
        assert!(build_version(ShareScope::Server, input(None)).is_err());
    }

    #[test]
    fn a_config_outside_the_scope_is_refused() {
        let mut input = input(Some("pack"));
        input.files[0].file_name = "options.txt".into();

        assert!(matches!(
            build_version(ShareScope::Server, input),
            Err(BuildError::NotShareable(_))
        ));
        assert!(keeps_config(ShareScope::Server, &[], "config/a.toml"));
        assert!(!keeps_config(
            ShareScope::Server,
            &["config/a.toml".into()],
            "config/a.toml"
        ));
    }
}
