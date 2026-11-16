use std::env;
use std::ffi::OsString;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
#[cfg(target_os = "windows")]
use std::path::{Component, Prefix};
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

fn main() {
    println!("cargo::rerun-if-changed=.env");
    println!("cargo::rerun-if-changed=java/gradle");
    println!("cargo::rerun-if-changed=java/src");
    println!("cargo::rerun-if-changed=java/build.gradle.kts");
    println!("cargo::rerun-if-changed=java/settings.gradle.kts");
    println!("cargo::rerun-if-changed=java/gradle.properties");

    set_env();
    build_java_jars();
    embed_common_controls_manifest();
}

/// Tauri imports `TaskDialogIndirect`, which only Common Controls 6 exports, so the engine's own
/// test binaries fail to load (STATUS_ENTRYPOINT_NOT_FOUND) without the manifest tauri-build gives
/// the App.
fn embed_common_controls_manifest() {
    if env::var_os("CARGO_FEATURE_TAURI").is_none()
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }
    println!("cargo::rustc-link-arg=/MANIFEST:EMBED");
    println!(
        "cargo::rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );
}

fn set_env() {
    for (var_name, var_value) in
        dotenvy::dotenv_iter().into_iter().flatten().flatten()
    {
        if var_name == "DATABASE_URL" {
            // The sqlx database URL is a build-time detail that should not be exposed to the crate
            continue;
        }

        println!("cargo::rustc-env={var_name}={var_value}");
    }
}

fn build_java_jars() {
    let out_dir =
        dunce::canonicalize(PathBuf::from(env::var_os("OUT_DIR").unwrap()))
            .unwrap();

    println!(
        "cargo::rustc-env=JAVA_JARS_DIR={}",
        out_dir.join("java/libs").display()
    );

    // Cargo reruns this script whenever the java files look newer, which the dev runner's shared
    // build causes on every engine edit. Gradle takes 10-30 s even with nothing to do.
    let stamp = out_dir.join("java/inputs.stamp");
    let inputs = java_inputs_hash();
    if out_dir.join("java/libs/theseus.jar").is_file()
        && fs::read_to_string(&stamp).is_ok_and(|built| built == inputs)
    {
        return;
    }
    // A failed run can still rewrite the JAR, so only a finished one leaves a stamp.
    if let Err(error) = fs::remove_file(&stamp)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        panic!("Could not remove {}: {error}", stamp.display());
    }

    #[cfg(target_os = "windows")]
    let java_dir = windows_java_dir(&out_dir);
    #[cfg(not(target_os = "windows"))]
    let java_dir = dunce::canonicalize("java").unwrap();

    let gradle_path = java_dir.join(if cfg!(target_os = "windows") {
        "gradlew.bat"
    } else {
        "gradlew"
    });

    let mut build_dir_str = OsString::from("-Dorg.gradle.project.buildDir=");
    build_dir_str.push(out_dir.join("java"));
    let exit_status = Command::new(gradle_path)
        .arg(build_dir_str)
        .arg("build")
        .arg("--no-daemon")
        .arg("--console=rich")
        .current_dir(java_dir)
        .status()
        .expect("Failed to wait on Gradle build");

    if !exit_status.success() {
        println!("cargo::error=Gradle build failed with {exit_status}");
        exit(exit_status.code().unwrap_or(1));
    }
    // An input edited while Gradle ran may not be in the JAR.
    if java_inputs_hash() == inputs {
        fs::write(stamp, inputs).unwrap();
    }
}

/// The content of everything Gradle reads, as the rerun-if-changed lines above name it.
fn java_inputs_hash() -> String {
    fn visit(path: &Path, hasher: &mut DefaultHasher) {
        if path.is_dir() {
            let mut entries = fs::read_dir(path)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect::<Vec<_>>();
            entries.sort();
            for entry in entries {
                visit(&entry, hasher);
            }
        } else if let Ok(content) = fs::read(path) {
            path.hash(hasher);
            content.hash(hasher);
        }
    }
    let mut hasher = DefaultHasher::new();
    for input in [
        "gradle",
        "src",
        "build.gradle.kts",
        "settings.gradle.kts",
        "gradle.properties",
    ] {
        visit(&Path::new("java").join(input), &mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

#[cfg(target_os = "windows")]
fn windows_java_dir(out_dir: &Path) -> PathBuf {
    let source = env::current_dir().unwrap().join("java");
    let canonical_source = fs::canonicalize(&source).unwrap();
    let is_unc = matches!(
        canonical_source.components().next(),
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..))
    );
    if !is_unc {
        return source;
    }

    let destination = out_dir.join("java-source");
    let _ = fs::remove_dir_all(&destination);
    copy_dir(&source, &destination);
    fs::write(out_dir.join(".gitattributes"), "* text=auto eol=lf\n").unwrap();
    destination
}

#[cfg(target_os = "windows")]
fn copy_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}
