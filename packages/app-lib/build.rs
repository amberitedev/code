use std::ffi::OsString;
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::path::{Component, Path, Prefix};
use std::process::{Command, exit};
use std::{env, fs};

fn main() {
    println!("cargo::rerun-if-changed=.env");
    println!("cargo::rerun-if-changed=java/gradle");
    println!("cargo::rerun-if-changed=java/src");
    println!("cargo::rerun-if-changed=java/build.gradle.kts");
    println!("cargo::rerun-if-changed=java/settings.gradle.kts");
    println!("cargo::rerun-if-changed=java/gradle.properties");

    set_env();
    build_java_jars();
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
