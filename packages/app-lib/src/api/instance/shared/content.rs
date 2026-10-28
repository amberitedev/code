use super::client::{
    ExternalFileResponse, InstanceVersionResponse, get_remote_version,
};
use super::*;
use std::collections::BTreeMap;

/// The config files a member writes to reach `latest`. Every version lists all config files
/// shared so far, so only those that changed since `applied_version` are written and the rest
/// stay as the member has them, as when upstream published configs as deltas. Every config is
/// written on a first install, or when the backend no longer keeps the applied version.
pub(super) async fn remote_shared_config_files(
    shared_instance_id: &str,
    applied_version: Option<i32>,
    latest: &InstanceVersionResponse,
    state: &State,
) -> crate::Result<Vec<ExternalFileResponse>> {
    let applied = match applied_version {
        Some(version) if version >= latest.version => return Ok(Vec::new()),
        Some(version) => {
            get_remote_version(shared_instance_id, version, state).await?
        }
        None => None,
    };
    changed_config_files(
        &latest.external_files,
        applied
            .as_ref()
            .map(|version| version.external_files.as_slice()),
    )
}

/// The config files of `latest` that are new or differ from `applied`. A legacy config bundle
/// comes first so individual files are written over it.
fn changed_config_files(
    latest: &[ExternalFileResponse],
    applied: Option<&[ExternalFileResponse]>,
) -> crate::Result<Vec<ExternalFileResponse>> {
    ensure_config_limits(latest)?;
    let applied = applied
        .unwrap_or_default()
        .iter()
        .map(|file| ((&file.file_type, &file.file_name), &file.sha256))
        .collect::<HashMap<_, _>>();
    let mut bundle = None;
    let mut files = BTreeMap::new();
    for file in latest.iter().filter(|file| {
        applied.get(&(&file.file_type, &file.file_name)) != Some(&&file.sha256)
    }) {
        match file.file_type.as_str() {
            CONFIG_BUNDLE_FILE_TYPE => bundle = Some(file.clone()),
            CONFIG_FILE_TYPE => {
                files.insert(file.file_name.clone(), file.clone());
            }
            _ => {}
        }
    }

    Ok(bundle.into_iter().chain(files.into_values()).collect())
}

fn ensure_config_limits(
    external_files: &[ExternalFileResponse],
) -> crate::Result<()> {
    let mut count = 0;
    let mut size = 0_u64;
    for file in external_files
        .iter()
        .filter(|file| file.file_type == CONFIG_FILE_TYPE)
    {
        count += 1;
        size = size.saturating_add(
            file.file_size
                .and_then(|size| u64::try_from(size).ok())
                .unwrap_or(u64::MAX),
        );
    }
    if count > MAX_SHARED_FILES || size > MAX_SHARED_FILES_TOTAL_SIZE {
        bail!(crate::ErrorKind::InputError(
            "shared instance files exceed the size or file count limit"
                .to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(
        file_type: &str,
        file_name: &str,
        sha256: &str,
    ) -> ExternalFileResponse {
        ExternalFileResponse {
            file_name: file_name.to_string(),
            file_type: file_type.to_string(),
            file_size: Some(1),
            sha256: Some(sha256.to_string()),
            ..Default::default()
        }
    }

    fn names(files: &[ExternalFileResponse]) -> Vec<(&str, &str)> {
        files
            .iter()
            .map(|file| (file.file_type.as_str(), file.file_name.as_str()))
            .collect()
    }

    #[test]
    fn only_configs_changed_since_the_applied_version_are_written() {
        let applied = [
            file(CONFIG_FILE_TYPE, "config/same.toml", "1"),
            file(CONFIG_FILE_TYPE, "config/changed.toml", "1"),
        ];
        let latest = [
            file("mod", "extra.jar", "2"),
            file(CONFIG_FILE_TYPE, "config/same.toml", "1"),
            file(CONFIG_FILE_TYPE, "config/changed.toml", "2"),
            file(CONFIG_FILE_TYPE, "config/new.toml", "2"),
        ];

        let files = changed_config_files(&latest, Some(&applied)).unwrap();

        assert_eq!(
            names(&files),
            [
                (CONFIG_FILE_TYPE, "config/changed.toml"),
                (CONFIG_FILE_TYPE, "config/new.toml"),
            ]
        );
    }

    #[test]
    fn a_first_install_writes_every_config_with_the_bundle_first() {
        let latest = [
            file(CONFIG_FILE_TYPE, "config/a.toml", "1"),
            file(CONFIG_BUNDLE_FILE_TYPE, "configs.zip", "1"),
        ];

        let files = changed_config_files(&latest, None).unwrap();

        assert_eq!(
            names(&files),
            [
                (CONFIG_BUNDLE_FILE_TYPE, "configs.zip"),
                (CONFIG_FILE_TYPE, "config/a.toml"),
            ]
        );
    }

    #[test]
    fn limits_apply_to_the_version() {
        let mut half = file(CONFIG_FILE_TYPE, "config/a.toml", "1");
        half.file_size = Some((MAX_SHARED_FILES_TOTAL_SIZE / 2 + 1) as i64);
        let mut other_half = half.clone();
        other_half.file_name = "config/b.toml".to_string();

        assert!(changed_config_files(&[half.clone()], None).is_ok());
        assert!(changed_config_files(&[half, other_half], None).is_err());
    }
}
