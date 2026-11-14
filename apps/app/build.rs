fn main() {
    set_env();

    tauri_build::try_build(
        tauri_build::Attributes::new()
            .codegen(tauri_build::CodegenContext::new()),
    )
    .expect("Failed to run tauri-build");
}

/// Account code reads Modrinth's URLs at compile time, from the `.env` the engine's build reads.
fn set_env() {
    const ENGINE_ENV: &str = "../../packages/engine/.env";
    println!("cargo::rerun-if-changed={ENGINE_ENV}");
    for (name, value) in dotenvy::from_path_iter(ENGINE_ENV)
        .into_iter()
        .flatten()
        .flatten()
    {
        if name.starts_with("MODRINTH_") {
            println!("cargo::rustc-env={name}={value}");
        }
    }
}
