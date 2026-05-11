# src/domain — Core entities and types

Business entities with no infrastructure dependencies. Everything in here is pure data — derives `Serialize/Deserialize`, no async, no I/O.

## File structure

```
domain/
  mod.rs        — re-exports: instance, modpack, event, java
  instance.rs   — InstanceId, InstanceStatus, ModLoader, MemorySettings, InstanceRecord
  event.rs      — Event broadcast enum (InstanceOutput, StatusChanged, MacroOutput, CreationProgress)
  modpack.rs    — PackFormat, PackFile, PackFileEnv, EnvType, LinkedData, ModpackManifest
  java.rs       — JavaInstall (version: u32, path: PathBuf)
```

## instance.rs

`InstanceId` is a newtype over `Uuid` — it implements `Display`, `FromStr`, and `Hash`. Path params in Axum routes use `.parse::<InstanceId>()` to extract it; if the UUID is malformed the route returns 400 before reaching the handler.

`InstanceStatus` and `ModLoader` serialize to lowercase strings (`#[serde(rename_all = "lowercase")]`). Their `FromStr` impls are case-sensitive — `"Running"` fails, `"running"` succeeds. Both statuses and loaders are stored in SQLite as lowercase text.

`InstanceRecord.data_dir` is a `String`, not a `PathBuf`. Conversion to `PathBuf` happens at every usage site with `PathBuf::from(&record.data_dir)`. This is intentional — SQLx maps SQLite TEXT to String cleanly.

`MemorySettings` defaults to `min_mb: 512, max_mb: 4096`. The default is used whenever a `CreateInstanceRequest` omits memory settings.

`ModLoader` values and their server JAR sources:
- `Vanilla` / `Paper` / `Fabric` → resolved directly in `infrastructure::minecraft::flavours`
- `Forge` / `NeoForge` / `Quilt` → downloaded via installer JARs, handled in `infrastructure::minecraft::installer`

## event.rs

`Event` uses `#[serde(tag = "type", rename_all = "snake_case")]`. JSON output looks like:
```json
{ "type": "instance_output", "instance_id": "...", "line": "..." }
{ "type": "status_changed", "instance_id": "...", "status": "running" }
{ "type": "creation_progress", "instance_id": "...", "progress": 0.5, "message": "..." }
```

`CreationProgress.progress` is an `f32` in `0.0..=1.0`. The final event when JAR download completes sends `progress: 1.0`.

## modpack.rs

`PackFormat` maps directly to `modrinth.index.json` inside a `.mrpack` ZIP archive. Field names use `camelCase` (`#[serde(rename_all = "camelCase")]`).

`EnvType` uses `kebab-case` serialization — `"required"`, `"optional"`, `"unsupported"`. The `server` field on `PackFileEnv` is what the install pipeline checks to decide whether to download a file for the server.

`ModpackManifest` is the persisted DB record — it is NOT the same as `PackFormat`. `PackFormat` is the in-memory representation of the `.mrpack` index; `ModpackManifest` is what gets stored in the `modpack_manifests` table after installation. `ModpackManifest.installed_at` is a `String` (RFC 3339), not a `chrono::DateTime`.

## java.rs

Minimal — just a struct holding `version: u32` (major version, e.g. 17 or 21) and `path: PathBuf`. Detection happens in `infrastructure::minecraft::java`; this struct is the data contract between detection and storage.
