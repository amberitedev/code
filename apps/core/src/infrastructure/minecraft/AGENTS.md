# infrastructure/minecraft/

Everything needed to download, install, and configure Minecraft servers.

## Files

| File | Purpose |
|------|---------|
| `flavours.rs` | Per-loader download URL resolution → `JarInfo { url, filename }` |
| `installer.rs` | Quilt/Forge/NeoForge GUI installer runner; `LaunchStyle` detection |
| `server_jar.rs` | Orchestrates: download or install → write `launch.json` |
| `modrinth_api.rs` | `ModrinthClient` — project/version lookup, file download |
| `mrpack.rs` | Parse and install `.mrpack` files (modrinth modpack format) |
| `server_properties.rs` | Read, write, and patch `server.properties` |
| `java.rs` | Detect installed Java versions on the host; `java_installations` DB sync |
| `mod.rs` | Re-exports |

## Loader dispatch matrix

| Loader | Handled by | Launch style |
|--------|-----------|--------------|
| `vanilla` | `flavours.rs` (Mojang API) | Plain `-jar` |
| `paper` | `flavours.rs` (PaperMC API) | Plain `-jar` |
| `fabric` | `flavours.rs` (Fabric Meta) | Plain `-jar` |
| `quilt` | `installer.rs` | `quilt-server-launch.jar` |
| `forge` | `installer.rs` | ArgsFile (`@libraries/…`) or plain jar |
| `neoforge` | `installer.rs` (requires 1.20.1+) | ArgsFile |

## `LaunchConfig` + `LaunchStyle` (`installer.rs`)

```rust
enum LaunchStyle {
    Jar     { jar: String },               // -jar <jar>
    ArgsFile { args: String },             // @<args-file>
}
struct LaunchConfig { style: LaunchStyle }
```

Written to `data_dir/instances/<id>/launch.json` after installation.  
Read by `instance_status_service` when starting an instance.

## `ModrinthClient` (not a struct — free functions in `modrinth_api.rs`)

- `get_project(client, id)` → project metadata (`client_side`, `server_side`, `title`)
- `get_version(client, version_id)` → version with files list
- `ModrinthVersion.files[0]` → `{ url, filename, hashes: { sha1, sha512 } }`

## `server_properties.rs`

- `write_initial_properties(data_dir, port)` — creates `server.properties` with `server-port` set
- `read_properties(data_dir)` → `HashMap<String, String>`
- `patch_properties(data_dir, patches)` — apply key/value updates; returns 404 if file missing

## `java.rs`

`detect_java_installations()` scans `JAVA_HOME`, `PATH`, and common OS paths.  
`sync_java_to_db(pool, installs)` upserts into `java_installations` table.  
`required_java_version(game_version)` returns required Java major (8, 11, 17, 21).  
`find_java(pool, major)` returns the path of the best matching Java install from DB.

## Rules

- All outbound HTTP goes through the shared `reqwest::Client` from `AppState.http`.
- Installer binaries are run via `tokio::process::Command` (not `PtySpawner`).
- `.mrpack` install filters files where `env.server != "unsupported"`.
- NeoForge requires MC 1.20.1+ — returns `InstallerError::UnsupportedVersion` otherwise.
- `server.properties` must exist before `patch_properties` is called.
