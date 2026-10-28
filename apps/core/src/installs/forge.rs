//! Forge and NeoForge. Versions come from launcher-meta, picked by the App's rule.
//!
//! Every NeoForge build, and Forge from 1.17, installs from launcher-meta's server-side data. Core
//! downloads the libraries into the shared store. The install steps (Forge's "processors") run
//! as `copal-servers` in a work folder inside the server folder, because they run code (see
//! `Servers::run_installer`). Core checks what they made against the hashes launcher-meta
//! publishes, then moves it into the store. The server starts from the store, so a build is
//! installed once for every server on it.
//!
//! Older Forge has no server start launcher-meta can describe. It still runs the vendor
//! installer in the server folder and starts from what that leaves behind.

use sha1::Digest;
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
};

use minecraft_meta_manifest::{
    fabriclike::LoaderVersion,
    profile::{DUMMY_REPLACE_STRING, ProfileMetadata},
};

use super::{
    ModLoader,
    download::{self, FileDownload, Hash},
    shared::{
        self, SharedDirs,
        libraries::{self, LibraryError, maven_path},
        meta,
        rules::classpath_separator,
        version_info::{self, merged_version_id},
    },
};
use crate::servers::{Hold, ServerError, Servers};

#[derive(Debug, thiserror::Error)]
pub enum InstallerError {
    #[error(transparent)]
    Download(#[from] download::DownloadError),
    #[error(transparent)]
    Metadata(#[from] meta::MetaError),
    #[error(transparent)]
    Shared(#[from] shared::SharedError),
    #[error("version info: {0}")]
    VersionInfo(#[from] version_info::VersionInfoError),
    #[error("library: {0}")]
    Library(#[from] LibraryError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Server(#[from] ServerError),
    #[error(
        "the {0} installer finished but left nothing to start the server with"
    )]
    NoLaunch(ModLoader),
    #[error("{0:?} is not a version")]
    BadVersion(String),
    #[error("the install step {0} has no main class")]
    NoMainClass(String),
    #[error("the install made a damaged {0}")]
    Damaged(String),
    #[error("the install did not make {0}, which the server starts with")]
    Incomplete(String),
}

/// `version`, unless it is not plainly one (see [`super::plain_version`]).
fn checked(version: &str) -> Result<&str, InstallerError> {
    if super::plain_version(version) {
        Ok(version)
    } else {
        Err(InstallerError::BadVersion(version.to_owned()))
    }
}

/// The loader version to install, from launcher-meta: `requested`, or the App's default for
/// `None`. A version launcher-meta does not list fails here, before anything downloads.
pub async fn resolve(
    http: &reqwest::Client,
    loader: ModLoader,
    game_version: &str,
    requested: Option<&str>,
) -> Result<LoaderVersion, InstallerError> {
    let version =
        meta::loader_version(http, loader, game_version, requested).await?;
    checked(&version.id)?;
    Ok(version)
}

/// Fetch the installer for `version` into the shared store, unless it is there. Checked against
/// the SHA-1 the maven publishes beside it, when it does.
pub async fn installer(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    loader: ModLoader,
    game_version: &str,
    version: &str,
) -> Result<PathBuf, InstallerError> {
    let (game_version, version) = (checked(game_version)?, checked(version)?);
    let (name, url) = match loader {
        ModLoader::NeoForge => {
            let name = format!("neoforge-{version}-installer.jar");
            let url = format!(
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/{name}"
            );
            (name, url)
        }
        _ => {
            let name = format!("forge-{game_version}-{version}-installer.jar");
            let url = format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{game_version}-{version}/{name}"
            );
            (name, url)
        }
    };
    let path = dirs.installer(&name);
    let sha1 = download::bytes(http, &format!("{url}.sha1"))
        .await
        .ok()
        .and_then(|text| {
            let text = String::from_utf8_lossy(&text);
            let hash = text.split_whitespace().next()?.to_owned();
            (hash.len() == 40).then_some(hash)
        });
    let hash = sha1.as_deref().map_or(Hash::Unknown, Hash::Sha1);
    if !download::has_hash(&path, hash).await {
        download::file(
            http,
            &url,
            &path,
            &FileDownload {
                hash,
                limit: Some(256 * 1024 * 1024),
                progress: &download::quiet,
            },
        )
        .await?;
    }
    Ok(path)
}

/// The Java arguments this platform's server starts with, as the installer ships them
/// (`data/win_args.txt` or `data/unix_args.txt`), or `None` for Forge before 1.17, which has
/// none. Read from the jar; nothing in it runs.
pub async fn server_args(
    installer: &Path,
) -> Result<Option<String>, InstallerError> {
    let installer = installer.to_owned();
    let name = if cfg!(windows) {
        "data/win_args.txt"
    } else {
        "data/unix_args.txt"
    };
    let read = tokio::task::spawn_blocking(move || {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&installer)?)
            .map_err(std::io::Error::other)?;
        let Ok(entry) = zip.by_name(name) else {
            return Ok(None);
        };
        let mut text = String::new();
        entry.take(1024 * 1024).read_to_string(&mut text)?;
        Ok::<_, std::io::Error>(Some(text))
    })
    .await
    .map_err(std::io::Error::other)??;
    Ok(read)
}

/// Install `version` from launcher-meta into the shared store and record how it starts. `args`
/// is the installer's [`server_args`]. A build another server already installed is reused unless
/// `verify` (repair) checks it again. `work` is an empty folder inside the server folder where
/// the install steps run; the caller removes it. `progress` is told the share done.
#[allow(clippy::too_many_arguments)]
pub async fn install_from_meta(
    servers: &Servers,
    hold: &Hold,
    http: &reqwest::Client,
    dirs: &SharedDirs,
    loader: ModLoader,
    game_version: &str,
    version: &LoaderVersion,
    installer: &Path,
    args: &str,
    java: &Path,
    work: &Path,
    verify: bool,
    progress: &(dyn Fn(f64) + Sync),
) -> Result<(), InstallerError> {
    let id = merged_version_id(game_version, loader, Some(&version.id));
    let catalog = meta::fetch_minecraft_manifest(http).await?;
    let game = meta::find_game_version(&catalog, game_version)?;
    let vanilla = version_info::vanilla(http, dirs, &game, verify).await?;
    shared::download_server_jar(http, dirs, &vanilla, game_version, verify)
        .await?;
    let mut profile: ProfileMetadata =
        download::json(http, version.url.as_str()).await?;
    for library in &mut profile.libraries {
        library.name = library.name.replace(DUMMY_REPLACE_STRING, game_version);
    }
    progress(0.1);
    libraries::download_libraries(
        http,
        dirs,
        &profile.libraries,
        std::env::consts::ARCH,
        verify,
        &|share| progress(0.1 + share * 0.4),
    )
    .await?;
    if !verify && dirs.server_args_path(&id).is_file() {
        progress(1.0);
        return Ok(());
    }

    let steps = Steps::new(dirs, &profile, game_version, installer, work)?;
    let processors: Vec<_> = profile
        .processors
        .iter()
        .flatten()
        .filter(|processor| {
            processor
                .sides
                .as_ref()
                .is_none_or(|sides| sides.iter().any(|side| side == "server"))
        })
        .collect();
    let sep = classpath_separator(std::env::consts::ARCH);
    for (done, processor) in processors.iter().enumerate() {
        let classpath = std::iter::once(&processor.jar)
            .chain(&processor.classpath)
            .map(|name| steps.store_path(name))
            .collect::<Result<Vec<_>, _>>()?;
        let main = main_class(&classpath[0]).await?.ok_or_else(|| {
            InstallerError::NoMainClass(processor.jar.clone())
        })?;
        let mut command =
            vec!["-cp".to_owned(), join_paths(&classpath, sep), main];
        for arg in &processor.args {
            command.push(steps.argument(arg)?);
        }
        servers
            .run_installer(hold, java.to_owned(), command)
            .await?;
        progress(0.5 + (done + 1) as f64 / processors.len() as f64 * 0.4);
    }
    store_outputs(&steps).await?;

    let libraries_dir = dunce::canonicalize(&steps.store)?;
    let (tokens, referenced) =
        absolute_args(args, &libraries_dir.to_string_lossy(), sep);
    if let Some(missing) = referenced
        .iter()
        .find(|path| !libraries_dir.join(path).is_file())
    {
        return Err(InstallerError::Incomplete(missing.clone()));
    }
    // Written last, so it is there only once the build is complete.
    download::write_whole(
        &dirs.server_args_path(&id),
        tokens.join("\n").as_bytes(),
    )
    .await?;
    progress(1.0);
    Ok(())
}

/// The values a Forge install step's arguments refer to, for the server side. A library
/// launcher-meta lists is read from the shared store. Anything else an argument names is made by
/// a step, so it is written to the work folder until Core moves it into the store.
struct Steps {
    store: PathBuf,
    made: PathBuf,
    listed: HashSet<String>,
    data: HashMap<String, String>,
}

impl Steps {
    fn new(
        dirs: &SharedDirs,
        profile: &ProfileMetadata,
        game_version: &str,
        installer: &Path,
        work: &Path,
    ) -> Result<Self, InstallerError> {
        let path = |path: &Path| path.to_string_lossy().into_owned();
        let made = work.join("libraries");
        let mut steps = Self {
            store: dirs.libraries_dir(),
            listed: profile
                .libraries
                .iter()
                .filter(|library| library.downloadable)
                .map(|library| library.name.clone())
                .collect(),
            data: HashMap::from([
                ("SIDE".to_owned(), "server".to_owned()),
                (
                    "MINECRAFT_JAR".to_owned(),
                    path(&dirs.server_jar_path(game_version)),
                ),
                ("MINECRAFT_VERSION".to_owned(), game_version.to_owned()),
                ("ROOT".to_owned(), path(work)),
                ("INSTALLER".to_owned(), path(installer)),
                ("LIBRARY_DIR".to_owned(), path(&made)),
            ]),
            made,
        };
        for (key, entry) in profile.data.iter().flatten() {
            let value = steps.value(&entry.server)?;
            steps.data.insert(key.clone(), value);
        }
        Ok(steps)
    }

    /// A data value: `[coordinates]` is a library's path, `'text'` is the text.
    fn value(&self, value: &str) -> Result<String, InstallerError> {
        if let Some(name) = value
            .strip_prefix('[')
            .and_then(|name| name.strip_suffix(']'))
        {
            return Ok(self.library(name)?.to_string_lossy().into_owned());
        }
        Ok(value
            .strip_prefix('\'')
            .and_then(|text| text.strip_suffix('\''))
            .unwrap_or(value)
            .to_owned())
    }

    /// Where a library is: in the store if launcher-meta lists it, else in the work folder.
    fn library(&self, name: &str) -> Result<PathBuf, InstallerError> {
        let root = if self.listed.contains(name) {
            &self.store
        } else {
            &self.made
        };
        Ok(root.join(maven_path(name)?))
    }

    /// A library on a step's classpath, which launcher-meta always lists.
    fn store_path(&self, name: &str) -> Result<PathBuf, InstallerError> {
        Ok(self.store.join(maven_path(name)?))
    }

    /// An install step argument: a library, or text with `{KEY}` replaced by its data value
    /// anywhere in it, such as `{ROOT}/libraries/`. app-lib's `get_processor_arguments`.
    fn argument(&self, arg: &str) -> Result<String, InstallerError> {
        if arg.starts_with('[') {
            return self.value(arg);
        }
        let mut arg = arg.to_owned();
        for (key, value) in &self.data {
            arg = arg.replace(&format!("{{{key}}}"), value);
        }
        Ok(arg)
    }

    /// The hash launcher-meta publishes for each file a step makes (`KEY` beside `KEY_SHA`), by
    /// its path under the work folder's `libraries/`.
    fn published(&self) -> HashMap<String, String> {
        self.data
            .iter()
            .filter_map(|(key, sha1)| {
                let made = self.data.get(key.strip_suffix("_SHA")?)?;
                let relative = Path::new(made).strip_prefix(&self.made).ok()?;
                Some((slashed(relative), sha1.to_ascii_lowercase()))
            })
            .collect()
    }
}

/// A relative path with `/` between its parts, on every platform.
fn slashed(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// The `Main-Class` of a jar's manifest. app-lib's `get_processor_main_class`.
async fn main_class(jar: &Path) -> Result<Option<String>, InstallerError> {
    let jar = jar.to_owned();
    let found = tokio::task::spawn_blocking(move || {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&jar)?)
            .map_err(std::io::Error::other)?;
        let mut manifest = String::new();
        zip.by_name("META-INF/MANIFEST.MF")
            .map_err(std::io::Error::other)?
            .take(1024 * 1024)
            .read_to_string(&mut manifest)?;
        Ok::<_, std::io::Error>(manifest.lines().find_map(|line| {
            line.strip_prefix("Main-Class:")
                .map(|class| class.trim().to_owned())
        }))
    })
    .await
    .map_err(std::io::Error::other)??;
    Ok(found)
}

fn join_paths(paths: &[PathBuf], sep: &str) -> String {
    paths
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join(sep)
}

/// Move what the install steps made into the store, each file whole. The steps run as
/// `copal-servers`, which can still change their work folder, so every file is read once,
/// through the folder so a planted link is never followed, and the bytes read are the ones
/// checked against launcher-meta's hash and stored. A library launcher-meta lists is never
/// replaced: Core downloaded and checked it itself. A file the store already has with the same
/// bytes is left alone, so a running server never sees it change.
async fn store_outputs(steps: &Steps) -> Result<(), InstallerError> {
    let published = steps.published();
    let listed = steps
        .listed
        .iter()
        .map(|name| maven_path(name))
        .collect::<Result<HashSet<_>, _>>()?;
    let made = steps.made.clone();
    let files = tokio::task::spawn_blocking(move || {
        walkdir::WalkDir::new(&made)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter_map(|entry| {
                Some(slashed(entry.path().strip_prefix(&made).ok()?))
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(std::io::Error::other)?;
    for relative in files.into_iter().filter(|file| !listed.contains(file)) {
        let (made, path) = (steps.made.clone(), relative.clone());
        let bytes = tokio::task::spawn_blocking(move || {
            let mut bytes = Vec::new();
            let file = crate::content::folder::Folder::open(&made)?
                .open_file(&path)?
                .ok_or_else(|| {
                    std::io::Error::from(std::io::ErrorKind::NotFound)
                })?;
            file.into_std().read_to_end(&mut bytes)?;
            Ok::<_, std::io::Error>(bytes)
        })
        .await
        .map_err(std::io::Error::other)??;
        if let Some(sha1) = published.get(&relative) {
            let digest = sha1::Sha1::digest(&bytes);
            if hex::encode(digest) != *sha1 {
                return Err(InstallerError::Damaged(relative));
            }
        }
        let target = steps.store.join(&relative);
        if tokio::fs::read(&target)
            .await
            .is_ok_and(|have| have == bytes)
        {
            continue;
        }
        download::write_whole(&target, &bytes).await?;
    }
    Ok(())
}

/// The installer's server arguments, one token each, with every `libraries/` path made absolute
/// into `libraries`; and those paths, relative to it. `sep` separates the entries of a module
/// path or classpath.
fn absolute_args(
    args: &str,
    libraries: &str,
    sep: &str,
) -> (Vec<String>, Vec<String>) {
    let mut referenced = Vec::new();
    let tokens = args
        .split_whitespace()
        .map(|token| {
            let (prefix, value) = match token.split_once('=') {
                Some((key, value)) if key.starts_with("-D") => {
                    (&token[..=key.len()], value)
                }
                _ => ("", token),
            };
            let value = value
                .split(sep)
                .map(|part| {
                    if part == "libraries" {
                        libraries.to_owned()
                    } else if let Some(path) = part.strip_prefix("libraries/") {
                        referenced.push(path.to_owned());
                        format!("{libraries}/{path}")
                    } else {
                        part.to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join(sep);
            format!("{prefix}{value}")
        })
        .collect();
    (tokens, referenced)
}

/// The arguments that run `installer` into `dir`.
pub fn installer_args(installer: &Path, dir: &Path) -> Vec<String> {
    vec![
        "-jar".into(),
        installer.to_string_lossy().into_owned(),
        "--installServer".into(),
        dir.to_string_lossy().into_owned(),
    ]
}

/// The arguments that start an installed Forge or NeoForge server in `dir`, after the memory
/// flags. A build installed from launcher-meta starts from the arguments recorded in the shared
/// store. Older Forge starts from the jar its installer wrote in the server folder.
pub fn launch_args(
    dirs: &SharedDirs,
    dir: &Path,
    loader: ModLoader,
    game_version: &str,
    version: &str,
) -> Result<Vec<String>, InstallerError> {
    let id = merged_version_id(game_version, loader, Some(version));
    if let Ok(args) = std::fs::read_to_string(dirs.server_args_path(&id)) {
        return Ok(args
            .lines()
            .map(str::to_owned)
            .chain(["--nogui".to_owned()])
            .collect());
    }
    let names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry.file_type().is_ok_and(|kind| kind.is_file())
                })
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    match old_forge_jar(&names, game_version, version) {
        Some(jar) if loader == ModLoader::Forge => {
            Ok(vec!["-jar".into(), jar, "--nogui".into()])
        }
        _ => Err(InstallerError::NoLaunch(loader)),
    }
}

/// The jar an installer before Forge 1.17 wrote, among the file names in the server folder. Its
/// name changed over the years; an exact name wins over a guess.
fn old_forge_jar(
    names: &[String],
    game_version: &str,
    version: &str,
) -> Option<String> {
    let exact = [
        // 1.12.2 to 1.16.5.
        format!("forge-{game_version}-{version}.jar"),
        // 1.8 to 1.11.2.
        format!("forge-{game_version}-{version}-universal.jar"),
        // 1.7.10.
        format!("forge-{game_version}-{version}-{game_version}-universal.jar"),
    ];
    if let Some(found) = exact.iter().find(|name| names.contains(name)) {
        return Some(found.clone());
    }
    names
        .iter()
        .filter(|name| {
            name.starts_with("forge-")
                && name.ends_with(".jar")
                && !name.ends_with("-installer.jar")
                && name.contains(game_version)
        })
        .min_by_key(|name| name.len())
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_that_is_not_plainly_one_never_reaches_a_url_or_path() {
        for good in [
            "47.2.0",
            "1.20.1-47.2.0",
            "21.1.77",
            "20.2.3-beta",
            "24w14a",
        ] {
            assert_eq!(checked(good).unwrap(), good);
        }
        for bad in [
            "47.2.0?x=/../../../../pwn",
            "..",
            "../47.2.0",
            "47.2.0/../x",
            r"47.2.0\x",
            ".hidden",
            "",
            "47 2",
        ] {
            assert!(checked(bad).is_err(), "{bad}");
        }
    }

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn old_forge_starts_from_the_jar_its_installer_wrote() {
        let installed = names(&[
            "forge-1.12.2-14.23.5.2860-installer.jar",
            "forge-1.12.2-14.23.5.2860.jar",
            "minecraft_server.1.12.2.jar",
        ]);
        assert_eq!(
            old_forge_jar(&installed, "1.12.2", "14.23.5.2860").as_deref(),
            Some("forge-1.12.2-14.23.5.2860.jar")
        );
        let universal = names(&[
            "forge-1.7.10-10.13.4.1614-1.7.10-universal.jar",
            "minecraft_server.1.7.10.jar",
        ]);
        assert_eq!(
            old_forge_jar(&universal, "1.7.10", "10.13.4.1614-1.7.10")
                .as_deref(),
            Some("forge-1.7.10-10.13.4.1614-1.7.10-universal.jar")
        );
        assert_eq!(
            old_forge_jar(&universal, "1.7.10", "10.13.4.1614").as_deref(),
            Some("forge-1.7.10-10.13.4.1614-1.7.10-universal.jar")
        );
        // The installer alone is not a server.
        let only_installer = names(&["forge-1.16.5-36.2.39-installer.jar"]);
        assert_eq!(old_forge_jar(&only_installer, "1.16.5", "36.2.39"), None);
    }

    #[test]
    fn the_installer_arguments_point_into_the_shared_store() {
        let args = "-p libraries/cpw/a.jar:libraries/org/b.jar\n--add-modules ALL-MODULE-PATH\n-DignoreList=a.jar,b.jar\n-Dfml.gameLayerLibraries=\n-DlibraryDirectory=libraries\n-DlegacyClassPath=libraries/net/c.jar:libraries/net/d.jar\ncpw.mods.bootstraplauncher.BootstrapLauncher\n--launchTarget forgeserver\n";
        let (tokens, referenced) =
            absolute_args(args, "/data/meta/libraries", ":");
        assert_eq!(
            tokens,
            [
                "-p",
                "/data/meta/libraries/cpw/a.jar:/data/meta/libraries/org/b.jar",
                "--add-modules",
                "ALL-MODULE-PATH",
                "-DignoreList=a.jar,b.jar",
                "-Dfml.gameLayerLibraries=",
                "-DlibraryDirectory=/data/meta/libraries",
                "-DlegacyClassPath=/data/meta/libraries/net/c.jar:/data/meta/libraries/net/d.jar",
                "cpw.mods.bootstraplauncher.BootstrapLauncher",
                "--launchTarget",
                "forgeserver",
            ]
        );
        assert_eq!(
            referenced,
            ["cpw/a.jar", "org/b.jar", "net/c.jar", "net/d.jar"]
        );
    }

    #[test]
    fn a_build_installed_from_launcher_meta_starts_from_the_store() {
        let data = tempfile::tempdir().unwrap();
        let dirs = SharedDirs::new(data.path());
        let path = dirs.server_args_path("1.21.1-neoforge-21.1.265");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "-p\n/m/libraries/a.jar\nMain").unwrap();
        let server = tempfile::tempdir().unwrap();
        assert_eq!(
            launch_args(
                &dirs,
                server.path(),
                ModLoader::NeoForge,
                "1.21.1",
                "21.1.265"
            )
            .unwrap(),
            ["-p", "/m/libraries/a.jar", "Main", "--nogui"]
        );
    }

    #[test]
    fn an_old_forge_folder_starts_from_its_jar() {
        let data = tempfile::tempdir().unwrap();
        let dirs = SharedDirs::new(data.path());
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "forge-1.12.2-14.23.5.2860.jar",
            "forge-1.12.2-14.23.5.2860-installer.jar",
        ] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        assert_eq!(
            launch_args(
                &dirs,
                dir.path(),
                ModLoader::Forge,
                "1.12.2",
                "14.23.5.2860"
            )
            .unwrap(),
            ["-jar", "forge-1.12.2-14.23.5.2860.jar", "--nogui"]
        );
        assert!(matches!(
            launch_args(
                &dirs,
                dir.path(),
                ModLoader::NeoForge,
                "1.21.1",
                "21.1.77"
            ),
            Err(InstallerError::NoLaunch(_))
        ));
    }

    #[tokio::test]
    async fn what_the_steps_made_is_checked_before_it_reaches_the_store() {
        let work = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        let made = work.path().join("libraries");
        let write = |path: PathBuf, text: &str| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        let patched = made.join("net/a/1/a-1.jar");
        write(patched.clone(), "patched");
        // A step may write over a library launcher-meta lists; the store keeps Core's copy.
        write(made.join("org/listed/1/listed-1.jar"), "planted");
        write(store.path().join("org/listed/1/listed-1.jar"), "downloaded");
        let mut steps = Steps {
            store: store.path().to_owned(),
            made: made.clone(),
            listed: HashSet::from(["org:listed:1".to_owned()]),
            data: HashMap::from([
                ("PATCHED".to_owned(), patched.to_string_lossy().into_owned()),
                (
                    "PATCHED_SHA".to_owned(),
                    hex::encode(sha1::Sha1::digest(b"patched")),
                ),
            ]),
        };
        store_outputs(&steps).await.unwrap();
        let read = |path: &str| {
            std::fs::read_to_string(store.path().join(path)).unwrap()
        };
        assert_eq!(read("net/a/1/a-1.jar"), "patched");
        assert_eq!(read("org/listed/1/listed-1.jar"), "downloaded");

        steps.data.insert("PATCHED_SHA".to_owned(), "0".repeat(40));
        assert!(matches!(
            store_outputs(&steps).await,
            Err(InstallerError::Damaged(path)) if path == "net/a/1/a-1.jar"
        ));
    }
}
