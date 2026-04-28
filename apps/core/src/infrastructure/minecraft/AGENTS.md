# infrastructure/minecraft/

Everything needed to download, install, and configure Minecraft servers.

## Files

| File | Purpose |
|------|---------|
| `flavours.rs` | Per-loader download URL resolution → `JarInfo { url, filename }` |
| `installer.rs` | Run forge/quilt/neoforge GUI installers; detect `LaunchStyle` |
| `server_jar.rs` | Orchestrates the full install: download → maybe install → write `launch.json` |
| `modrinth_api.rs` | `ModrinthClient` — project/version lookup, file download |
| `mrpack.rs` | Parse and install `.mrpack` files (modrinth modpack format) |
| `server_properties.rs` | Read, write, and patch `server.properties` |
| `java.rs` | Detect installed Java versions on the host |
| `mod.rs` | Re-exports |

## Key types

### `JarInfo` (`flavours.rs`)
```rust
struct JarInfo { url: String, filename: String }
```
`get_jar_info(loader, game_version, loader_version, client) -> JarInfo`

Loaders handled: `vanilla` (Mojang API), `paper` (PaperMC API), `fabric` (Fabric Meta).  
`quilt`, `forge`, `neoforge` → `Err(Unsupported)` from `flavours`; `installer.rs` handles them.

### `LaunchConfig` + `LaunchStyle` (`installer.rs`)
```rust
enum LaunchStyle { Jar, Classpath }
struct LaunchConfig { launch_style: LaunchStyle, main_jar: String, classpath: Vec<String> }
```
Written to `data_dir/instances/<id>/launch.json` after install.

### `ModrinthClient` (`modrinth_api.rs`)
- `get_version(project_id, version_id)` → `ModrinthVersion`
- `ModrinthVersion.files[0].url` + `.hashes.sha512`
- `ModrinthProject` has `client_side`, `server_side`, `title`

## Rules

- All outbound HTTP goes through `AppState.http` (the shared `reqwest::Client`).
- Installer binaries (`forge-*.jar`) are run via `PtySpawner` (to capture stdout).
- `server.properties` must exist before `patch_properties` can be called (returns 404 otherwise).
- `mrpack` download follows `PackFile.env.server != "unsupported"` filter.
