//! Addons as files in their folders: mods, plugins, datapacks, shaders and resource packs. A
//! disabled addon is its file with `.disabled` added; its row keeps the enabled path, so a pack
//! update treats a disabled pack file like one the owner removed and leaves it alone.

use modrinth_content_management::ContentType;

use crate::{
    installs::ModLoader,
    state::servers::{
        Error,
        adapters::filesystem::folder::{self, Folder},
    },
};

/// Added to an addon's file name to disable it.
pub const DISABLED: &str = ".disabled";

/// The folder addons of `kind` live in. Datapacks are inside the world; adding one on purpose is
/// the one way content goes there. A modpack is not an addon and has none.
pub fn kind_folder(
    kind: ContentType,
    level_name: &str,
) -> Result<String, Error> {
    Ok(match kind {
        ContentType::Mod => "mods".into(),
        ContentType::Plugin => "plugins".into(),
        ContentType::DataPack => format!("{level_name}/datapacks"),
        ContentType::Shader => "shaderpacks".into(),
        ContentType::ResourcePack => "resourcepacks".into(),
        ContentType::ModPack => {
            return Err(Error::Invalid("A modpack is not an addon.".into()));
        }
    })
}

/// The kinds a server with `loader` lists.
pub fn kinds(loader: Option<ModLoader>) -> Vec<ContentType> {
    match loader {
        Some(ModLoader::Paper) => {
            vec![ContentType::Plugin, ContentType::DataPack]
        }
        Some(ModLoader::Vanilla) | None => vec![ContentType::DataPack],
        Some(_) => vec![ContentType::Mod, ContentType::DataPack],
    }
}

/// One addon file on disk.
#[derive(Debug, Clone)]
pub struct Listed {
    /// The enabled path, which its row uses.
    pub path: String,
    pub kind: ContentType,
    pub filename: String,
    pub disabled: bool,
    pub size: u64,
    pub created: Option<chrono::DateTime<chrono::Utc>>,
}

/// Every addon file of the server's kinds.
pub fn scan(folder: &Folder, loader: Option<ModLoader>) -> Vec<Listed> {
    let level = folder.level_name();
    let mut listed = Vec::new();
    for kind in kinds(loader) {
        let Ok(dir) = kind_folder(kind, &level) else {
            continue;
        };
        let Ok(entries) =
            folder.open_dir(Some(&dir)).and_then(|dir| dir.entries())
        else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let (filename, disabled) = match name.strip_suffix(DISABLED) {
                Some(enabled) => (enabled.to_owned(), true),
                None => (name.clone(), false),
            };
            if !(filename.ends_with(".jar") || filename.ends_with(".zip")) {
                continue;
            }
            listed.push(Listed {
                path: format!("{dir}/{filename}"),
                kind,
                filename,
                disabled,
                size: meta.len(),
                created: meta
                    .created()
                    .or_else(|_| meta.modified())
                    .ok()
                    .map(|time| chrono::DateTime::from(time.into_std())),
            });
        }
    }
    listed.sort_by_key(|listed| listed.filename.to_lowercase());
    listed
}

/// Where an addon is on disk: its enabled path, and the path of the file there now.
pub fn locate(
    folder: &Folder,
    kind: ContentType,
    filename: &str,
) -> Result<(String, String), Error> {
    let path = addon_path(folder, kind, filename)?;
    let disabled = format!("{path}{DISABLED}");
    if folder.exists(&path) {
        Ok((path.clone(), path))
    } else if folder.exists(&disabled) {
        Ok((path, disabled))
    } else {
        Err(Error::NotFound(format!("There is no {filename}.")))
    }
}

/// The enabled path of the addon `filename` of `kind`, whether or not it is on disk.
pub fn addon_path(
    folder: &Folder,
    kind: ContentType,
    filename: &str,
) -> Result<String, Error> {
    let name = folder::clean(filename.trim_end_matches(DISABLED))?;
    if name.contains('/') {
        return Err(Error::Invalid(format!("{filename} is not a file name.")));
    }
    Ok(format!(
        "{}/{name}",
        kind_folder(kind, &folder.level_name())?
    ))
}

pub fn rename_enabled(
    folder: &Folder,
    path: &str,
    on_disk: &str,
    enabled: bool,
) -> Result<(), Error> {
    let wanted = if enabled {
        path.to_owned()
    } else {
        format!("{path}{DISABLED}")
    };
    if on_disk != wanted {
        folder.rename(on_disk, &wanted)?;
    }
    Ok(())
}

/// Find an addon by file name in any folder this server lists.
pub fn find(
    folder: &Folder,
    loader: Option<ModLoader>,
    filename: &str,
) -> Option<Listed> {
    let name = filename.trim_end_matches(DISABLED);
    scan(folder, loader)
        .into_iter()
        .find(|listed| listed.filename == name)
}
