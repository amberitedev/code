//! The share diff: what one share content changes over another, named for people. The App's
//! update preview (member's instance -> latest version), its publish preview and Core's push
//! preview (latest version -> what a push sends now), and the version history (one version ->
//! the next) all diff with [`share_diff`], built on modrinth-content-management's
//! `diff_content_sets` and `diff_configuration`. Each side reads its own content into a
//! [`ShareContent`] and names things through its own Modrinth client ([`Names`]).

mod model;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::future::Future;

use modrinth_content_management::{
    Change, CommonExternalFilePolicy, ConfigurationDiff,
    ContentSetConfiguration, ContentSetDiffEntry, ContentSetDiffKind,
    ContentSetDiffOptions, ContentSetSnapshot, ExternalFileKey,
    LoaderReference, diff_configuration, diff_content_sets,
};

use bedrock::state::ProjectType;

use crate::install::{ModpackFile, split_content};
use crate::rules::CONFIG_FILE_TYPE;
use crate::versions::{ManifestFile, VersionContent};

pub use model::{SharedInstanceUpdateDiff, SharedInstanceUpdateDiffType};

/// One side of a share diff.
#[derive(Debug, Clone)]
pub struct ShareContent {
    /// Modrinth versions, not counting the modpack.
    pub version_ids: Vec<String>,
    /// Files without a Modrinth version, not counting config files, with their SHA-1 where it
    /// is known.
    pub external_files: BTreeMap<ExternalFileKey, Option<String>>,
    /// Config files, by path, with their SHA-1. One that is absent, or that could not be read,
    /// is left out, so it differs from any copy on the other side.
    pub configs: BTreeMap<String, String>,
    pub configuration: ContentSetConfiguration,
}

/// A file a share content carries outside Modrinth, as a diff compares it.
#[derive(Debug, Clone)]
pub struct CarriedFile<'a> {
    pub file_type: &'a str,
    pub file_name: &'a str,
    /// Its SHA-1, when it is known.
    pub sha1: Option<&'a str>,
}

impl<'a> From<&'a ManifestFile> for CarriedFile<'a> {
    fn from(file: &'a ManifestFile) -> Self {
        Self {
            file_type: &file.file_type,
            file_name: &file.file_name,
            sha1: Some(&file.sha1),
        }
    }
}

impl ShareContent {
    /// What `content` gives members, with `files` and the files of its modpack (`modpack`, see
    /// `install::split_content`), on `configuration`. A file of an unknown type is refused.
    pub fn of<'a, S>(
        content: &VersionContent,
        files: impl IntoIterator<Item = CarriedFile<'a>>,
        modpack: Vec<ModpackFile<S>>,
        configuration: ContentSetConfiguration,
    ) -> Result<Self, modrinth_content_management::Error> {
        let files = files.into_iter().collect::<Vec<_>>();
        let split = split_content(
            content,
            files.iter().map(|file| (file.file_type, file.file_name)),
            modpack,
        );
        let mut external_files = BTreeMap::new();
        let mut configs = BTreeMap::new();
        for file in &files {
            if file.file_type == CONFIG_FILE_TYPE {
                if let Some(sha1) = file.sha1 {
                    configs.insert(file.file_name.to_owned(), sha1.to_owned());
                }
                continue;
            }
            external_files.insert(
                ExternalFileKey {
                    content_type: file.file_type.parse()?,
                    path: file.file_name.to_owned(),
                },
                file.sha1.map(str::to_owned),
            );
        }
        for file in split.unknown_modpack_files() {
            let key = file.path.split_once('/').and_then(|(parent, name)| {
                Some(ExternalFileKey {
                    content_type: ProjectType::from_name(parent)?.into(),
                    path: name.to_owned(),
                })
            });
            if let Some(key) = key {
                external_files.insert(key, file.sha1.clone());
            }
        }
        Ok(Self {
            version_ids: split.all_modrinth_ids(),
            external_files,
            configs,
            configuration,
        })
    }

    /// Nothing at all, on `configuration`: the base of a version without a version before it.
    pub fn empty(configuration: ContentSetConfiguration) -> Self {
        Self {
            version_ids: Vec::new(),
            external_files: BTreeMap::new(),
            configs: BTreeMap::new(),
            configuration,
        }
    }
}

/// How [`share_diff`] reads its two sides.
#[derive(Debug, Clone, Default)]
pub struct DiffOptions {
    /// A file on both sides whose SHA-1 is not known on both is updated
    /// ([`CommonExternalFilePolicy::AssumeUpdated`]) or not. A file whose SHA-1 is known on
    /// both is updated exactly when the hashes differ.
    pub common_external_files: CommonExternalFilePolicy,
    /// What `before` had turned off, so its removal is marked `disabled`.
    pub disabled: Disabled,
}

#[derive(Debug, Clone, Default)]
pub struct Disabled {
    pub project_ids: HashSet<String>,
    pub version_ids: HashSet<String>,
    pub external_files: BTreeSet<ExternalFileKey>,
}

/// A Modrinth version, as a diff names it.
#[derive(Debug, Clone)]
pub struct VersionName {
    pub id: String,
    pub project_id: String,
    /// The version's title, which names a modpack whose project cannot be read.
    pub name: String,
    pub version_number: String,
}

/// Modrinth's names, as the caller reaches them, so each side uses its own client and cache.
pub trait Names {
    type Error;

    /// The versions among `ids` that Modrinth has.
    fn versions(
        &self,
        ids: &[String],
    ) -> impl Future<Output = Result<Vec<VersionName>, Self::Error>> + Send;

    /// The titles of the projects among `ids` that Modrinth has, by id.
    fn projects(
        &self,
        ids: &[String],
    ) -> impl Future<Output = Result<HashMap<String, String>, Self::Error>> + Send;
}

#[derive(Debug, thiserror::Error)]
pub enum DiffError<E> {
    #[error("shared content version {0} was not found")]
    MissingVersion(String),
    #[error(transparent)]
    Names(E),
}

/// Whether `after` changes anything over `before`, without naming the changes: what
/// [`share_diff`] would find non-empty, short of a version Modrinth no longer has.
pub fn has_changes(
    before: &ShareContent,
    after: &ShareContent,
    common_external_files: CommonExternalFilePolicy,
) -> bool {
    fn ids(content: &ShareContent) -> BTreeSet<&String> {
        content.version_ids.iter().collect()
    }
    ids(before) != ids(after)
        || !external_file_changes(before, after, common_external_files)
            .is_empty()
        || !changed_configs(before, after).is_empty()
        || !diff_configuration(&before.configuration, &after.configuration)
            .is_empty()
}

/// What `after` changes over `before`: the modpack, Minecraft and loader first, then projects
/// and files by name, then one entry for the config files. A version `before` lists that Modrinth
/// no longer has is shown as removed, so an owner can push a version without it.
pub async fn share_diff<N: Names>(
    before: &ShareContent,
    after: &ShareContent,
    options: &DiffOptions,
    names: &N,
) -> Result<Vec<SharedInstanceUpdateDiff>, DiffError<N::Error>> {
    let modpacks = [&before.configuration, &after.configuration]
        .into_iter()
        .filter_map(|configuration| configuration.modpack_version_id.clone())
        .filter(|id| !id.is_empty());
    let ids = before
        .version_ids
        .iter()
        .chain(&after.version_ids)
        .cloned()
        .chain(modpacks)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let versions = if ids.is_empty() {
        HashMap::new()
    } else {
        names
            .versions(&ids)
            .await
            .map_err(DiffError::Names)?
            .into_iter()
            .map(|version| (version.id.clone(), version))
            .collect::<HashMap<_, _>>()
    };
    if let Some(missing) = after
        .version_ids
        .iter()
        .find(|id| !versions.contains_key(*id))
    {
        return Err(DiffError::MissingVersion(missing.clone()));
    }

    let mut content =
        external_file_changes(before, after, options.common_external_files);
    content.extend(project_changes(before, after, &versions));
    let configuration =
        diff_configuration(&before.configuration, &after.configuration);
    let configs = changed_configs(before, after);
    let missing_before = before
        .version_ids
        .iter()
        .filter(|id| !versions.contains_key(*id))
        .collect::<BTreeSet<_>>();
    if content.is_empty()
        && configuration.is_empty()
        && configs.is_empty()
        && missing_before.is_empty()
    {
        return Ok(Vec::new());
    }

    let project_ids = content
        .iter()
        .filter_map(|entry| match entry {
            ContentSetDiffEntry::Project { project_id, .. } => {
                Some(project_id.clone())
            }
            ContentSetDiffEntry::ExternalFile { .. } => None,
        })
        .chain(configuration.iter().filter_map(|change| match change {
            ConfigurationDiff::Modpack(change) => {
                let id = change.after().or(change.before())?;
                versions.get(id).map(|version| version.project_id.clone())
            }
            _ => None,
        }))
        .chain(configuration.iter().filter_map(|change| {
            match change {
                ConfigurationDiff::Modpack(Change::Updated {
                    before, ..
                }) => versions
                    .get(before)
                    .map(|version| version.project_id.clone()),
                _ => None,
            }
        }))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let projects = if project_ids.is_empty() {
        HashMap::new()
    } else {
        names
            .projects(&project_ids)
            .await
            .map_err(DiffError::Names)?
    };

    let mut diffs = configuration
        .into_iter()
        .map(|change| configuration_entry(change, &versions, &projects))
        .collect::<Vec<_>>();
    let version_number = |id: &String| {
        versions.get(id).map_or_else(
            || id.clone(),
            |version| version.version_number.clone(),
        )
    };
    let mut content_diffs = missing_before
        .into_iter()
        .map(|id| SharedInstanceUpdateDiff {
            project_name: Some(id.clone()),
            current_version_name: Some(id.clone()),
            ..SharedInstanceUpdateDiff::of(
                SharedInstanceUpdateDiffType::Removed,
            )
        })
        .collect::<Vec<_>>();
    for entry in content {
        content_diffs.push(match entry {
            ContentSetDiffEntry::Project { project_id, change } => {
                let disabled = change.kind() == ContentSetDiffKind::Removed
                    && (options.disabled.project_ids.contains(&project_id)
                        || change.before().is_some_and(|id| {
                            options.disabled.version_ids.contains(id)
                        }));
                SharedInstanceUpdateDiff {
                    project_name: Some(
                        projects
                            .get(&project_id)
                            .cloned()
                            .unwrap_or_else(|| project_id.clone()),
                    ),
                    current_version_name: change.before().map(version_number),
                    new_version_name: change.after().map(version_number),
                    project_id: Some(project_id),
                    disabled,
                    ..SharedInstanceUpdateDiff::of(kind_type(change.kind()))
                }
            }
            ContentSetDiffEntry::ExternalFile { file, kind } => {
                SharedInstanceUpdateDiff {
                    disabled: kind == ContentSetDiffKind::Removed
                        && options.disabled.external_files.contains(&file),
                    file_name: Some(file.path),
                    ..SharedInstanceUpdateDiff::of(kind_type(kind))
                }
            }
        });
    }
    content_diffs.sort_by(|a, b| {
        a.project_name
            .as_deref()
            .or(a.file_name.as_deref())
            .cmp(&b.project_name.as_deref().or(b.file_name.as_deref()))
    });
    diffs.extend(content_diffs);
    if !configs.is_empty() {
        diffs.push(SharedInstanceUpdateDiff {
            config_file_count: Some(configs.len()),
            config_file_names: configs,
            ..SharedInstanceUpdateDiff::of(
                SharedInstanceUpdateDiffType::ConfigFilesUpdated,
            )
        });
    }
    Ok(diffs)
}

/// Files added, removed, or updated by the rule of [`DiffOptions::common_external_files`].
fn external_file_changes(
    before: &ShareContent,
    after: &ShareContent,
    common_external_files: CommonExternalFilePolicy,
) -> Vec<ContentSetDiffEntry> {
    let snapshot = |content: &ShareContent| ContentSetSnapshot {
        projects: BTreeMap::new(),
        external_files: content.external_files.keys().cloned().collect(),
    };
    let mut changes = diff_content_sets(
        &snapshot(before),
        &snapshot(after),
        &ContentSetDiffOptions {
            common_external_files: CommonExternalFilePolicy::AssumeUnchanged,
        },
    )
    .content;
    for (file, sha1) in &after.external_files {
        let Some(previous) = before.external_files.get(file) else {
            continue;
        };
        let updated = match (previous, sha1) {
            (Some(previous), Some(sha1)) => {
                !previous.eq_ignore_ascii_case(sha1)
            }
            _ => {
                common_external_files == CommonExternalFilePolicy::AssumeUpdated
            }
        };
        if updated {
            changes.push(ContentSetDiffEntry::ExternalFile {
                file: file.clone(),
                kind: ContentSetDiffKind::Updated,
            });
        }
    }
    changes
}

/// Each project added, removed, or on another version. Only an unambiguous replacement (one
/// version of a project out, one in) is an update; every other version change is kept as is.
fn project_changes(
    before: &ShareContent,
    after: &ShareContent,
    versions: &HashMap<String, VersionName>,
) -> Vec<ContentSetDiffEntry> {
    let known = |content: &ShareContent| {
        content
            .version_ids
            .iter()
            .filter_map(|id| versions.get(id))
            .map(|version| (version.id.clone(), version.project_id.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    let (before, after) = (known(before), known(after));
    let project_ids = before
        .values()
        .chain(after.values())
        .collect::<BTreeSet<_>>();
    let mut changes = Vec::new();
    for project_id in project_ids {
        let only = |side: &BTreeMap<String, String>,
                    other: &BTreeMap<String, String>| {
            side.iter()
                .filter(|(id, project)| {
                    *project == project_id && !other.contains_key(*id)
                })
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>()
        };
        let (removed, added) = (only(&before, &after), only(&after, &before));
        if let ([before], [after]) = (removed.as_slice(), added.as_slice()) {
            changes.push(ContentSetDiffEntry::Project {
                project_id: project_id.clone(),
                change: Change::Updated {
                    before: before.clone(),
                    after: after.clone(),
                },
            });
            continue;
        }
        changes.extend(removed.into_iter().map(|before| {
            ContentSetDiffEntry::Project {
                project_id: project_id.clone(),
                change: Change::Removed { before },
            }
        }));
        changes.extend(added.into_iter().map(|after| {
            ContentSetDiffEntry::Project {
                project_id: project_id.clone(),
                change: Change::Added { after },
            }
        }));
    }
    changes
}

/// The paths of the config files that differ between the sides: new, changed, or gone.
fn changed_configs(before: &ShareContent, after: &ShareContent) -> Vec<String> {
    before
        .configs
        .keys()
        .chain(after.configs.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| {
            match (before.configs.get(*path), after.configs.get(*path)) {
                (Some(before), Some(after)) => {
                    !before.eq_ignore_ascii_case(after)
                }
                _ => true,
            }
        })
        .cloned()
        .collect()
}

fn kind_type(kind: ContentSetDiffKind) -> SharedInstanceUpdateDiffType {
    match kind {
        ContentSetDiffKind::Added => SharedInstanceUpdateDiffType::Added,
        ContentSetDiffKind::Removed => SharedInstanceUpdateDiffType::Removed,
        ContentSetDiffKind::Updated => SharedInstanceUpdateDiffType::Updated,
    }
}

fn configuration_entry(
    change: ConfigurationDiff,
    versions: &HashMap<String, VersionName>,
    projects: &HashMap<String, String>,
) -> SharedInstanceUpdateDiff {
    use SharedInstanceUpdateDiffType as Type;

    let changed = |type_, current, new| SharedInstanceUpdateDiff {
        current_version_name: current,
        new_version_name: new,
        ..SharedInstanceUpdateDiff::of(type_)
    };
    // A modpack's project title, else its version's title.
    let project_name = |id: &String| {
        let version = versions.get(id)?;
        Some(
            projects
                .get(&version.project_id)
                .cloned()
                .unwrap_or_else(|| version.name.clone()),
        )
    };
    let label = |id: &String| match (project_name(id), versions.get(id)) {
        (Some(project), Some(version)) => {
            format!("{project} {}", version.version_number)
        }
        _ => id.clone(),
    };
    let version_number = |id: &String| {
        versions.get(id).map_or_else(
            || id.clone(),
            |version| version.version_number.clone(),
        )
    };
    match change {
        ConfigurationDiff::Modpack(Change::Added { after }) => {
            changed(Type::ModpackLinked, None, Some(label(&after)))
        }
        ConfigurationDiff::Modpack(Change::Removed { before }) => {
            changed(Type::ModpackUnlinked, Some(label(&before)), None)
        }
        ConfigurationDiff::Modpack(Change::Updated { before, after }) => {
            SharedInstanceUpdateDiff {
                project_name: project_name(&after)
                    .or_else(|| project_name(&before)),
                ..changed(
                    Type::ModpackUpdated,
                    Some(version_number(&before)),
                    Some(version_number(&after)),
                )
            }
        }
        ConfigurationDiff::GameVersion(change) => changed(
            Type::GameVersionUpdated,
            change.before().cloned(),
            change.after().cloned(),
        ),
        ConfigurationDiff::Loader(change) => changed(
            Type::LoaderUpdated,
            change.before().map(loader_label),
            change.after().map(loader_label),
        ),
    }
}

fn loader_label(loader: &LoaderReference) -> String {
    let name = match loader.name.as_str() {
        "vanilla" => "Vanilla",
        "forge" => "Forge",
        "fabric" => "Fabric",
        "quilt" => "Quilt",
        "neoforge" => "NeoForge",
        name => name,
    };
    match loader.version.as_deref() {
        Some(version) => format!("{name} {version}"),
        None => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use modrinth_content_management::ContentType;

    struct Fixed;

    impl Names for Fixed {
        type Error = std::convert::Infallible;

        async fn versions(
            &self,
            ids: &[String],
        ) -> Result<Vec<VersionName>, Self::Error> {
            Ok(ids
                .iter()
                .filter(|id| !id.starts_with("gone"))
                .map(|id| VersionName {
                    id: id.clone(),
                    project_id: format!("p-{}", &id[..1]),
                    name: id.clone(),
                    version_number: format!("{id}.0"),
                })
                .collect())
        }

        async fn projects(
            &self,
            ids: &[String],
        ) -> Result<HashMap<String, String>, Self::Error> {
            Ok(ids
                .iter()
                .map(|id| (id.clone(), id.to_uppercase()))
                .collect())
        }
    }

    fn content(
        ids: &[&str],
        files: &[(&str, Option<&str>)],
        configs: &[(&str, &str)],
    ) -> ShareContent {
        ShareContent {
            version_ids: ids.iter().map(|id| id.to_string()).collect(),
            external_files: files
                .iter()
                .map(|(path, sha1)| {
                    (
                        ExternalFileKey {
                            content_type: ContentType::Mod,
                            path: path.to_string(),
                        },
                        sha1.map(str::to_owned),
                    )
                })
                .collect(),
            configs: configs
                .iter()
                .map(|(path, sha1)| (path.to_string(), sha1.to_string()))
                .collect(),
            configuration: ContentSetConfiguration {
                modpack_version_id: None,
                game_version: "1.21.1".into(),
                loader: LoaderReference {
                    name: "fabric".into(),
                    version: Some("0.16.0".into()),
                },
            },
        }
    }

    fn summary(diffs: &[SharedInstanceUpdateDiff]) -> Vec<String> {
        diffs
            .iter()
            .map(|diff| {
                format!(
                    "{:?} {} {}",
                    diff.type_,
                    diff.project_name
                        .as_deref()
                        .or(diff.file_name.as_deref())
                        .unwrap_or_default(),
                    diff.config_file_names.join(",")
                )
                .trim_end()
                .to_owned()
            })
            .collect()
    }

    #[tokio::test]
    async fn files_and_configs_count_only_when_they_changed() {
        let before = content(
            &["a1", "b1", "gone1"],
            &[
                ("same.jar", Some("1")),
                ("changed.jar", Some("1")),
                ("unknown.jar", None),
            ],
            &[
                ("config/same.toml", "1"),
                ("config/edited.toml", "1"),
                ("config/left.toml", "1"),
            ],
        );
        let after = content(
            &["a2", "b1", "c1"],
            &[
                ("same.jar", Some("1")),
                ("changed.jar", Some("2")),
                ("unknown.jar", None),
            ],
            &[
                ("config/same.toml", "1"),
                ("config/edited.toml", "2"),
                ("config/new.toml", "1"),
            ],
        );

        let diffs =
            share_diff(&before, &after, &DiffOptions::default(), &Fixed)
                .await
                .unwrap();

        assert_eq!(
            summary(&diffs),
            [
                "Updated P-A",
                "Added P-C",
                "Updated changed.jar",
                "Removed gone1",
                "ConfigFilesUpdated  config/edited.toml,config/left.toml,config/new.toml",
            ]
        );
        assert!(has_changes(
            &before,
            &after,
            CommonExternalFilePolicy::AssumeUnchanged
        ));
        assert!(!has_changes(
            &after,
            &after,
            CommonExternalFilePolicy::AssumeUnchanged
        ));
    }

    #[tokio::test]
    async fn an_unknown_hash_follows_the_policy() {
        let before = content(&[], &[("a.jar", None)], &[]);
        let options = DiffOptions {
            common_external_files: CommonExternalFilePolicy::AssumeUpdated,
            ..Default::default()
        };

        let diffs = share_diff(&before, &before, &options, &Fixed)
            .await
            .unwrap();

        assert_eq!(summary(&diffs), ["Updated a.jar"]);
    }
}
