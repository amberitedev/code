# Features to Implement

Complete inventory of every feature in this plan with important implementation context.
Use this as the primary checklist. See the numbered plan files for grouped detail.

---

## BLOCKERS — Fix Before Anything Else

### B1 — Fix Serde Version Pin
**What it does:** Allows the project to compile.
**Why broken:** `serde = "=1.0.219"` conflicts with `serde_json` requiring `^1.0.220`.
**Fix:** Change to `serde = "^1.0.220"` in `apps/core/Cargo.toml`.
**Touches:** `Cargo.toml`

### B2 — Fix Migration 002 Silent Skip
**What it does:** Makes all instance columns actually exist in the database.
**Why broken:** `CREATE TABLE IF NOT EXISTS instances` in migration `002` is silently
skipped because migration `001` already created that table. All extended columns
(game_version, loader, port, memory_min, memory_max, status, data_dir) are never added.
**Fix:** New `migrations/003_alter_instances.sql`. Use `ALTER TABLE instances ADD COLUMN`
for each missing column with `IF NOT EXISTS` guards. Do not modify 001 or 002.
**Touches:** new `migrations/003_alter_instances.sql`

### B3 — Wire Repos Into AppState
**What it does:** Makes the existing repo layer actually get used.
**Why needed:** `InstanceRepo` and `ModpackRepo` are fully implemented but `AppState`
holds only `pool: SqlitePool`, so services write raw inline SQL and ignore the repos.
**Fix:**
1. Add `instance_store: Arc<dyn InstanceStore + Send + Sync>` to `AppState`
2. Add `modpack_store: Arc<dyn ModpackStore + Send + Sync>` to `AppState`
3. In `main.rs` startup: `InstanceRepo::new(pool.clone())` and `ModpackRepo::new(pool.clone())`
4. Replace all raw SQL in `instance_service.rs` and `modpack_service.rs` with store calls
**Touches:** `application/state.rs`, `main.rs`, `application/instance_service.rs`,
`application/modpack_service.rs`

### B4 — Write server.properties on Instance Creation
**What it does:** Ensures every new server starts successfully on first launch.
**Why broken:** `create_instance()` never calls `write_initial_properties()`. New servers
have no `server.properties` so Minecraft refuses to run (EULA not accepted).
**Fix:** Call `write_initial_properties(data_dir, port)` in `create_instance` after
creating the instance directory. Writes `eula=true`, `server-port={port}`,
`online-mode=false`.
**Touches:** `application/instance_service.rs`, `infrastructure/minecraft/server_properties.rs`

### B5 — Fix list_macros Name Collision
**What it does:** Allows `macros.rs` to compile.
**Why broken:** Handler `fn list_macros` shadows the imported `macro_service::list_macros`.
Rust resolves the handler instead of the service function.
**Fix:** Rename handler to `list_macros_handler`.
**Touches:** `presentation/handlers/macros.rs`

### B6 — Split instance_service.rs Over 200 Lines
**What it does:** Brings the file back under the 200-line hard cap.
**Why needed:** The file is 223 lines, violating the project rule.
**Fix:** After B3 (repo wiring), split into:
- `instance_service.rs` — create, delete, get, list (instance lifecycle)
- `instance_status_service.rs` — start, stop, kill, restart, status polling
**Touches:** `application/instance_service.rs` → split into two files

---

## LOADERS — All 6 Server Flavours

### L1 — Quilt Server JAR
**What it does:** Lets users create instances with the Quilt loader (Fabric fork, better
mod compatibility guarantees).
**How:** Download Quilt installer from `maven.quiltmc.org`. Run
`java -jar quilt-installer.jar install server {mc_version} --install-dir {data_dir}`.
**Important:** The resulting launch JAR is `quilt-server-launch.jar`, not `server.jar`.
Update the startup command accordingly.
**Touches:** `infrastructure/minecraft/flavours.rs`

### L2 — Paper Server JAR
**What it does:** Lets users create high-performance Bukkit/Spigot-compatible servers.
Paper is the most popular server software for plugin-based servers.
**How:** Query `https://api.papermc.io/v2/projects/paper/versions/{mc_version}/builds`,
pick the latest stable build, download the JAR from the builds endpoint. No installer step.
**Important:** The downloaded JAR IS the server — just run it directly.
**Touches:** `infrastructure/minecraft/flavours.rs`

### L3 — Forge Server JAR
**What it does:** Lets users create modded servers for 1.1–1.20.x using Forge.
**How:** Fetch `promotions_slim.json` from Minecraftforge to get the recommended Forge
version for the requested MC version. Download installer JAR. Run
`java -jar forge-installer.jar --installServer {data_dir}`.
**Important:** Forge 1.17+ uses an `@args.txt` launch style, not `-jar forge.jar`.
Forge <1.17 uses plain `-jar`. The installer generates the correct launch script —
read it to determine the startup command.
**Important:** Run the installer with `tokio::process::Command::spawn()` and stream output
to the instance log. Never use `.output()` — it looks frozen to the user.
**Touches:** `infrastructure/minecraft/flavours.rs`

### L4 — NeoForge Server JAR
**What it does:** Lets users create modded servers on Minecraft 1.20.1+. NeoForge is the
modern successor to Forge with active development.
**How:** Parse the latest NeoForge version from NeoForged Maven metadata XML. Download
installer JAR. Run `java -jar neoforge-installer.jar --installServer {data_dir}`.
Same `@args.txt` launch pattern as modern Forge.
**Important:** NeoForge only supports Minecraft 1.20.1+. Reject older versions with a
clear error: "NeoForge requires Minecraft 1.20.1 or newer."
**Touches:** `infrastructure/minecraft/flavours.rs`

---

## MOD MANAGEMENT — Entirely New Layer

### M0 — Mods Table Migration
**What it does:** Creates the database table that tracks every installed mod per instance.
**Schema columns:** id (UUID PK), instance_id (FK → instances, CASCADE DELETE),
filename, display_name, modrinth_project_id (nullable), modrinth_version_id (nullable),
version_number, client_side, server_side, sha512, enabled (0/1), installed_at.
**Unique constraint:** (instance_id, filename) — one mod entry per file per instance.
**Touches:** new `migrations/004_mods.sql`

### M1 — GET /instances/:id/mods
**What it does:** Returns the full mod list for an instance including metadata and status.
**How:** Query the `mods` table for the instance. Also scan the actual `mods/` directory
to catch any `.jar` or `.jar.disabled` files not yet in the DB. Untracked files appear in
the response with `"tracked": false`.
**Response per mod:** filename, display_name, version_number, enabled (bool),
client_side, server_side, modrinth_project_id, update_available (bool or null if
no Modrinth ID).
**Touches:** new `presentation/handlers/mods.rs`, new `application/mod_service.rs`

### M2 — POST /instances/:id/mods (Add from Modrinth)
**What it does:** Downloads and installs a mod from Modrinth by version ID.
**Request:** `{ "version_id": "P7dR8mSH" }`
**How:**
1. Call Modrinth `GET /version/{version_id}` for file info
2. Call Modrinth `GET /project/{project_id}` for `client_side` / `server_side`
3. If `server_side = "unsupported"`, return HTTP 400 with message "This mod is
   client-only and cannot run on a server."
4. Find the primary file in `files` array (look for `"primary": true`; fallback to first `.jar`)
5. Download to `{data_dir}/mods/{filename}` using existing ModrinthClient
6. Insert `mods` row with all metadata + sha512 hash
**Important:** The version endpoint alone doesn't include `client_side`/`server_side`.
You need to also fetch the project. Two API calls per install.
**Touches:** `presentation/handlers/mods.rs`, `application/mod_service.rs`,
`infrastructure/minecraft/modrinth_api.rs` (add project fetch if not present)

### M3 — POST /instances/:id/mods/upload (Add Private Mod)
**What it does:** Installs a mod from a local file upload for private or unlisted mods.
**How:** Accept multipart upload. Write `.jar` to `{data_dir}/mods/{filename}`. Insert
`mods` row with `modrinth_project_id = null`, `modrinth_version_id = null`.
**Important:** Do NOT attempt to look up the mod on Modrinth — it's private. Don't
call any Modrinth API. No `server_side` check (user is responsible for private mods).
**Touches:** `presentation/handlers/mods.rs`, `application/mod_service.rs`

### M4 — DELETE /instances/:id/mods/:filename
**What it does:** Removes a mod from the instance.
**How:** Delete the file from disk — check both `mod.jar` and `mod.jar.disabled`.
Remove the row from `mods` table.
**Important:** If the server is currently running, return 200 but include a
`"warning": "Server must be restarted for this change to take effect"` in the response.
**Touches:** `presentation/handlers/mods.rs`, `application/mod_service.rs`

### M5 — PATCH /instances/:id/mods/:filename (Enable/Disable)
**What it does:** Toggles a mod on or off without deleting it.
**Request:** `{ "enabled": false }`
**How:** Rename file on disk: `mod.jar` → `mod.jar.disabled` (disable) or reverse
(enable). Update `enabled` column in DB.
**Important:** Check that the file exists with one of the two extensions before renaming.
If neither exists, return 404.
**Touches:** `presentation/handlers/mods.rs`, `application/mod_service.rs`

### M6 — PUT /instances/:id/mods/:filename/update (Update One Mod)
**What it does:** Updates a specific mod to the latest version compatible with the
instance's Minecraft version and loader.
**How:**
1. Lookup `modrinth_project_id` in DB. If null, return HTTP 400 (can't auto-update private)
2. Query Modrinth for the latest version matching instance `game_version` + `loader`
3. If returned `version_id` = stored `modrinth_version_id`, return `{ "already_latest": true }`
4. Download new JAR, delete old file, update `mods` row
**Touches:** `presentation/handlers/mods.rs`, `application/mod_service.rs`

### M7 — POST /instances/:id/mods/update-all
**What it does:** Runs the update check and update flow for every Modrinth-tracked mod
in the instance at once.
**Response:** `{ "updated": [...], "already_latest": [...], "failed": [...] }`
**Important:** Process mods sequentially — not in parallel. Modrinth has API rate limits.
**Touches:** `presentation/handlers/mods.rs`, `application/mod_service.rs`

---

## LOGS — Historical Log Files

### LOG1 — GET /instances/:id/logs
**What it does:** Lists all historical log files from past server sessions.
**How:** Scan `{data_dir}/logs/`. Return array of `{ filename, size_bytes, modified_at }`.
Sort newest first (by modified_at).
**Touches:** new `presentation/handlers/logs.rs`, new `application/log_service.rs`

### LOG2 — GET /instances/:id/logs/:filename
**What it does:** Returns the content of a specific log file.
**How:** Read `{data_dir}/logs/{filename}`. If filename ends in `.gz`, decompress with
`flate2` crate before returning. Stream the response body — do not load into memory.
**Important:** Validate filename — reject any path with `..`, `/`, or `\`. Accept only
plain filenames. Large log files (>50MB) are common on busy servers; streaming is required.
**Touches:** `presentation/handlers/logs.rs`, `application/log_service.rs`

### LOG3 — GET /instances/:id/crash-reports
**What it does:** Lists all crash reports in `{data_dir}/crash-reports/`.
Same response shape as LOG1.
**Touches:** `presentation/handlers/logs.rs`, `application/log_service.rs`

### LOG4 — GET /instances/:id/crash-reports/:filename
**What it does:** Returns content of a specific crash report (plain text, not compressed).
Same filename validation as LOG2.
**Touches:** `presentation/handlers/logs.rs`, `application/log_service.rs`

---

## SERVER PROPERTIES

### PROP1 — GET /instances/:id/properties
**What it does:** Reads `server.properties` and returns it as JSON for the dashboard.
**How:** Read `{data_dir}/server.properties` line by line. Skip `#` comments and blank
lines. Split each line on the first `=` to get key/value pairs. Return as JSON object.
**Important:** This is NOT a standard .ini/.toml file. Parse it manually.
**Touches:** new `presentation/handlers/properties.rs`, `infrastructure/minecraft/server_properties.rs`

### PROP2 — PATCH /instances/:id/properties
**What it does:** Updates one or more fields in `server.properties` without destroying
the file or its comments.
**Request:** `{ "max-players": "40", "view-distance": "12" }`
**How:** Read file line by line. Replace values for matching keys. Append any keys not
found in the file. Write back. Preserve all comments and unchanged lines exactly.
**Important:** Changing `server-port` must also update the `port` column in the
`instances` table (DB and file must stay in sync). Note in response that most property
changes only take effect after server restart.
**Touches:** `presentation/handlers/properties.rs`, `infrastructure/minecraft/server_properties.rs`,
`application/instance_service.rs` (update port)

---

## MODPACK IMPROVEMENTS

### MP1 — Install Modpack from Modrinth ID
**What it does:** Installs a modpack by Modrinth project+version ID without the client
needing to upload the .mrpack file. The Core downloads it directly.
**Request:** `{ "project_id": "aabbcc", "version_id": "P7dR8mSH" }`
**How:** Fetch version metadata, find the file in `files[]` where filename ends `.mrpack`,
download to a temp path, then run the existing install pipeline.
**Important:** Some versions have multiple files. Match on `.mrpack` extension, not on
`"primary": true` (the primary file is often a different format).
Return 202 Accepted; install runs in the background same as file upload.
**Touches:** `application/modpack_service.rs`, `presentation/handlers/modpack.rs`

### MP2 — Export Instance to .mrpack
**What it does:** Generates a `.mrpack` file from the current server state so owners can
distribute the exact mod setup to players as a client-side modpack.
**How:** A `.mrpack` is a ZIP file containing:
- `modrinth.index.json` — manifest with game version, loader, and list of Modrinth-tracked
  mods (each entry has CDN URL + SHA512 hash from the `mods` table)
- `overrides/mods/` — private/uploaded mods included as actual files in the ZIP
- `overrides/config/` — any config files from the server
**Important:** Use the `sha512` stored in the `mods` table — do not rehash files. This
requires the `sha512` column to exist (part of M0 schema). Without it, export is broken.
**Touches:** new `application/export_service.rs`, new `presentation/handlers/modpack.rs`

---

## MISC ENDPOINTS

### MISC1 — POST /instances/:id/restart
**What it does:** Stops and restarts the server in a single API call.
**How:** Call stop, poll instance status until it reaches `stopped` (or 30s timeout),
then call start. Return 200 on success or 504 if timeout exceeded.
**Important:** Do NOT use sleep(). Poll actual process state.
**Touches:** `application/instance_service.rs` or `instance_status_service.rs`

### MISC2 — GET /instances/:id/stats
**What it does:** Returns live resource usage for a running server instance.
**Fields:** cpu_percent (f32), memory_mb (u64), player_count (u32), uptime_seconds (u64).
All fields are null if the server is not running.
**How:** Use the `sysinfo` crate to get CPU/RAM by the Java process PID. Parse
`player_count` from the live log stream. Track process start time for uptime.
**Important:** Never read `/proc` directly — the app runs on Windows via Tauri. The
`sysinfo` crate is cross-platform and is the only acceptable approach.
**Touches:** new `application/stats_service.rs`, new `presentation/handlers/stats.rs`

---

## DEFERRED — TODO Comments Only, No Implementation

Add `// TODO` comments in source code and update `apps/core/AGENTS.md`.

| Feature | Where | Comment |
|---------|-------|---------|
| World backups (manual) | `instance_service.rs` near data_dir usage | `// TODO(backups): zip {data_dir}/world` |
| Scheduled backups | same file or future `backup_service.rs` | `// TODO(backups/scheduled): tokio-cron-scheduler` |
| Playit.gg tunnel | instance start logic | `// TODO(networking/playit): tunnel on start` |
| UPnP port forwarding | instance start logic | `// TODO(networking/upnp): igd2 crate` |
| File manager | future `file_service.rs` | `// TODO(filemgr): browse/read/edit instance files` |
| Import existing server | `create_instance` | `// TODO(import): accept existing server dir` |
| Auto-restart on crash | process exit handler | `// TODO(resilience): restart policy on crash` |
