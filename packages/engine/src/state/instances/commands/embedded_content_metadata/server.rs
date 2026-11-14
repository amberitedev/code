//! Copal's additions to upstream's reader: the metadata a server's content list shows for files
//! Modrinth does not know, such as CurseForge jars in a pack. It reads every file upstream reads
//! with upstream's parsers, and adds what a server needs: Paper and Bukkit plugins, the sides a
//! Fabric or Quilt mod names, and warnings when the metadata is malformed or lists several mods.

use std::{collections::HashMap, sync::Mutex, time::SystemTime};

use serde::Serialize;

use super::*;

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
        file: File,
        loader: Option<Platform>,
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
pub fn read_icon(file: File, path: &str) -> Option<Vec<u8>> {
    let mut archive = ZipArchive::new(file).ok()?;
    read_entry(&mut archive, &safe_archive_path(path)?, MAX_ICON_BYTES)
}

/// One metadata file a jar can carry: upstream's kinds, and plugins.
#[derive(Clone, Copy)]
enum Kind {
    Mod(ModMetadataKind),
    Plugin,
}

/// The metadata to read first is the server's loader's; a jar can carry several. Mods come in
/// upstream's order; a Paper server reads plugins first, any other server last.
fn order(loader: Option<Platform>) -> Vec<Kind> {
    let mod_loader = match loader {
        Some(Platform::Fabric) => Some(ModLoader::Fabric),
        Some(Platform::Quilt) => Some(ModLoader::Quilt),
        Some(Platform::Forge) => Some(ModLoader::Forge),
        Some(Platform::NeoForge) => Some(ModLoader::NeoForge),
        Some(Platform::Paper) | None => None,
    };
    let mods = metadata_order(mod_loader).map(Kind::Mod);
    if loader == Some(Platform::Paper) {
        std::iter::once(Kind::Plugin).chain(mods).collect()
    } else {
        mods.into_iter().chain([Kind::Plugin]).collect()
    }
}

/// What one metadata file gave.
struct Found {
    platform: Platform,
    name: Option<String>,
    version: Option<String>,
    environment: Option<Environment>,
    icon: Option<String>,
    /// How many mods the metadata lists, where it can list several.
    entries: Option<usize>,
}

/// What a jar says about itself, reading the metadata of `loader` first when a jar has several.
/// `None` when it has no metadata, or only malformed metadata and no `loader` to name.
pub fn inspect<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    loader: Option<Platform>,
) -> Option<Inspection> {
    let jar_manifest = read_manifest(archive);
    let mut malformed = false;
    let parsed = order(loader).into_iter().find_map(|kind| {
        let text = match kind {
            Kind::Mod(kind) => read_text_entry(archive, mod_path(kind)),
            Kind::Plugin => read_plugin(archive),
        }?;
        let found = match kind {
            Kind::Mod(kind) => parse_mod(archive, kind, &text, &jar_manifest),
            Kind::Plugin => parse_plugin(&text),
        };
        malformed |= found.is_none();
        found
    });
    let unreadable = parsed.is_none();
    let mut found = match (parsed, malformed) {
        (Some(found), _) => found,
        // A metadata file is there, but nothing could be read from it.
        (None, true) => Found {
            platform: loader?,
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

/// Where a kind of mod metadata is in the jar.
fn mod_path(kind: ModMetadataKind) -> &'static str {
    match kind {
        ModMetadataKind::Fabric => "fabric.mod.json",
        ModMetadataKind::Quilt => "quilt.mod.json",
        ModMetadataKind::NeoForge => "META-INF/neoforge.mods.toml",
        ModMetadataKind::Forge => "META-INF/mods.toml",
        ModMetadataKind::LegacyForge => "mcmod.info",
    }
}

/// A mod's metadata through upstream's parser, plus the sides and entry count upstream does not
/// read. `None` when upstream cannot read it.
fn parse_mod<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    kind: ModMetadataKind,
    text: &str,
    jar_manifest: &HashMap<String, String>,
) -> Option<Found> {
    let (metadata, icon) = match kind {
        ModMetadataKind::Fabric => parse_fabric_metadata(archive),
        ModMetadataKind::Quilt => parse_quilt_metadata(archive),
        ModMetadataKind::NeoForge => parse_toml_metadata(
            archive,
            "META-INF/neoforge.mods.toml",
            jar_manifest,
        ),
        ModMetadataKind::Forge => {
            parse_toml_metadata(archive, "META-INF/mods.toml", jar_manifest)
        }
        ModMetadataKind::LegacyForge => parse_legacy_forge_metadata(archive),
    }?;
    let (platform, environment, entries) = match kind {
        ModMetadataKind::Fabric => {
            let root = serde_json::from_str::<JsonValue>(text).ok()?;
            (
                Platform::Fabric,
                side(root.get("environment"), "server"),
                None,
            )
        }
        ModMetadataKind::Quilt => {
            let root = json5::from_str::<JsonValue>(text).ok()?;
            let environment = root
                .get("minecraft")
                .and_then(|minecraft| minecraft.get("environment"));
            (Platform::Quilt, side(environment, "dedicated_server"), None)
        }
        // Forge metadata has no sides of its own.
        ModMetadataKind::NeoForge | ModMetadataKind::Forge => {
            let root = toml::from_str::<TomlValue>(text).ok()?;
            let entries = root.get("mods")?.as_array()?.len();
            let platform = match kind {
                ModMetadataKind::NeoForge => Platform::NeoForge,
                _ => Platform::Forge,
            };
            (platform, None, Some(entries))
        }
        ModMetadataKind::LegacyForge => {
            let root = serde_json::from_str::<JsonValue>(text).ok()?;
            let entries = root
                .as_array()
                .or_else(|| root.get("modList").and_then(JsonValue::as_array))?
                .len();
            (Platform::Forge, None, Some(entries))
        }
    };
    Some(Found {
        platform,
        name: metadata.name,
        version: metadata.version,
        environment,
        icon,
        entries,
    })
}

/// A Fabric or Quilt `environment`: `*` or missing for both sides, `client`, or the server's
/// name for it.
fn side(value: Option<&JsonValue>, server: &str) -> Option<Environment> {
    match value.and_then(JsonValue::as_str) {
        None | Some("*") => Some(Environment::ClientAndServer),
        Some("client") => Some(Environment::ClientOnly),
        Some(other) if other == server => {
            Some(Environment::DedicatedServerOnly)
        }
        Some(_) => None,
    }
}

/// Reads a jar's `paper-plugin.yml`, or its `plugin.yml` when it has none.
fn read_plugin<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Option<String> {
    read_text_entry(archive, "paper-plugin.yml")
        .or_else(|| read_text_entry(archive, "plugin.yml"))
}

/// A Bukkit or Paper plugin's `name` and `version`, read from the top level of its YAML without
/// a YAML parser: they are plain `key: value` lines. Plugins have no logo and run only on a
/// server.
fn parse_plugin(text: &str) -> Option<Found> {
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
        let inspection = inspect(&mut archive, Some(Platform::Fabric)).unwrap();
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
    fn a_quilt_jar_is_read_as_json5() {
        let mut archive = jar(&[(
            "quilt.mod.json",
            b"{\n  // a comment\n  quilt_loader: { id: 'qsl', version: '7.0.0', metadata: { name: 'QSL' } },\n  minecraft: { environment: 'dedicated_server' },\n}",
        )]);
        let manifest = inspect(&mut archive, Some(Platform::Quilt))
            .unwrap()
            .manifest;
        assert_eq!(manifest.platform, Platform::Quilt);
        assert_eq!(manifest.name.as_deref(), Some("QSL"));
        assert_eq!(manifest.version.as_deref(), Some("7.0.0"));
        assert_eq!(
            manifest.environment,
            Some(Environment::DedicatedServerOnly)
        );
        assert!(!manifest.warnings.malformed);
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
        let manifest = inspect(&mut archive, Some(Platform::NeoForge))
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
    fn a_paper_server_reads_plugins_first() {
        let mut archive = jar(&[
            ("plugin.yml", b"name: Essentials\nversion: '2.20'\n"),
            ("fabric.mod.json", br#"{"id":"other"}"#),
        ]);
        let manifest = inspect(&mut archive, Some(Platform::Paper))
            .unwrap()
            .manifest;
        assert_eq!(manifest.platform, Platform::Paper);
        assert_eq!(manifest.name.as_deref(), Some("Essentials"));
        assert_eq!(manifest.version.as_deref(), Some("2.20"));
        let manifest = inspect(&mut archive, Some(Platform::Fabric))
            .unwrap()
            .manifest;
        assert_eq!(manifest.platform, Platform::Fabric);
    }

    #[test]
    fn unreadable_metadata_is_malformed() {
        let mut archive = jar(&[("fabric.mod.json", b"{ not json")]);
        let manifest = inspect(&mut archive, Some(Platform::Fabric))
            .unwrap()
            .manifest;
        assert!(manifest.warnings.malformed);
        assert_eq!(manifest.name, None);
        assert!(
            inspect(&mut jar(&[("a.txt", b"")]), Some(Platform::Fabric))
                .is_none()
        );
    }
}
