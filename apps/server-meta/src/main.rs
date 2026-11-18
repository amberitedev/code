//! Amberite's server metadata generator, adapted from Modrinth's `meta-generator`. launcher-meta
//! describes how a client installs each loader version; this writes what a server needs on top,
//! a [`ServerProfile`] for each Forge and NeoForge build, at launcher-meta's path in `meta/`.
//!
//! `cargo run -p server-meta -- [--out meta] [--force] [loader version ids...]`
//!
//! Builds that already have a profile are skipped, so a run only reads new installers.

mod remote;
mod server;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use minecraft_meta_manifest::forgelike;
use minecraft_meta_manifest::minecraft::{self, Library, VersionManifest};
use minecraft_meta_manifest::profile::{ProfileMetadata, ServerProfile};
use serde::Deserialize;

use crate::remote::Remote;
use crate::server::{Build, Repository};

const LAUNCHER_META: &str = "https://launcher-meta.modrinth.com/";
const FORGE_CATALOG: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
const NEOFORGE_CATALOG: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/";
const WORKERS: usize = 16;

/// Builds launcher-meta lists that cannot be installed, from Modrinth's generator.
const BLACKLIST: &[&str] = &[
    // The `data` field is `[]` though its type is a map.
    "1.12.2-14.23.5.2851",
    // Its Forge jar names no main class.
    "1.12.2-14.23.5.2852",
    // Listed upstream, but the installer returns 404.
    "1.20.1-47.1.7",
    "47.1.82",
    // The installer references a missing universal jar.
    "21.4.31-beta",
];

#[derive(Deserialize)]
struct NeoforgeCatalog {
    versions: Vec<String>,
}

struct Job {
    /// launcher-meta's folder for the loader: `forge` or `neo`.
    folder: &'static str,
    game_version: String,
    id: String,
    url: String,
    repository: Repository,
    name: String,
}

fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let mut out = PathBuf::from("meta");
    let mut force = false;
    let mut only = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                out = args.next().context("--out needs a folder")?.into()
            }
            "--force" => force = true,
            _ => only.push(arg),
        }
    }

    let remote = Remote::new()?;
    let jobs = jobs(&remote)?;
    let jobs: Vec<Job> = jobs
        .into_iter()
        .filter(|job| only.is_empty() || only.contains(&job.id))
        .filter(|job| force || !current(&profile_path(&out, job)))
        .collect();
    println!("{} builds to generate", jobs.len());

    let minecraft: minecraft::Catalog = remote.json(&format!(
        "{LAUNCHER_META}minecraft/v{}/manifest.json",
        minecraft::CURRENT_FORMAT_VERSION
    ))?;
    let vanilla = Vanilla {
        urls: minecraft
            .versions
            .iter()
            .map(|version| (version.id.to_string(), version.url.to_string()))
            .collect(),
        cache: Mutex::default(),
    };

    let queue = Mutex::new(jobs.into_iter());
    let failures = Mutex::new(Vec::new());
    let done = Mutex::new(0usize);
    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            scope.spawn(|| {
                loop {
                    let Some(job) = queue.lock().unwrap().next() else {
                        break;
                    };
                    if let Err(error) =
                        generate(&remote, &vanilla, &out, &job, force)
                    {
                        failures.lock().unwrap().push(format!(
                            "{} {} ({}): {error:#}",
                            job.folder, job.id, job.name
                        ));
                    }
                    let mut done = done.lock().unwrap();
                    *done += 1;
                    if (*done).is_multiple_of(100) {
                        println!("{done} done");
                    }
                }
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    for failure in &failures {
        println!("failed: {failure}");
    }
    println!(
        "{} generated, {} failed",
        done.into_inner().unwrap() - failures.len(),
        failures.len()
    );
    Ok(())
}

/// Every Forge build from Minecraft 1.7.2, and every NeoForge build, that launcher-meta lists,
/// with its full maven name from the loader's own catalog.
fn jobs(remote: &Remote) -> Result<Vec<Job>> {
    let mut names = HashMap::<(&str, String), (Repository, String)>::new();
    let forge: HashMap<String, Vec<String>> = remote.json(FORGE_CATALOG)?;
    for name in forge.into_values().flatten() {
        let id = name.split('-').nth(1).unwrap_or(&name).to_owned();
        names.insert(("forge", id), (Repository::Forge, name));
    }
    for artifact in ["forge", "neoforge"] {
        let catalog: NeoforgeCatalog =
            remote.json(&format!("{NEOFORGE_CATALOG}{artifact}"))?;
        for name in catalog.versions {
            let id = neoforge_id(&name).to_owned();
            names.insert(("neo", id), (Repository::NeoForged(artifact), name));
        }
    }

    let mut jobs = Vec::new();
    for folder in ["forge", "neo"] {
        let manifest: forgelike::Manifest = remote
            .json(&format!("{LAUNCHER_META}{folder}/v0/manifest.json"))?;
        for game in manifest.game_versions {
            if folder == "forge"
                && (game.id.starts_with("1.5") || game.id.starts_with("1.6"))
            {
                continue;
            }
            for loader in game.loaders {
                let Some((repository, name)) =
                    names.get(&(folder, loader.id.clone()))
                else {
                    println!(
                        "skipped: {folder} {} has no maven name",
                        loader.id
                    );
                    continue;
                };
                if BLACKLIST.contains(&name.as_str()) {
                    continue;
                }
                jobs.push(Job {
                    folder,
                    game_version: game.id.clone(),
                    id: loader.id,
                    url: loader.url.to_string(),
                    repository: *repository,
                    name: name.clone(),
                });
            }
        }
    }
    Ok(jobs)
}

/// launcher-meta's id for a NeoForge build: the loader version alone, also for the 1.20.1
/// builds published as `1.20.1-47.1.106`.
fn neoforge_id(name: &str) -> &str {
    let numeric = |part: &str| {
        part.contains('.')
            && part.bytes().all(|b| b.is_ascii_digit() || b == b'.')
    };
    match name.split_once('-') {
        Some((game, version)) if numeric(game) && numeric(version) => version,
        _ => name,
    }
}

/// Vanilla version profiles, fetched once per game version.
struct Vanilla {
    urls: HashMap<String, String>,
    cache: Mutex<HashMap<String, std::sync::Arc<VersionManifest>>>,
}

impl Vanilla {
    fn get(
        &self,
        remote: &Remote,
        game_version: &str,
    ) -> Result<std::sync::Arc<VersionManifest>> {
        if let Some(found) = self.cache.lock().unwrap().get(game_version) {
            return Ok(found.clone());
        }
        let url = self.urls.get(game_version).with_context(|| {
            format!("launcher-meta has no Minecraft {game_version}")
        })?;
        let manifest =
            std::sync::Arc::new(remote.json::<VersionManifest>(url)?);
        self.cache
            .lock()
            .unwrap()
            .insert(game_version.to_owned(), manifest.clone());
        Ok(manifest)
    }
}

fn profile_path(out: &Path, job: &Job) -> PathBuf {
    out.join(job.folder)
        .join("v0/versions")
        .join(format!("{}.json", job.id))
}

fn generate(
    remote: &Remote,
    vanilla: &Vanilla,
    out: &Path,
    job: &Job,
    force: bool,
) -> Result<()> {
    let profile: ProfileMetadata = remote.json(&job.url)?;
    let path = profile_path(out, job);
    // A build's installer never changes, so what an earlier run read from it is kept.
    let server = match std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<FromInstaller>(&bytes).ok())
    {
        Some(earlier) if !force => ServerProfile {
            arguments: earlier.arguments,
            libraries: earlier.libraries,
            processors: server::server_steps(&profile)?,
        },
        _ => server::server_profile(
            remote,
            &Build {
                repository: job.repository,
                name: &job.name,
                profile: &profile,
                vanilla: &*vanilla.get(remote, &job.game_version)?,
            },
        )?,
    };
    std::fs::create_dir_all(path.parent().context("no parent")?)?;
    let mut json = serde_json::to_string(&server)?;
    json.push('\n');
    std::fs::write(&path, json)?;
    Ok(())
}

/// What a profile holds from its installer.
#[derive(Deserialize)]
struct FromInstaller {
    arguments: Vec<String>,
    libraries: Vec<Library>,
}

/// Whether the profile at `path` is in the current format.
fn current(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|bytes| {
        serde_json::from_slice::<ServerProfile>(&bytes).is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::neoforge_id;

    #[test]
    fn neoforge_ids_match_launcher_meta() {
        for (name, id) in [
            ("1.20.1-47.1.106", "47.1.106"),
            ("47.1.82", "47.1.82"),
            ("21.1.77", "21.1.77"),
            ("20.2.3-beta", "20.2.3-beta"),
            ("26.1.0.0-alpha.1+snapshot-1", "26.1.0.0-alpha.1+snapshot-1"),
        ] {
            assert_eq!(neoforge_id(name), id);
        }
    }
}
