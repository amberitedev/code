use contracts::Side;

use crate::rules::CONFIG_FILE_TYPE;
use contracts::backend::sharing::{ExternalFile, InstanceVersion};

use super::{ConfigUpdate, ModpackFile, content_path, version_files};

/// Where a manifest entry goes on `side`, or `None` when that side does not take it. A server
/// takes mods and the config files a version carries; resource packs, shaders and the rest are
/// for players. A modpack's own files all go to the server's pack install, which leaves out the
/// client-only ones itself.
fn side_path(side: Side, file_type: &str, file_name: &str) -> Option<String> {
    match side {
        Side::Client => content_path(file_type, file_name),
        Side::Server => match file_type {
            CONFIG_FILE_TYPE => Some(file_name.to_owned()),
            "mod" => Some(format!("mods/{file_name}")),
            _ => None,
        },
    }
}

/// Where a planned file's bytes come from.
#[derive(Debug, Clone)]
pub enum Source<S> {
    /// The version's modpack, which places it.
    Modpack(ModpackFile<S>),
    /// The pool, as the manifest names it.
    Pool(ExternalFile),
}

/// One thing an install does to a path.
#[derive(Debug, Clone)]
pub enum Step<S> {
    /// Put the file at `path`. Each side's writer decides how, and whether a file already there
    /// is replaced (the App skips one with the same bytes; a Core keeps an owner's edit pending).
    Place { path: String, source: Source<S> },
    /// A config the version still shares and this update leaves as the member has it.
    Keep { path: String, file: ExternalFile },
    /// Delete the file at `path` while it is still what the share sent (its SHA-1 is `file`'s):
    /// a config that left the share. A copy the member edited stays theirs. Checked when the
    /// writer gets to it, since the member may change it meanwhile.
    Delete { path: String, file: ExternalFile },
}

/// What installing a share version does: the Modrinth versions it installs beside its modpack,
/// and the files it places, keeps and deletes at their paths.
#[derive(Debug, Clone)]
pub struct InstallPlan<S> {
    /// The Modrinth versions the version adds over its modpack, sorted. Each side resolves them
    /// to files with its own Modrinth client.
    pub modrinth_ids: Vec<String>,
    pub steps: Vec<Step<S>>,
}

impl<S> InstallPlan<S> {
    /// The pool files the install writes, at their paths.
    pub fn pool_files(&self) -> impl Iterator<Item = (&str, &ExternalFile)> {
        self.steps.iter().filter_map(|step| match step {
            Step::Place {
                path,
                source: Source::Pool(file),
            } => Some((path.as_str(), file)),
            _ => None,
        })
    }

    /// The modpack's files the install places.
    pub fn modpack_files(&self) -> impl Iterator<Item = &ModpackFile<S>> {
        self.steps.iter().filter_map(|step| match step {
            Step::Place {
                source: Source::Modpack(file),
                ..
            } => Some(file),
            _ => None,
        })
    }

    /// The configs that left the share, which the install deletes while they are unedited.
    pub fn deleted(&self) -> impl Iterator<Item = (&str, &ExternalFile)> {
        self.steps.iter().filter_map(|step| match step {
            Step::Delete { path, file } => Some((path.as_str(), file)),
            _ => None,
        })
    }
}

/// How `side` installs `version` over `modpack` (every file of the version's modpack as the
/// caller reads it, empty without one), with `configs` from [`super::config_update`] against
/// the version the member applied. The modpack keeps the files the owner did not remove and the
/// version does not carry itself ([`version_files`]); the version's pool files go to this
/// side's paths; configs are written when new or changed, kept otherwise, and deleted once they
/// leave the share. Both the App and a Core plan their installs here and apply the steps with
/// their own writer.
pub fn install_plan<S>(
    version: &InstanceVersion,
    modpack: Vec<ModpackFile<S>>,
    configs: ConfigUpdate,
    side: Side,
) -> InstallPlan<S> {
    let split = version_files(version, modpack);
    let mut steps = split
        .modpack_files
        .into_iter()
        .map(|file| Step::Place {
            path: file.path.clone(),
            source: Source::Modpack(file),
        })
        .collect::<Vec<_>>();
    let written = |file: &ExternalFile| {
        configs
            .write
            .iter()
            .any(|written| written.file_name == file.file_name)
    };
    for file in &version.manifest.external_files {
        let Some(path) = side_path(side, &file.file_type, &file.file_name)
        else {
            continue;
        };
        let config = file.file_type == CONFIG_FILE_TYPE;
        steps.push(if config && !written(file) {
            Step::Keep {
                path,
                file: file.clone(),
            }
        } else {
            Step::Place {
                path,
                source: Source::Pool(file.clone()),
            }
        });
    }
    for file in configs.left {
        if let Some(path) = side_path(side, &file.file_type, &file.file_name) {
            steps.push(Step::Delete { path, file });
        }
    }
    InstallPlan {
        modrinth_ids: split.modrinth_ids,
        steps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::config_update;
    use contracts::Hash;
    use contracts::backend::sharing::{VersionContent, VersionManifest};

    use crate::versions::removed_path;

    fn manifest(file_type: &str, file_name: &str, sha256: u8) -> ExternalFile {
        ExternalFile {
            file_name: file_name.into(),
            file_type: file_type.into(),
            url: None,
            file_size: 1,
            sha1: format!("{sha256}"),
            sha256: Hash([sha256; 32]),
            object: Hash([0; 32]),
            object_size: 17,
            key: None,
        }
    }

    fn version(files: Vec<ExternalFile>) -> InstanceVersion {
        InstanceVersion {
            manifest: VersionManifest {
                content: VersionContent {
                    modrinth_ids: vec![
                        "pack".into(),
                        "a".into(),
                        "extra".into(),
                    ],
                    modpack_id: Some("pack".into()),
                    removed_files: vec![
                        removed_path("mods", "gone.jar").unwrap(),
                    ],
                    game_version: "1.21.1".into(),
                    loader: "fabric".into(),
                    loader_version: "0.16.0".into(),
                },
                version_hash: Hash([0; 32]),
                external_files: files,
            },
            version: 2,
            ready: true,
            download: None,
            upload: None,
            reports: None,
        }
    }

    fn pack_file(path: &str, version_id: Option<&str>) -> ModpackFile {
        ModpackFile {
            path: path.into(),
            sha1: None,
            version_id: version_id.map(str::to_owned),
            alias: None,
            source: (),
        }
    }

    /// Each step as `(kind, path)`.
    fn steps(plan: &InstallPlan<()>) -> Vec<(&'static str, &str)> {
        plan.steps
            .iter()
            .map(|step| match step {
                Step::Place {
                    path,
                    source: Source::Modpack(_),
                } => ("modpack", path.as_str()),
                Step::Place {
                    path,
                    source: Source::Pool(_),
                } => ("pool", path.as_str()),
                Step::Keep { path, .. } => ("keep", path.as_str()),
                Step::Delete { path, .. } => ("delete", path.as_str()),
            })
            .collect()
    }

    fn modpack() -> Vec<ModpackFile> {
        vec![
            pack_file("mods/a.jar", Some("a")),
            pack_file("mods/gone.jar", None),
            pack_file("mods/carried.jar", None),
            pack_file("config/pack.toml", None),
        ]
    }

    #[test]
    fn an_update_places_what_changed_keeps_the_rest_and_deletes_what_left() {
        let applied = [
            manifest(CONFIG_FILE_TYPE, "config/same.toml", 1),
            manifest(CONFIG_FILE_TYPE, "config/changed.toml", 1),
            manifest(CONFIG_FILE_TYPE, "config/left.toml", 1),
        ];
        let version = version(vec![
            manifest("mod", "carried.jar", 2),
            manifest("resourcepack", "look.zip", 2),
            manifest(CONFIG_FILE_TYPE, "config/same.toml", 1),
            manifest(CONFIG_FILE_TYPE, "config/changed.toml", 2),
        ]);
        let configs =
            config_update(&version.manifest.external_files, Some(&applied));

        let plan = install_plan(&version, modpack(), configs, Side::Client);

        assert_eq!(plan.modrinth_ids, ["extra"]);
        assert_eq!(
            steps(&plan),
            [
                ("modpack", "mods/a.jar"),
                ("modpack", "config/pack.toml"),
                ("pool", "mods/carried.jar"),
                ("pool", "resourcepacks/look.zip"),
                ("keep", "config/same.toml"),
                ("pool", "config/changed.toml"),
                ("delete", "config/left.toml"),
            ]
        );
        let deleted = plan.deleted().map(|(_, file)| file.sha1.as_str());
        assert_eq!(deleted.collect::<Vec<_>>(), ["1"]);
    }

    #[test]
    fn a_server_takes_mods_and_configs_and_every_modpack_file() {
        let version = version(vec![
            manifest("mod", "carried.jar", 2),
            manifest("resourcepack", "look.zip", 2),
            manifest("shader", "glow.zip", 2),
            manifest(CONFIG_FILE_TYPE, "config/a.toml", 1),
        ]);
        let configs = config_update(&version.manifest.external_files, None);

        let plan = install_plan(&version, modpack(), configs, Side::Server);

        assert_eq!(
            steps(&plan),
            [
                ("modpack", "mods/a.jar"),
                ("modpack", "config/pack.toml"),
                ("pool", "mods/carried.jar"),
                ("pool", "config/a.toml"),
            ]
        );
        assert_eq!(plan.pool_files().count(), 2);
        assert_eq!(plan.modpack_files().count(), 2);
    }
}
