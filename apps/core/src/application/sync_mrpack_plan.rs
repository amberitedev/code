use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use bytes::Bytes;
use serde::Serialize;
use sha2::Digest;

use crate::{
    application::{social_models::SocialError, sync_mrpack_files},
    domain::modpack::{
        EnvType, PackFile, PackFormat, PackOverride, PackOverrideSideState,
    },
    infrastructure::minecraft::mrpack::validate_download_url,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedModState {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone)]
pub enum PlannedModSource {
    Download(String),
    Archive(Bytes),
}

#[derive(Debug, Clone)]
pub struct PlannedServerMod {
    pub filename: String,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub sha512: Option<String>,
    pub sha1: Option<String>,
    pub state: PlannedModState,
    pub source: PlannedModSource,
}

#[derive(Debug)]
pub struct SnapshotServerPlan {
    pub mods: HashMap<String, PlannedServerMod>,
    pub removed: HashSet<String>,
    pub invalid_overrides: Vec<InvalidOverride>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidOverride {
    pub project_id: Option<String>,
    pub path: Option<String>,
    pub reason: String,
}

#[derive(Debug)]
struct Candidate {
    filename: String,
    path: String,
    project_id: Option<String>,
    supported: bool,
}

pub async fn server_plan(
    archive_path: &Path,
    metadata: &PackFormat,
) -> Result<SnapshotServerPlan, SocialError> {
    let archive_mods = sync_mrpack_files::archive_mods(archive_path).await?;
    let archive_names = archive_mods
        .iter()
        .map(|(filename, _)| filename.clone())
        .collect::<Vec<_>>();
    let candidates = candidates(metadata, &archive_names)?;
    let (actions, invalid_overrides) =
        resolve_override_actions(&candidates, &metadata.overrides);
    let mut mods = HashMap::new();
    let mut removed = HashSet::new();

    for file in &metadata.files {
        let Some(candidate) = file_candidate(file)? else {
            continue;
        };
        let action = actions
            .get(&candidate.filename)
            .unwrap_or(&PackOverrideSideState::Enabled);
        if matches!(action, PackOverrideSideState::Removed) {
            removed.insert(candidate.filename);
            continue;
        }
        if !candidate.supported {
            continue;
        }
        let url = file.downloads.first().ok_or_else(|| {
            SocialError::Invalid(format!(
                "mod file has no download: {}",
                file.path
            ))
        })?;
        validate_download_url(url)?;
        let state = planned_state(action);
        mods.insert(
            candidate.filename.clone(),
            PlannedServerMod {
                filename: candidate.filename,
                project_id: file.project_id.clone(),
                version_id: file.version_id.clone(),
                sha512: file.hashes.sha512.clone(),
                sha1: file.hashes.sha1.clone(),
                state,
                source: PlannedModSource::Download(url.clone()),
            },
        );
    }

    for (filename, data) in archive_mods {
        let action = actions
            .get(&filename)
            .unwrap_or(&PackOverrideSideState::Enabled);
        if matches!(action, PackOverrideSideState::Removed) {
            removed.insert(filename);
            continue;
        }
        let sha512 = hex::encode(sha2::Sha512::digest(&data));
        let sha1 = hex::encode(sha1::Sha1::digest(&data));
        let state = planned_state(action);
        mods.insert(
            filename.clone(),
            PlannedServerMod {
                filename,
                project_id: None,
                version_id: None,
                sha512: Some(sha512),
                sha1: Some(sha1),
                state,
                source: PlannedModSource::Archive(Bytes::from(data)),
            },
        );
    }

    Ok(SnapshotServerPlan {
        mods,
        removed,
        invalid_overrides,
    })
}

pub async fn installed_managed_names(
    archive_path: Option<&Path>,
    metadata: &PackFormat,
) -> Result<HashSet<String>, SocialError> {
    let archive_names = if let Some(path) = archive_path {
        sync_mrpack_files::archive_mod_names(path).await?
    } else {
        Vec::new()
    };
    let candidates = candidates(metadata, &archive_names)?;
    let (actions, _) =
        resolve_override_actions(&candidates, &metadata.overrides);
    let mut names = HashSet::new();

    for candidate in candidates {
        let action = actions
            .get(&candidate.filename)
            .unwrap_or(&PackOverrideSideState::Enabled);
        if candidate.supported
            && !matches!(action, PackOverrideSideState::Removed)
        {
            names.insert(candidate.filename);
        }
    }

    Ok(names)
}

fn candidates(
    metadata: &PackFormat,
    archive_names: &[String],
) -> Result<Vec<Candidate>, SocialError> {
    let mut candidates = Vec::new();
    for file in &metadata.files {
        if let Some(candidate) = file_candidate(file)? {
            candidates.push(candidate);
        }
    }
    for filename in archive_names {
        validate_filename(filename)?;
        candidates.push(Candidate {
            filename: filename.clone(),
            path: format!("mods/{filename}"),
            project_id: None,
            supported: true,
        });
    }
    Ok(candidates)
}

fn file_candidate(file: &PackFile) -> Result<Option<Candidate>, SocialError> {
    if !file.path.starts_with("mods/") || !file.path.ends_with(".jar") {
        return Ok(None);
    }
    let filename = file.path.trim_start_matches("mods/").to_string();
    validate_filename(&filename)?;
    Ok(Some(Candidate {
        filename,
        path: normalize_path(&file.path),
        project_id: clean_string(file.project_id.as_deref()),
        supported: server_supported(file),
    }))
}

fn resolve_override_actions(
    candidates: &[Candidate],
    overrides: &[PackOverride],
) -> (HashMap<String, PackOverrideSideState>, Vec<InvalidOverride>) {
    let mut actions = HashMap::new();
    let mut invalid = Vec::new();

    for item in overrides {
        let Some(state) = item.server.clone() else {
            continue;
        };
        let Some(candidate) = match_override(candidates, item) else {
            invalid.push(invalid_override(item, "no matching file"));
            continue;
        };
        if !candidate.supported
            && !matches!(state, PackOverrideSideState::Removed)
        {
            invalid.push(invalid_override(item, "server unsupported"));
            continue;
        }
        actions.insert(candidate.filename.clone(), state);
    }

    (actions, invalid)
}

fn match_override<'a>(
    candidates: &'a [Candidate],
    item: &PackOverride,
) -> Option<&'a Candidate> {
    if let Some(project_id) = clean_string(item.project_id.as_deref()) {
        if let Some(candidate) = candidates.iter().find(|candidate| {
            candidate.project_id.as_deref() == Some(project_id.as_str())
        }) {
            return Some(candidate);
        }
    }
    let path =
        clean_string(item.path.as_deref()).map(|path| normalize_path(&path))?;
    candidates.iter().find(|candidate| candidate.path == path)
}

fn planned_state(state: &PackOverrideSideState) -> PlannedModState {
    match state {
        PackOverrideSideState::Disabled => PlannedModState::Disabled,
        PackOverrideSideState::Enabled | PackOverrideSideState::Removed => {
            PlannedModState::Enabled
        }
    }
}

fn server_supported(file: &PackFile) -> bool {
    !matches!(
        file.env.as_ref().map(|env| &env.server),
        Some(EnvType::Unsupported)
    )
}

fn invalid_override(item: &PackOverride, reason: &str) -> InvalidOverride {
    InvalidOverride {
        project_id: item.project_id.clone(),
        path: item.path.clone(),
        reason: reason.to_string(),
    }
}

fn clean_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn validate_filename(name: &str) -> Result<(), SocialError> {
    if name.is_empty()
        || name.contains("..")
        || name.contains('/')
        || name.contains('\\')
    {
        return Err(SocialError::Invalid(
            "invalid mod filename in mrpack".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Write;

    use crate::domain::modpack::{
        EnvType, PackFile, PackFileEnv, PackFileHashes, PackFormat,
        PackOverride, PackOverrideSideState,
    };

    use super::{server_plan, PlannedModState};

    #[tokio::test]
    async fn override_matching_prefers_project_id_then_falls_back_to_path() {
        let archive = empty_mrpack();
        let metadata = PackFormat {
            format_version: Some(1),
            game: "minecraft".into(),
            name: "Pack".into(),
            version_id: "v1".into(),
            summary: None,
            files: vec![
                file("mods/project.jar", Some("project-a")),
                file("mods/path.jar", None),
            ],
            dependencies: HashMap::new(),
            overrides: vec![
                PackOverride {
                    project_id: Some("project-a".into()),
                    path: Some("mods/path.jar".into()),
                    client: None,
                    server: Some(PackOverrideSideState::Disabled),
                },
                PackOverride {
                    project_id: Some("missing-project".into()),
                    path: Some("mods/path.jar".into()),
                    client: None,
                    server: Some(PackOverrideSideState::Removed),
                },
            ],
        };

        let plan = server_plan(archive.path(), &metadata).await.unwrap();

        assert_eq!(plan.mods["project.jar"].state, PlannedModState::Disabled);
        assert!(!plan.mods.contains_key("path.jar"));
        assert!(plan.removed.contains("path.jar"));
    }

    #[tokio::test]
    async fn records_invalid_override_entries_without_failing_plan() {
        let archive = empty_mrpack();
        let metadata = PackFormat {
            format_version: Some(1),
            game: "minecraft".into(),
            name: "Pack".into(),
            version_id: "v1".into(),
            summary: None,
            files: vec![],
            dependencies: HashMap::new(),
            overrides: vec![PackOverride {
                project_id: Some("missing".into()),
                path: Some("mods/missing.jar".into()),
                client: None,
                server: Some(PackOverrideSideState::Disabled),
            }],
        };

        let plan = server_plan(archive.path(), &metadata).await.unwrap();

        assert_eq!(plan.invalid_overrides.len(), 1);
        assert_eq!(plan.invalid_overrides[0].reason, "no matching file");
    }

    fn file(path: &str, project_id: Option<&str>) -> PackFile {
        PackFile {
            path: path.into(),
            project_id: project_id.map(str::to_string),
            version_id: Some(format!("{path}-version")),
            hashes: PackFileHashes {
                sha1: None,
                sha512: None,
            },
            env: Some(PackFileEnv {
                client: EnvType::Required,
                server: EnvType::Required,
            }),
            downloads: vec![format!(
                "https://cdn.modrinth.com/data/test/versions/{}",
                path.replace('/', "-")
            )],
            file_size: 1,
        }
    }

    fn empty_mrpack() -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        {
            let cursor = std::io::Cursor::new(Vec::new());
            let mut zip = zip::ZipWriter::new(cursor);
            zip.start_file(
                "modrinth.index.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"{}").unwrap();
            let bytes = zip.finish().unwrap().into_inner();
            std::fs::write(file.path(), bytes).unwrap();
        }
        file
    }
}
