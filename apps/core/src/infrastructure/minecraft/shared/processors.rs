//! Forge / NeoForge post-download processors, server side.
//!
//! Port of the processor loop in app-lib's `launcher::install_minecraft`,
//! switched from the client to the server side: processors are filtered to those
//! that run on the server, the `SIDE` datum is `server`, `MINECRAFT_JAR` points
//! at the shared vanilla server jar, and every `{KEY}` placeholder resolves from
//! the `server` value of each `SidedDataEntry` (app-lib uses `client`).
//!
//! Processors binpatch/remap the vanilla server jar and emit the loader's
//! launch jars/args into the shared `libraries/` tree, so their effects are
//! shared across every instance of the build just like the libraries are.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;

use daedalus::minecraft::VersionInfo;
use daedalus::modded::SidedDataEntry;

use super::libraries::{classpath_from_artifacts, lib_path, LibraryError};

#[derive(Debug, thiserror::Error)]
pub enum ProcessorError {
	#[error("io: {0}")]
	Io(#[from] std::io::Error),
	#[error("library: {0}")]
	Library(#[from] LibraryError),
	#[error("processor {jar} has no Main-Class")]
	NoMainClass { jar: String },
	#[error("processor {jar} failed: {stderr}")]
	Failed { jar: String, stderr: String },
}

/// Run all server-side processors for a Forge/NeoForge build, if any.
///
/// `root` is the shared store root used for the `ROOT` datum, `libraries_dir`
/// the shared maven tree, and `server_jar` the freshly downloaded vanilla
/// server jar that processors transform in place.
pub async fn run_processors(
	java: &Path,
	version_info: &VersionInfo,
	root: &Path,
	libraries_dir: &Path,
	server_jar: &Path,
	game_version: &str,
	java_arch: &str,
) -> Result<(), ProcessorError> {
	let Some(processors) = &version_info.processors else {
		return Ok(());
	};
	if processors.is_empty() {
		return Ok(());
	}

	let mut data: HashMap<String, SidedDataEntry> = HashMap::new();
	if let Some(existing) = &version_info.data {
		for (key, entry) in existing {
			data.insert(
				key.clone(),
				SidedDataEntry {
					client: entry.client.clone(),
					server: entry.server.clone(),
				},
			);
		}
	}
	insert_datum(&mut data, "SIDE", "server");
	insert_datum(&mut data, "MINECRAFT_JAR", &server_jar.to_string_lossy());
	insert_datum(&mut data, "MINECRAFT_VERSION", game_version);
	insert_datum(&mut data, "ROOT", &root.to_string_lossy());
	insert_datum(&mut data, "LIBRARY_DIR", &libraries_dir.to_string_lossy());

	for processor in processors {
		// Run when the processor targets the server, or declares no sides.
		if let Some(sides) = &processor.sides {
			if !sides.iter().any(|s| s == "server") {
				continue;
			}
		}

		let mut cp = processor.classpath.clone();
		cp.push(processor.jar.clone());
		let classpath = classpath_from_artifacts(libraries_dir, &cp, java_arch)?;

		let jar_path = lib_path(libraries_dir, &processor.jar, false)?;
		let main_class = read_main_class(&jar_path)?.ok_or_else(|| {
			ProcessorError::NoMainClass {
				jar: processor.jar.clone(),
			}
		})?;

		let args = resolve_args(libraries_dir, &processor.args, &data)?;

		let output = tokio::process::Command::new(java)
			.arg("-cp")
			.arg(&classpath)
			.arg(&main_class)
			.args(&args)
			.output()
			.await?;

		if !output.status.success() {
			return Err(ProcessorError::Failed {
				jar: processor.jar.clone(),
				stderr: String::from_utf8_lossy(&output.stderr).to_string(),
			});
		}
		tracing::info!("ran server processor {}", processor.jar);
	}

	Ok(())
}

fn insert_datum(
	data: &mut HashMap<String, SidedDataEntry>,
	key: &str,
	value: &str,
) {
	data.insert(
		key.to_string(),
		SidedDataEntry {
			client: value.to_string(),
			server: value.to_string(),
		},
	);
}

/// Resolve a processor's argument list: `[maven:coord]` entries become shared
/// library paths, and `{KEY}` placeholders are replaced with the `server` value
/// of the matching datum (itself possibly a `[lib]` reference). Port of
/// app-lib's `get_processor_arguments`, using `server` instead of `client`.
fn resolve_args(
	libraries_dir: &Path,
	args: &[String],
	data: &HashMap<String, SidedDataEntry>,
) -> Result<Vec<String>, ProcessorError> {
	let mut out = Vec::with_capacity(args.len());
	for arg in args {
		if let Some(inner) = arg.strip_prefix('[') {
			if let Some(lib_key) = inner.strip_suffix(']') {
				out.push(lib_path(libraries_dir, lib_key, true)?);
				continue;
			}
		}

		let mut arg = arg.clone();
		for (key, entry) in data {
			let replacement = if let Some(inner) = entry.server.strip_prefix('[')
			{
				if let Some(lib_key) = inner.strip_suffix(']') {
					lib_path(libraries_dir, lib_key, true)?
				} else {
					entry.server.clone()
				}
			} else {
				entry.server.clone()
			};
			arg = arg.replace(&format!("{{{key}}}"), &replacement);
		}
		out.push(arg);
	}
	Ok(out)
}

/// Read the `Main-Class` from a jar's `META-INF/MANIFEST.MF`. Port of
/// app-lib's `get_processor_main_class`.
fn read_main_class(path: &str) -> Result<Option<String>, ProcessorError> {
	let file = std::fs::File::open(path)?;
	let mut archive = zip::ZipArchive::new(file)
		.map_err(|e| ProcessorError::Io(std::io::Error::other(e)))?;
	let manifest = archive
		.by_name("META-INF/MANIFEST.MF")
		.map_err(|e| ProcessorError::Io(std::io::Error::other(e)))?;
	for line in BufReader::new(manifest).lines() {
		let mut line = line?;
		line.retain(|c| !c.is_whitespace());
		if line.starts_with("Main-Class:") {
			if let Some(class) = line.split(':').nth(1) {
				return Ok(Some(class.to_string()));
			}
		}
	}
	Ok(None)
}
