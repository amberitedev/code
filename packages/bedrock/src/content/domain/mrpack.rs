//! What a pack index says about the game it runs on.

use crate::content::model::mrpack::Index;
use crate::state::ModLoader;

/// Each loader's key in a pack's `dependencies`.
const LOADER_DEPENDENCIES: [(ModLoader, &str); 4] = [
    (ModLoader::Fabric, "fabric-loader"),
    (ModLoader::Quilt, "quilt-loader"),
    (ModLoader::Forge, "forge"),
    (ModLoader::NeoForge, "neoforge"),
];

/// `loader`'s key in a pack's `dependencies`; Vanilla has none.
pub fn loader_dependency(loader: ModLoader) -> Option<&'static str> {
    LOADER_DEPENDENCIES
        .iter()
        .find(|(known, _)| *known == loader)
        .map(|(_, key)| *key)
}

/// The loader and versions a pack runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackPlatform {
    pub loader: ModLoader,
    pub game_version: String,
    pub loader_version: Option<String>,
}

/// The platform `index` runs on, `None` when it names no Minecraft version. Older packs spell
/// NeoForge `neo-forge`, which upstream also reads.
pub fn platform(index: &Index) -> Option<PackPlatform> {
    let game_version = index.dependencies.get("minecraft")?.clone();
    let (loader, loader_version) = LOADER_DEPENDENCIES
        .iter()
        .chain(&[(ModLoader::NeoForge, "neo-forge")])
        .find_map(|(loader, key)| {
            index
                .dependencies
                .get(*key)
                .map(|version| (*loader, Some(version.clone())))
        })
        .unwrap_or((ModLoader::Vanilla, None));
    Some(PackPlatform {
        loader,
        game_version,
        loader_version,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn index(dependencies: &[(&str, &str)]) -> Index {
        Index {
            format_version: 1,
            game: "minecraft".into(),
            version_id: "1".into(),
            name: "Pack".into(),
            summary: None,
            files: Vec::new(),
            dependencies: dependencies
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    #[test]
    fn a_pack_names_its_loader_and_versions() {
        assert_eq!(
            platform(&index(&[
                ("minecraft", "1.21.1"),
                ("neo-forge", "21.1.1")
            ])),
            Some(PackPlatform {
                loader: ModLoader::NeoForge,
                game_version: "1.21.1".into(),
                loader_version: Some("21.1.1".into()),
            })
        );
        assert_eq!(
            platform(&index(&[("minecraft", "1.21.1")])).map(|p| p.loader),
            Some(ModLoader::Vanilla)
        );
        assert_eq!(platform(&index(&[("fabric-loader", "0.16.0")])), None);
        assert_eq!(loader_dependency(ModLoader::Quilt), Some("quilt-loader"));
        assert_eq!(loader_dependency(ModLoader::Vanilla), None);
    }
}
