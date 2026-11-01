//! What a mod or plugin jar says about itself: its name, version, sides, and logo. The content
//! list shows these for files Modrinth does not know, such as CurseForge jars in a pack. Ported
//! from app-lib's `embedded_content_metadata`, which does the same for local instances.

use std::{
    collections::HashMap,
    io::{Read, Seek},
    sync::Mutex,
    time::SystemTime,
};

use serde::Serialize;
use serde_json::Value as Json;
use toml::Value as Toml;
use zip::ZipArchive;

use crate::installs::ModLoader;

const MAX_METADATA_BYTES: u64 = 1024 * 1024;
const MAX_ICON_BYTES: u64 = 8 * 1024 * 1024;
/// The icon size picked from a Fabric or Quilt size map: the smallest at least this big.
const PREFERRED_ICON_SIZE: u32 = 96;
const MAX_NAME_CHARS: usize = 256;
const MAX_VERSION_CHARS: usize = 128;

/// The loader a jar's metadata is for. Mirrors Archon.Content.v1.Modloader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Fabric,
    Quilt,
    NeoForge,
    Forge,
    Paper,
}

/// Mirrors Archon.Content.v1.AddonManifestEnvironment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    ClientAndServer,
    ClientOnly,
    DedicatedServerOnly,
}

/// Mirrors Archon.Content.v1.AddonManifestWarnings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Warnings {
    /// How many mods the metadata lists, when it lists more than one.
    pub multiple_mod_entries: Option<usize>,
    /// The jar has a metadata file that could not be read.
    pub malformed: bool,
}

/// An addon's `manifest`. Mirrors Archon.Content.v1.AddonManifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Manifest {
    pub platform: Platform,
    pub name: Option<String>,
    pub version: Option<String>,
    pub environment: Option<Environment>,
    pub icon_embedded: bool,
    pub warnings: Warnings,
}

/// A jar's manifest, and where its logo is inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection {
    pub manifest: Manifest,
    pub icon: Option<String>,
}

/// Inspections by file, kept while the file's size and modified time stay the same, so listing
/// content opens each jar's metadata once.
#[derive(Default)]
pub struct Cache(Mutex<HashMap<String, (u64, SystemTime, Option<Inspection>)>>);

impl Cache {
    /// The inspection of `file`, cached under `key`. `None` when it is not a jar with metadata.
    pub fn get(
        &self,
        key: &str,
        file: std::fs::File,
        loader: Option<ModLoader>,
    ) -> Option<Inspection> {
        let meta = file.metadata().ok()?;
        let stamp = (meta.len(), meta.modified().ok()?);
        if let Some((len, modified, inspection)) =
            self.0.lock().expect("not poisoned").get(key)
            && (*len, *modified) == stamp
        {
            return inspection.clone();
        }
        let inspection = ZipArchive::new(file)
            .ok()
            .and_then(|mut archive| inspect(&mut archive, loader));
        self.0
            .lock()
            .expect("not poisoned")
            .insert(key.to_owned(), (stamp.0, stamp.1, inspection.clone()));
        inspection
    }
}

/// The logo at `path` inside the jar `file`.
pub fn read_icon(file: std::fs::File, path: &str) -> Option<Vec<u8>> {
    let mut archive = ZipArchive::new(file).ok()?;
    read_entry(&mut archive, &safe_archive_path(path)?, MAX_ICON_BYTES)
}

/// What a jar says about itself, reading the metadata of `loader` first when a jar has several.
pub fn inspect<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    loader: Option<ModLoader>,
) -> Option<Inspection> {
    let jar_manifest = read_jar_manifest(archive);
    let mut malformed = false;
    let parsed = order(loader).into_iter().find_map(|kind| {
        let parsed = match kind {
            Kind::Fabric => parse_fabric(archive),
            Kind::Quilt => parse_quilt(archive),
            Kind::NeoForge => parse_toml(
                archive,
                "META-INF/neoforge.mods.toml",
                Platform::NeoForge,
                &jar_manifest,
            ),
            Kind::Forge => parse_toml(
                archive,
                "META-INF/mods.toml",
                Platform::Forge,
                &jar_manifest,
            ),
            Kind::LegacyForge => parse_legacy_forge(archive),
            Kind::Plugin => parse_plugin(archive),
        };
        match parsed {
            Parsed::Missing => None,
            Parsed::Malformed => {
                malformed = true;
                None
            }
            Parsed::Found(found) => Some(found),
        }
    });
    // Every metadata file the jar has failed to parse.
    let unreadable = parsed.is_none();
    let mut found = match (parsed, malformed) {
        (Some(found), _) => found,
        // A metadata file is there, but nothing could be read from it.
        (None, true) => Found {
            platform: platform_of(loader)?,
            name: None,
            version: None,
            environment: None,
            icon: None,
            entries: None,
        },
        (None, false) => return None,
    };
    if found.name.is_none() {
        found.name = jar_manifest
            .get("implementation-title")
            .and_then(|value| clean_string(value, MAX_NAME_CHARS));
    }
    if found.version.is_none() {
        found.version = jar_manifest
            .get("implementation-version")
            .and_then(|value| clean_string(value, MAX_VERSION_CHARS));
    }
    let icon = found
        .icon
        .filter(|path| entry_fits(archive, path, MAX_ICON_BYTES));
    Some(Inspection {
        manifest: Manifest {
            platform: found.platform,
            name: found.name,
            version: found.version,
            environment: found.environment,
            icon_embedded: icon.is_some(),
            warnings: Warnings {
                multiple_mod_entries: found.entries.filter(|count| *count > 1),
                malformed: unreadable,
            },
        },
        icon,
    })
}

#[derive(Clone, Copy)]
enum Kind {
    Fabric,
    Quilt,
    NeoForge,
    Forge,
    LegacyForge,
    Plugin,
}

/// The metadata to read first is the server's loader's; a jar can carry several.
fn order(loader: Option<ModLoader>) -> [Kind; 6] {
    use Kind::*;
    match loader {
        Some(ModLoader::Quilt) => {
            [Quilt, Fabric, NeoForge, Forge, LegacyForge, Plugin]
        }
        Some(ModLoader::Forge) => {
            [Forge, LegacyForge, NeoForge, Fabric, Quilt, Plugin]
        }
        Some(ModLoader::NeoForge) => {
            [NeoForge, Forge, LegacyForge, Fabric, Quilt, Plugin]
        }
        Some(ModLoader::Paper) => {
            [Plugin, Fabric, Quilt, NeoForge, Forge, LegacyForge]
        }
        _ => [Fabric, Quilt, NeoForge, Forge, LegacyForge, Plugin],
    }
}

fn platform_of(loader: Option<ModLoader>) -> Option<Platform> {
    Some(match loader? {
        ModLoader::Fabric => Platform::Fabric,
        ModLoader::Quilt => Platform::Quilt,
        ModLoader::NeoForge => Platform::NeoForge,
        ModLoader::Forge => Platform::Forge,
        ModLoader::Paper => Platform::Paper,
        ModLoader::Vanilla => return None,
    })
}

struct Found {
    platform: Platform,
    name: Option<String>,
    version: Option<String>,
    environment: Option<Environment>,
    icon: Option<String>,
    /// How many mods the metadata lists, where it can list several.
    entries: Option<usize>,
}

enum Parsed {
    Missing,
    Malformed,
    Found(Found),
}

impl Parsed {
    fn from(
        text: Option<String>,
        parse: impl FnOnce(&str) -> Option<Found>,
    ) -> Self {
        match text {
            None => Self::Missing,
            Some(text) => parse(&text).map_or(Self::Malformed, Self::Found),
        }
    }
}

fn parse_fabric<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Parsed {
    Parsed::from(read_text(archive, "fabric.mod.json"), |text| {
        let root: Json = serde_json::from_str(text).ok()?;
        Some(Found {
            platform: Platform::Fabric,
            name: json_str(&root, "name")
                .or_else(|| json_str(&root, "id"))
                .and_then(|value| clean_string(value, MAX_NAME_CHARS)),
            version: root
                .get("version")
                .and_then(json_scalar)
                .and_then(|value| clean_string(&value, MAX_VERSION_CHARS)),
            environment: side(root.get("environment"), "server"),
            icon: root.get("icon").and_then(json_icon_path),
            entries: None,
        })
    })
}

fn parse_quilt<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Parsed {
    Parsed::from(read_text(archive, "quilt.mod.json"), |text| {
        let root: Json = serde_json::from_str(text).ok()?;
        let loader = root.get("quilt_loader").unwrap_or(&root);
        let display = loader.get("metadata").unwrap_or(loader);
        Some(Found {
            platform: Platform::Quilt,
            name: json_str(display, "name")
                .or_else(|| json_str(loader, "id"))
                .and_then(|value| clean_string(value, MAX_NAME_CHARS)),
            version: loader
                .get("version")
                .and_then(json_scalar)
                .and_then(|value| clean_string(&value, MAX_VERSION_CHARS)),
            environment: side(
                root.get("minecraft")
                    .and_then(|minecraft| minecraft.get("environment")),
                "dedicated_server",
            ),
            icon: display
                .get("icon")
                .or_else(|| loader.get("icon"))
                .and_then(json_icon_path),
            entries: None,
        })
    })
}

/// A Fabric or Quilt `environment`: `*` or missing for both sides, `client`, or the server's
/// name for it.
fn side(value: Option<&Json>, server: &str) -> Option<Environment> {
    match value.and_then(Json::as_str) {
        None | Some("*") => Some(Environment::ClientAndServer),
        Some("client") => Some(Environment::ClientOnly),
        Some(other) if other == server => {
            Some(Environment::DedicatedServerOnly)
        }
        Some(_) => None,
    }
}

fn parse_toml<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
    platform: Platform,
    jar_manifest: &HashMap<String, String>,
) -> Parsed {
    Parsed::from(read_text(archive, path), |text| {
        let root: Toml = toml::from_str(text).ok()?;
        let mods = root.get("mods")?.as_array()?;
        let first = mods.first()?.as_table()?;
        Some(Found {
            platform,
            name: first
                .get("displayName")
                .or_else(|| first.get("modId"))
                .and_then(toml_scalar)
                .and_then(|value| clean_string(&value, MAX_NAME_CHARS)),
            version: first
                .get("version")
                .and_then(toml_scalar)
                .and_then(|value| {
                    resolve_toml_value(&value, &root, jar_manifest)
                })
                .and_then(|value| clean_string(&value, MAX_VERSION_CHARS)),
            // Forge metadata has no sides of its own.
            environment: None,
            icon: first
                .get("logoFile")
                .and_then(Toml::as_str)
                .and_then(safe_archive_path),
            entries: Some(mods.len()),
        })
    })
}

fn parse_legacy_forge<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Parsed {
    Parsed::from(read_text(archive, "mcmod.info"), |text| {
        let root: Json = serde_json::from_str(text).ok()?;
        let entries = root
            .as_array()
            .or_else(|| root.get("modList").and_then(Json::as_array))?;
        let first = entries.first()?;
        Some(Found {
            platform: Platform::Forge,
            name: json_str(first, "name")
                .or_else(|| json_str(first, "modid"))
                .and_then(|value| clean_string(value, MAX_NAME_CHARS)),
            version: first
                .get("version")
                .and_then(json_scalar)
                .and_then(|value| clean_string(&value, MAX_VERSION_CHARS)),
            environment: None,
            icon: json_str(first, "logoFile").and_then(safe_archive_path),
            entries: Some(entries.len()),
        })
    })
}

/// A Bukkit or Paper plugin's `name` and `version`, read from the top level of its YAML without
/// a YAML parser: they are plain `key: value` lines. Plugins have no logo and run only on a
/// server.
fn parse_plugin<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Parsed {
    let text = read_text(archive, "paper-plugin.yml")
        .or_else(|| read_text(archive, "plugin.yml"));
    Parsed::from(text, |text| {
        let top = |key: &str| {
            text.lines().find_map(|line| {
                let value = line.strip_prefix(key)?.strip_prefix(':')?.trim();
                Some(value.trim_matches(|c| c == '"' || c == '\'').to_owned())
            })
        };
        let name =
            top("name").and_then(|value| clean_string(&value, MAX_NAME_CHARS));
        name.as_ref()?;
        Some(Found {
            platform: Platform::Paper,
            name,
            version: top("version")
                .and_then(|value| clean_string(&value, MAX_VERSION_CHARS)),
            environment: Some(Environment::DedicatedServerOnly),
            icon: None,
            entries: None,
        })
    })
}

/// A Forge `version` of `${file.jarVersion}` or `${file.<property>}`, filled in.
fn resolve_toml_value(
    value: &str,
    root: &Toml,
    jar_manifest: &HashMap<String, String>,
) -> Option<String> {
    if value == "${file.jarVersion}" {
        return jar_manifest.get("implementation-version").cloned();
    }
    if let Some(key) = value
        .strip_prefix("${file.")
        .and_then(|value| value.strip_suffix('}'))
    {
        return root
            .get("properties")
            .and_then(Toml::as_table)
            .and_then(|properties| properties.get(key))
            .and_then(toml_scalar);
    }
    Some(value.to_owned())
}

/// A Fabric or Quilt `icon`: one path, or paths by size.
fn json_icon_path(value: &Json) -> Option<String> {
    if let Some(path) = value.as_str() {
        return safe_archive_path(path);
    }
    let mut by_size: Vec<(u32, String)> = value
        .as_object()?
        .iter()
        .filter_map(|(size, path)| {
            Some((size.parse().ok()?, safe_archive_path(path.as_str()?)?))
        })
        .collect();
    by_size.sort_by_key(|(size, _)| *size);
    by_size
        .iter()
        .find(|(size, _)| *size >= PREFERRED_ICON_SIZE)
        .or_else(|| by_size.last())
        .map(|(_, path)| path.clone())
}

/// `path` as a plain path inside an archive, or `None` when it could leave it.
fn safe_archive_path(path: &str) -> Option<String> {
    let path = path.trim().trim_start_matches('/');
    let unsafe_path = path.is_empty()
        || path.contains(['\0', '\\'])
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    (!unsafe_path).then(|| path.to_owned())
}

/// `value` trimmed, without control characters, and at most `max_chars` long. `None` when it is
/// empty or an unfilled `${...}` placeholder.
fn clean_string(value: &str, max_chars: usize) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.contains("${") {
        return None;
    }
    let value: String = value
        .chars()
        .filter(|c| !c.is_control())
        .take(max_chars)
        .collect();
    (!value.is_empty()).then_some(value)
}

fn json_str<'a>(value: &'a Json, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Json::as_str)
}

fn json_scalar(value: &Json) -> Option<String> {
    match value {
        Json::String(value) => Some(value.clone()),
        Json::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn toml_scalar(value: &Toml) -> Option<String> {
    match value {
        Toml::String(value) => Some(value.clone()),
        Toml::Integer(value) => Some(value.to_string()),
        Toml::Float(value) => Some(value.to_string()),
        _ => None,
    }
}

/// `META-INF/MANIFEST.MF`'s attributes, by lowercase name.
fn read_jar_manifest<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> HashMap<String, String> {
    let Some(text) = read_text(archive, "META-INF/MANIFEST.MF") else {
        return HashMap::new();
    };
    let mut attributes: HashMap<String, String> = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        // A line starting with a space continues the one before.
        if let Some(rest) = line.strip_prefix(' ') {
            if let Some(value) =
                current.as_ref().and_then(|key| attributes.get_mut(key))
            {
                value.push_str(rest);
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            current = None;
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        attributes.insert(key.clone(), value.trim_start().to_owned());
        current = Some(key);
    }
    attributes
}

fn read_text<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Option<String> {
    String::from_utf8(read_entry(archive, path, MAX_METADATA_BYTES)?).ok()
}

fn entry_fits<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
    max_bytes: u64,
) -> bool {
    archive
        .by_name(path)
        .is_ok_and(|entry| entry.size() <= max_bytes)
}

/// The entry at `path`, read up to `max_bytes`; `None` when it is missing or bigger, whatever
/// its header claims.
fn read_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
    max_bytes: u64,
) -> Option<Vec<u8>> {
    let entry = archive.by_name(path).ok()?;
    if entry.size() > max_bytes {
        return None;
    }
    let mut bytes = Vec::new();
    entry.take(max_bytes + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= max_bytes).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::*;

    fn jar(files: &[(&str, &[u8])]) -> ZipArchive<Cursor<Vec<u8>>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in files {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        ZipArchive::new(writer.finish().unwrap()).unwrap()
    }

    #[test]
    fn a_fabric_jar_names_itself_and_its_logo() {
        let mut archive = jar(&[
            (
                "fabric.mod.json",
                br#"{"id":"sodium","name":"Sodium","version":"0.6.0","environment":"client","icon":{"32":"a.png","128":"b.png"}}"#,
            ),
            ("b.png", b"png"),
        ]);
        let inspection =
            inspect(&mut archive, Some(ModLoader::Fabric)).unwrap();
        assert_eq!(inspection.manifest.platform, Platform::Fabric);
        assert_eq!(inspection.manifest.name.as_deref(), Some("Sodium"));
        assert_eq!(inspection.manifest.version.as_deref(), Some("0.6.0"));
        assert_eq!(
            inspection.manifest.environment,
            Some(Environment::ClientOnly)
        );
        assert!(inspection.manifest.icon_embedded);
        assert_eq!(inspection.icon.as_deref(), Some("b.png"));
    }

    #[test]
    fn a_forge_jar_takes_its_version_from_the_jar_manifest() {
        let mut archive = jar(&[
            (
                "META-INF/neoforge.mods.toml",
                b"[[mods]]\nmodId=\"jei\"\ndisplayName=\"JEI\"\nversion=\"${file.jarVersion}\"\nlogoFile=\"../escape.png\"\n[[mods]]\nmodId=\"other\"\n",
            ),
            ("META-INF/MANIFEST.MF", b"Implementation-Version: 19.0.1\n"),
        ]);
        let manifest = inspect(&mut archive, Some(ModLoader::NeoForge))
            .unwrap()
            .manifest;
        assert_eq!(manifest.platform, Platform::NeoForge);
        assert_eq!(manifest.version.as_deref(), Some("19.0.1"));
        assert!(
            !manifest.icon_embedded,
            "a logo outside the jar is not read"
        );
        assert_eq!(manifest.warnings.multiple_mod_entries, Some(2));
    }

    #[test]
    fn unreadable_metadata_is_malformed() {
        let mut archive = jar(&[("fabric.mod.json", b"{ not json")]);
        let manifest = inspect(&mut archive, Some(ModLoader::Fabric))
            .unwrap()
            .manifest;
        assert!(manifest.warnings.malformed);
        assert_eq!(manifest.name, None);
        assert!(
            inspect(&mut jar(&[("a.txt", b"")]), Some(ModLoader::Fabric))
                .is_none()
        );
    }
}
