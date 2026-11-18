//! How a Forge or NeoForge build's server starts, worked out from what its installer would leave
//! in a server folder, without running it.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{Read, Seek};

use anyhow::{Context, Result, bail, ensure};
use minecraft_meta_manifest::minecraft::{Library, VersionManifest};
use minecraft_meta_manifest::profile::{
    Processor, ProfileMetadata, SERVER_CLASSPATH_SEPARATOR, SERVER_JAR,
    SERVER_LIBRARY_DIRECTORY, ServerProfile,
};
use minecraft_meta_manifest::util::get_path_from_artifact;

use crate::remote::Remote;

const MODRINTH_MAVEN: &str = "https://launcher-meta.modrinth.com/maven/";

/// Which maven a build's installer is on.
#[derive(Debug, Clone, Copy)]
pub enum Repository {
    Forge,
    /// NeoForge, under the artifact it was published as: `forge` for 1.20.1, then `neoforge`.
    NeoForged(&'static str),
}

impl Repository {
    fn group_path(self) -> String {
        match self {
            Self::Forge => "net/minecraftforge/forge".to_owned(),
            Self::NeoForged(artifact) => format!("net/neoforged/{artifact}"),
        }
    }

    fn base(self) -> &'static str {
        match self {
            Self::Forge => "https://maven.minecraftforge.net/",
            Self::NeoForged(_) => "https://maven.neoforged.net/releases/",
        }
    }

    fn artifact(self) -> &'static str {
        match self {
            Self::Forge => "forge",
            Self::NeoForged(artifact) => artifact,
        }
    }
}

/// One loader build: its maven name and the launcher-meta profiles it builds on.
pub struct Build<'a> {
    pub repository: Repository,
    /// The full maven name, such as `1.7.10-10.13.4.1614-1.7.10`.
    pub name: &'a str,
    pub profile: &'a ProfileMetadata,
    pub vanilla: &'a VersionManifest,
}

pub fn server_profile(remote: &Remote, build: &Build) -> Result<ServerProfile> {
    let (repository, name) = (build.repository, build.name);
    let group = repository.group_path();
    let artifact = repository.artifact();
    let installer_url = format!(
        "{}{group}/{name}/{artifact}-{name}-installer.jar",
        repository.base()
    );
    let mut installer = remote
        .archive(&installer_url)?
        .with_context(|| format!("{installer_url} does not exist"))?;
    let unix = entry(&mut installer, "data/unix_args.txt")?;
    let shim = format!("{artifact}-{name}-shim.jar");
    let args = match unix {
        // From Forge 1.17, the installer ships the arguments the server starts with.
        Some(unix) => {
            let unix = String::from_utf8(unix)?;
            let windows = String::from_utf8(
                entry(&mut installer, "data/win_args.txt")?
                    .context("the installer has no Windows arguments")?,
            )?;
            ensure!(
                same_but_separator(&unix, &windows),
                "the Windows arguments differ by more than the separator"
            );
            let tokens: Vec<String> =
                unix.split_whitespace().map(str::to_owned).collect();
            match tokens.iter().position(|token| token == "-jar") {
                // Forge 1.20.6 on starts a shim jar that lists its libraries.
                Some(index) => {
                    let jar =
                        tokens.get(index + 1).context("-jar names no jar")?;
                    let mut args = tokens[..index].to_vec();
                    args.extend(shim_args(&mut installer, &group, name, jar)?);
                    args
                }
                None => tokens,
            }
        }
        // Early Forge 1.20.3 and 1.20.4 ship only the shim.
        None if installer
            .index_for_name(&format!("maven/{group}/{name}/{shim}"))
            .is_some() =>
        {
            shim_args(&mut installer, &group, name, &shim)?
        }
        // Before 1.17, the server starts from the Forge jar, which names its main class and
        // classpath in its manifest. launcher-meta mirrors that jar.
        None => {
            let path = get_path_from_artifact(&format!(
                "net.minecraftforge:forge:{name}"
            ))?;
            let url = format!("{MODRINTH_MAVEN}{path}");
            let mut jar = remote
                .archive(&url)?
                .with_context(|| format!("{url} does not exist"))?;
            let manifest = manifest(&mut jar)?;
            let main = manifest.get("Main-Class").context("no Main-Class")?;
            let classpath = std::iter::once(format!("libraries/{path}"))
                .chain(
                    manifest
                        .get("Class-Path")
                        .map(|paths| {
                            paths
                                .split_whitespace()
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_else(Vec::new),
                )
                .collect::<Vec<_>>()
                .join(":");
            vec!["-cp".to_owned(), classpath, main.clone()]
        }
    };
    let (arguments, referenced) = placeholders(&args);
    let libraries =
        missing_libraries(remote, build, &mut installer, &referenced)?;
    Ok(ServerProfile {
        arguments,
        libraries,
        processors: server_steps(build.profile)?,
    })
}

/// The profile's install steps a server runs, with the installer gone: the steps that read it are
/// dropped, and what they extract into the server folder's `libraries/` goes to the shared one.
pub fn server_steps(profile: &ProfileMetadata) -> Result<Vec<Processor>> {
    profile
        .processors
        .iter()
        .flatten()
        .filter(|step| {
            step.sides
                .as_ref()
                .is_none_or(|sides| sides.iter().any(|side| side == "server"))
        })
        .filter(|step| !step.args.iter().any(|arg| arg.contains("{INSTALLER}")))
        .map(|step| {
            let mut step = step.clone();
            for arg in &mut step.args {
                *arg = arg.replace("{ROOT}/libraries/", "{LIBRARY_DIR}/");
            }
            ensure!(
                !step.args.iter().any(|arg| arg.contains("{ROOT}")),
                "a server step of {} writes into the server folder",
                step.jar
            );
            Ok(step)
        })
        .collect()
}

/// Whether the Windows arguments are the Unix ones with `;` for `:`, token by token. Some lists
/// use `;` in both.
fn same_but_separator(unix: &str, windows: &str) -> bool {
    let unix: Vec<_> = unix.split_whitespace().collect();
    let windows: Vec<_> = windows.split_whitespace().collect();
    unix.len() == windows.len()
        && unix.iter().zip(&windows).all(|(unix, windows)| {
            unix.replace(':', ";") == windows.replace(':', ";")
        })
}

fn entry<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<Option<Vec<u8>>> {
    let mut file = match archive.by_name(name) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

/// The main section of a jar's manifest.
fn manifest<R: Read + Seek>(
    jar: &mut zip::ZipArchive<R>,
) -> Result<HashMap<String, String>> {
    let text = String::from_utf8(
        entry(jar, "META-INF/MANIFEST.MF")?
            .context("the jar has no manifest")?,
    )?;
    Ok(manifest_attributes(&text))
}

fn manifest_attributes(text: &str) -> HashMap<String, String> {
    let mut attributes = HashMap::new();
    let mut last: Option<String> = None;
    for line in text.lines() {
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.strip_prefix(' ') {
            if let Some(key) = &last {
                attributes
                    .entry(key.clone())
                    .and_modify(|value: &mut String| value.push_str(rest));
            }
        } else if let Some((key, value)) = line.split_once(": ") {
            attributes.insert(key.to_owned(), value.to_owned());
            last = Some(key.to_owned());
        }
    }
    attributes
}

/// What `java -jar` on a Forge shim jar runs: the shim's classpath and listed libraries, then the
/// main class and arguments it names.
fn shim_args<R: Read + Seek>(
    installer: &mut zip::ZipArchive<R>,
    group: &str,
    name: &str,
    jar: &str,
) -> Result<Vec<String>> {
    let bytes = entry(installer, &format!("maven/{group}/{name}/{jar}"))?
        .with_context(|| format!("the installer has no {jar}"))?;
    let shim = &mut zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let manifest = manifest(shim)?;
    let list = String::from_utf8(
        entry(shim, "bootstrap-shim.list")?
            .context("the shim lists no libraries")?,
    )?;
    let properties = String::from_utf8(
        entry(shim, "bootstrap-shim.properties")?
            .context("the shim has no properties")?,
    )?;
    let properties: HashMap<_, _> = properties
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect();
    let classpath = manifest
        .get("Class-Path")
        .into_iter()
        .flat_map(|paths| paths.split_whitespace().map(str::to_owned))
        .chain(list.lines().filter_map(|line| {
            line.split('\t')
                .nth(2)
                .map(|path| format!("libraries/{path}"))
        }))
        .collect::<Vec<_>>()
        .join(":");
    let mut args = vec![
        "-cp".to_owned(),
        classpath,
        (*properties
            .get("Main-Class")
            .context("the shim names no main class")?)
        .to_owned(),
    ];
    args.extend(
        properties
            .get("Arguments")
            .into_iter()
            .flat_map(|extra| extra.split_whitespace().map(str::to_owned)),
    );
    Ok(args)
}

/// The arguments with paths into a server folder's `libraries/` and its vanilla jar made
/// placeholders, and the library paths they reference.
fn placeholders(args: &[String]) -> (Vec<String>, BTreeSet<String>) {
    let mut referenced = BTreeSet::new();
    let arguments = args
        .iter()
        .map(|token| {
            let (prefix, value) = match token.split_once('=') {
                Some((key, value)) if key.starts_with("-D") => {
                    (&token[..=key.len()], value)
                }
                _ => ("", token.as_str()),
            };
            let mut changed = false;
            let parts: Vec<String> = value
                .split(':')
                .map(|part| {
                    if part == "libraries" {
                        changed = true;
                        SERVER_LIBRARY_DIRECTORY.to_owned()
                    } else if let Some(path) = part.strip_prefix("libraries/") {
                        changed = true;
                        referenced.insert(path.to_owned());
                        format!("{SERVER_LIBRARY_DIRECTORY}/{path}")
                    } else if part.starts_with("minecraft_server.")
                        && part.ends_with(".jar")
                    {
                        changed = true;
                        SERVER_JAR.to_owned()
                    } else {
                        part.to_owned()
                    }
                })
                .collect();
            if changed {
                format!("{prefix}{}", parts.join(SERVER_CLASSPATH_SEPARATOR))
            } else {
                token.clone()
            }
        })
        .collect();
    (arguments, referenced)
}

/// Libraries the arguments reference that neither the profiles nor the install steps provide,
/// each with the repository that hosts it. A reference the installer would also leave dangling
/// stays dangling: some vendor arguments name files no step makes.
fn missing_libraries<R: Read + Seek>(
    remote: &Remote,
    build: &Build,
    installer: &mut zip::ZipArchive<R>,
    referenced: &BTreeSet<String>,
) -> Result<Vec<Library>> {
    let mut known: HashSet<String> = build
        .profile
        .libraries
        .iter()
        .chain(&build.vanilla.libraries)
        .filter_map(|library| get_path_from_artifact(&library.name).ok())
        .collect();
    known.extend(outputs(
        build
            .profile
            .data
            .iter()
            .flatten()
            .map(|(_, entry)| &entry.server),
    ));
    let mut vendor = None;
    let mut libraries = Vec::new();
    for path in referenced.iter().filter(|path| !known.contains(*path)) {
        if let Some(host) = remote.library_host(path)? {
            libraries.push(Library {
                downloads: None,
                extract: None,
                name: coordinate(path)?,
                url: Some(host.to_owned()),
                natives: None,
                rules: None,
                checksums: None,
                include_in_classpath: false,
                downloadable: true,
            });
            continue;
        }
        let made = match &vendor {
            Some(made) => made,
            None => vendor.insert(vendor_outputs(installer)?),
        };
        if made.contains(path)
            || installer.index_for_name(&format!("maven/{path}")).is_some()
        {
            bail!("only the installer has {path}");
        }
    }
    Ok(libraries)
}

/// The repository paths of the `[coordinate]` values among install-step data.
fn outputs<'a>(
    values: impl Iterator<Item = &'a String>,
) -> impl Iterator<Item = String> {
    values.filter_map(|value| {
        let coordinate = value.strip_prefix('[')?.strip_suffix(']')?;
        get_path_from_artifact(coordinate).ok()
    })
}

/// What the installer's own install steps make on a server.
fn vendor_outputs<R: Read + Seek>(
    installer: &mut zip::ZipArchive<R>,
) -> Result<HashSet<String>> {
    let Some(bytes) = entry(installer, "install_profile.json")? else {
        return Ok(HashSet::new());
    };
    let profile: serde_json::Value = serde_json::from_slice(&bytes)?;
    let servers: Vec<String> = profile
        .get("data")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(_, entry)| {
            entry.get("server")?.as_str().map(str::to_owned)
        })
        .collect();
    Ok(outputs(servers.iter()).collect())
}

/// The maven coordinate of a repository-relative jar path.
fn coordinate(path: &str) -> Result<String> {
    let coordinate =
        minecraft_meta_manifest::util::MavenCoordinate::from_maven_path(
            &format!("maven/{path}"),
        )?;
    let mut name = format!(
        "{}:{}:{}",
        coordinate.group_id, coordinate.artifact_id, coordinate.version
    );
    if let Some(classifier) = coordinate.classifier {
        name.push_str(&format!(":{classifier}"));
    }
    match coordinate.extension {
        Some(extension) if extension.as_str() != "jar" => {
            name.push_str(&format!("@{extension}"));
        }
        _ => {}
    }
    if get_path_from_artifact(&name)? != path {
        bail!("{path} is not a maven path");
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn server_folder_paths_become_placeholders() {
        // Shapes from real installers: a module path, a -D classpath, a plain option, and the
        // classpath an old Forge jar's manifest names.
        let (arguments, referenced) = placeholders(&strings(&[
            "-p",
            "libraries/cpw/a.jar:libraries/org/b.jar",
            "--add-opens",
            "java.base/java.util.jar=cpw.mods.securejarhandler",
            "-DignoreList=a.jar,b.jar",
            "-DlibraryDirectory=libraries",
            "-DlegacyClassPath=libraries/net/c.jar",
            "-cp",
            "libraries/net/forge.jar:minecraft_server.1.7.10.jar",
            "cpw.mods.fml.relauncher.ServerLaunchWrapper",
        ]));
        assert_eq!(
            arguments,
            strings(&[
                "-p",
                "${library_directory}/cpw/a.jar${classpath_separator}${library_directory}/org/b.jar",
                "--add-opens",
                "java.base/java.util.jar=cpw.mods.securejarhandler",
                "-DignoreList=a.jar,b.jar",
                "-DlibraryDirectory=${library_directory}",
                "-DlegacyClassPath=${library_directory}/net/c.jar",
                "-cp",
                "${library_directory}/net/forge.jar${classpath_separator}${server_jar}",
                "cpw.mods.fml.relauncher.ServerLaunchWrapper",
            ])
        );
        assert_eq!(
            referenced.into_iter().collect::<Vec<_>>(),
            ["cpw/a.jar", "net/c.jar", "net/forge.jar", "org/b.jar"]
        );
    }

    #[test]
    fn windows_arguments_differ_only_by_the_separator() {
        for (unix, windows, same) in [
            ("-p a.jar:b.jar Main", "-p a.jar;b.jar Main", true),
            // Early NeoForge 1.20.1 writes this list with `;` in both files.
            ("-Dlist=a.jar;b.jar Main", "-Dlist=a.jar;b.jar Main", true),
            ("-p a.jar:b.jar Main", "-p a.jar;c.jar Main", false),
            ("-p a.jar Main", "-p a.jar Main --extra", false),
        ] {
            assert_eq!(same_but_separator(unix, windows), same, "{unix}");
        }
    }

    #[test]
    fn server_steps_need_no_installer_and_write_to_the_shared_store() {
        let profile = |steps: serde_json::Value| -> ProfileMetadata {
            serde_json::from_value(serde_json::json!({
                "id": "1.21.10-21.10.64",
                "inheritsFrom": "1.21.10",
                "releaseTime": "2025-10-07T00:00:00Z",
                "time": "2025-10-07T00:00:00Z",
                "type": "release",
                "libraries": [],
                "processors": steps
            }))
            .unwrap()
        };
        // Shapes from NeoForge 21.10's profile.
        let steps = server_steps(&profile(serde_json::json!([
            {"jar": "a:extract:1", "classpath": [], "sides": ["server"], "args": ["--archive", "{INSTALLER}", "--to", "{ROOT}/run.sh"]},
            {"jar": "a:split:1", "classpath": [], "sides": ["client"], "args": ["--input", "{MINECRAFT_JAR}"]},
            {"jar": "a:patch:1", "classpath": [], "args": ["--output", "{PATCHED}", "--extract-libraries-to", "{ROOT}/libraries/"]}
        ])))
        .unwrap();
        assert_eq!(
            steps
                .iter()
                .map(|step| (step.jar.as_str(), step.args.join(" ")))
                .collect::<Vec<_>>(),
            [(
                "a:patch:1",
                "--output {PATCHED} --extract-libraries-to {LIBRARY_DIR}/"
                    .to_owned()
            )]
        );
        assert!(
            server_steps(&profile(serde_json::json!([
                {"jar": "a:write:1", "classpath": [], "args": ["--to", "{ROOT}/server.properties"]}
            ])))
            .is_err()
        );
    }

    #[test]
    fn manifests_join_continued_lines() {
        let text = "Manifest-Version: 1.0\r\nClass-Path: libraries/a.jar libr\r\n aries/b.jar\r\nMain-Class: a.Main\r\n\r\nName: other\r\nMain-Class: b.Main\r\n";
        let attributes = manifest_attributes(text);
        assert_eq!(attributes["Class-Path"], "libraries/a.jar libraries/b.jar");
        assert_eq!(attributes["Main-Class"], "a.Main");
    }

    #[test]
    fn library_paths_map_back_to_coordinates() {
        for (path, name) in [
            ("org/ow2/asm/asm/9.1/asm-9.1.jar", "org.ow2.asm:asm:9.1"),
            (
                "net/minecraftforge/forge/1.20.6-50.2.15/forge-1.20.6-50.2.15-server.jar",
                "net.minecraftforge:forge:1.20.6-50.2.15:server",
            ),
        ] {
            assert_eq!(coordinate(path).unwrap(), name);
        }
    }
}
