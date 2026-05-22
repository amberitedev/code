use std::{io::Read, path::Path};

use crate::{
    application::social_models::SocialError,
    domain::modpack::{EnvType, PackFile},
};

pub fn is_client_only(file: &PackFile) -> bool {
    matches!(
        file.env.as_ref().map(|e| &e.server),
        Some(EnvType::Unsupported)
    )
}

pub async fn archive_mods(
    path: &Path,
) -> Result<Vec<(String, Vec<u8>)>, SocialError> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(
        move || -> Result<Vec<(String, Vec<u8>)>, SocialError> {
            let file = std::fs::File::open(path)?;
            let mut zip = zip::ZipArchive::new(file)
                .map_err(|e| SocialError::Invalid(e.to_string()))?;
            let mut mods = Vec::new();
            for i in 0..zip.len() {
                let mut entry = zip
                    .by_index(i)
                    .map_err(|e| SocialError::Invalid(e.to_string()))?;
                let name = entry.name().replace('\\', "/");
                let Some(filename) = name
                    .strip_prefix("overrides/mods/")
                    .or_else(|| name.strip_prefix("server-overrides/mods/"))
                else {
                    continue;
                };
                if !filename.ends_with(".jar") || entry.is_dir() {
                    continue;
                }
                validate_filename(filename)?;
                let mut data = Vec::new();
                entry.read_to_end(&mut data)?;
                mods.push((filename.to_string(), data));
            }
            Ok(mods)
        },
    )
    .await
    .map_err(|e| SocialError::Invalid(e.to_string()))?
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
