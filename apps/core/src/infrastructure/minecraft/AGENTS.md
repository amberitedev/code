# src/infrastructure/minecraft — Minecraft-specific infrastructure

Downloads, installs, and validates Minecraft server JARs; detects Java; queries the Modrinth API; parses/installs `.mrpack` archives; manages `server.properties`.

## File structure

```
minecraft/
  mod.rs              — re-exports all submodules
  flavours.rs         — resolve_jar(): URL-based loaders (Vanilla, Paper, Fabric)
  installer.rs        — install_with_installer(): installer-based loaders (Quilt, Forge, NeoForge)
  java.rs             — detect_java_installations(), required_java_version()
  modrinth_api.rs     — ModrinthClient: project/version lookup, hash-based version lookup
  mrpack.rs           — extract_metadata(), install_mrpack(): parse and install .mrpack archives
  server_jar.rs       — download_server_jar(): orchestrates flavours/installer dispatch + SHA1 verify
  server_properties.rs — read/write/patch server.properties
```

## flavours.rs

Handles the three URL-based loaders. **Quilt, Forge, and NeoForge are explicitly rejected here** — they call `installer.rs`.

| Loader | Endpoint | Notes |
|--------|----------|-------|
| Vanilla | `launchermeta.mojang.com/mc/game/version_manifest.json` + per-version URL | SHA1 included in manifest |
| Paper | `api.papermc.io/v2/projects/paper/versions/{ver}/builds` | Picks the last `channel = "default"` build only — no SHA1 |
| Fabric | `meta.fabricmc.net/v2/versions/loader/{mc}/{lv}/1.0.1/server/jar` | Uses hardcoded Fabric Installer API version `1.0.1`; no SHA1 |

## installer.rs

Handles Quilt, Forge, and NeoForge by downloading their installer JARs and running them with `java -jar installer.jar --installServer {data_dir}`.

Installer JARs are downloaded to `tempfile::NamedTempFile` — cleaned up automatically on drop.

`detect_launch_style`: after a Forge/NeoForge install, reads `run.sh` or `run.bat` to check for `@libraries` args-file references (Forge 1.17+ pattern). Falls back to `LaunchStyle::Jar { jar: "forge-server.jar" }` for older Forge.

**NeoForge**: requires MC 1.20.1+. Returns `InstallerError::UnsupportedVersion` for older versions.

**`launch.json`**: after every installer-based install, `write_launch_config` writes `{data_dir}/launch.json`. This JSON file tells `instance_status_service::start_instance` how to launch the server. Read by `read_launch_config` at startup.

## java.rs

`detect_java_installations()`: uses `which::which` to probe for these binary names in order: `java21`, `java17`, `java`. If none of the versioned names are found, falls back to `java` with version `8`. This means:
- Java installations not on `PATH` are not detected.
- Only one installation per version slot is stored (the first `which` finds).
- Java 11 is never detected — only 8, 17, 21.

`required_java_version(game_version)`: maps MC version → required Java major version.

| MC versions | Java required |
|-------------|---------------|
| 1.20.x and above | 21 |
| 1.17.x – 1.19.x | 17 |
| older | 8 |

**Known inaccuracy**: 1.20.0–1.20.4 only requires Java 17; the code requires 21 for all 1.20.x. This is intentionally conservative — it will not break any server, but users running 1.20.0–1.20.4 need Java 21 installed.

## modrinth_api.rs

`ModrinthClient` wraps a `reqwest::Client` with a user-agent header. All calls hit `api.modrinth.com/v2`.

Key methods:
- `get_project(id_or_slug)` → `ModrinthProject`
- `get_version(version_id)` → `ModrinthVersion`
- `list_versions(project_id, game_version?, loader?)` → `Vec<ModrinthVersion>`
- `get_version_by_hash(sha512_hex)` → `ModrinthVersion` — used by `mod_service::update_mod` to find the latest version for an installed JAR

`ModrinthVersion.files` contains multiple `ModrinthFile` entries; the primary file is the one with `primary: true`.

## mrpack.rs

Uses `async_zip` for async ZIP reading (tokio-compatible). The `ZipFileReader` must be opened fresh for each pass — `extract_metadata` and `extract_overrides` each open the file independently.

`install_mrpack`:
1. Calls `extract_metadata` to get `PackFormat` (the parsed `modrinth.index.json`).
2. Iterates `PackFormat.files` — skips files where `env.server == EnvType::Unsupported` (client-only mods).
3. Downloads each server-side file to its `path` (relative to `instance_dir`), with SHA1 verification.
4. Extracts `overrides/` entries to `instance_dir/`.
5. Extracts `server-overrides/` entries to `instance_dir/`.

**SHA1 verification**: only verified if `file.hashes.sha1` is `Some`. Files with no SHA1 hash are downloaded without verification.

**`is_client_only`**: checks `env.server == EnvType::Unsupported`. Mods with `env: None` (no env field) are downloaded. Mods with `server: Optional` or `server: Required` are downloaded.

## server_properties.rs

Reads and writes `{data_dir}/server.properties` as key-value pairs. `patch_properties` reads the file, updates the specified keys in-place (preserving comments and ordering), and writes it back atomically.

`write_initial_properties`: writes a minimal `server.properties` (port, online-mode=false) and `eula.txt` (`eula=true`). Called by `create_instance` before the server JAR is downloaded. Note: Minecraft reads EULA from `eula.txt`, not `server.properties`.

## Gotchas

- **Fabric uses hardcoded installer API version `1.0.1`**: if the Fabric installer API bumps its version scheme, this URL will break silently (the download will 404 or return the wrong format).
- **Paper: no SHA1**: the PaperMC API response does not include a SHA1 hash for the JAR. Downloads are not verified.
- **Forge installer blocks for 1–5 minutes**: `run_installer` awaits a `tokio::process::Command` for the duration. This blocks the JAR download task.
