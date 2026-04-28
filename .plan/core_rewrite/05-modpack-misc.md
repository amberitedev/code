# Part 5 — Modpack Improvements, Misc Endpoints & Deferred TODOs

---

## Modpack Improvements

### MP1 — Install Modpack from Modrinth ID

**Context:** The existing install endpoint only accepts a `.mrpack` file upload. This
extends it to also accept a Modrinth project+version ID so the Core downloads the pack
directly. Both modes run the same install pipeline after the file is obtained.

**Detection:** Look at the `Content-Type` header:
- `multipart/form-data` → file upload mode (existing path)
- `application/json` → Modrinth ID mode (new path)

**Modrinth ID request:**
```json
{ "project_id": "aabbcc", "version_id": "P7dR8mSH" }
```

**Steps (ID mode):**
1. Call Modrinth `GET /version/{version_id}` for the file list
2. Find the file where `filename` ends in `.mrpack`
3. Download it to a temp path
4. Hand off to the existing install pipeline (same code as file upload)
5. Return 202 Accepted; install runs in the background

**Don't:** Match on `"primary": true` to find the `.mrpack` file — for modpacks the
primary file is sometimes a different format. Match on the `.mrpack` extension instead.

**Don't:** Block the HTTP response waiting for the full install. Return 202 immediately.

---

### MP2 — Export Instance to .mrpack

**What:** Generates a `.mrpack` file from the current server's mod state. Used by the
server owner to distribute the exact mod setup as a client-side modpack.

**The `.mrpack` format** is a ZIP archive containing:

1. `modrinth.index.json` — the manifest:
   ```json
   {
     "formatVersion": 1,
     "game": "minecraft",
     "versionId": "{instance_version}",
     "name": "{instance_name}",
     "dependencies": { "minecraft": "{mc_version}", "{loader}": "{loader_version}" },
     "files": [
       {
         "path": "mods/{filename}",
         "hashes": { "sha512": "{hash_from_mods_table}" },
         "downloads": ["https://cdn.modrinth.com/data/{project_id}/versions/{version_id}/{filename}"],
         "fileSize": 12345,
         "env": { "client": "{client_side}", "server": "{server_side}" }
       }
     ]
   }
   ```

2. `overrides/mods/` — contains the actual `.jar` files for any private/uploaded mods
   (those with `modrinth_project_id = null` in the DB). These can't be listed in `files[]`
   because they have no CDN URL.

3. `overrides/config/` — if the instance has a `config/` directory, include it.

**Don't:** Rehash files when building the manifest. Use the `sha512` stored in the `mods`
table. This requires the `sha512` column from migration `004_mods.sql` (M0). If a mod
has no stored hash, compute and store it first, then export.

**Endpoint:** `GET /instances/:id/modpack/export`
Response: binary ZIP file with `Content-Disposition: attachment; filename="{name}.mrpack"`

---

## Misc Endpoints

### MISC1 — POST /instances/:id/restart

**What:** Stops the server and starts it again in a single API call.

**Steps:**
1. Call the stop logic (send SIGTERM / `stop` command to server process)
2. Poll instance status every 500ms until it reaches `stopped`
3. If not stopped within 30 seconds, return HTTP 504 with message "Shutdown timed out"
4. Once stopped, call the start logic
5. Return 200

**Don't:** Use `sleep(3_000)`. Sleep is fragile — some servers take 10+ seconds to flush
chunks and write data. A poll loop is required.

**Touches:** `application/instance_status_service.rs` (after B6 split)

---

### MISC2 — GET /instances/:id/stats

**What:** Returns live resource usage for a running server instance.

**Response:**
```json
{
  "cpu_percent": 12.4,
  "memory_mb": 2048,
  "player_count": 3,
  "uptime_seconds": 3721
}
```
All fields are `null` if the server is not running.

**How to get CPU/RAM:** Use the `sysinfo` crate. Store the Java process PID when the
server is started. Query `sysinfo::System::refresh_process(pid)` at request time.

**How to get player count:** Parse the in-memory log buffer for Minecraft's player count
pattern: `"There are X of a max of Y players online"`. This is emitted on `/list` command.
Alternative: send `/list` via the existing command channel on each stats request and parse
the response line.

**How to get uptime:** Store a `started_at: Instant` in the running process state when
the server starts. Compute `started_at.elapsed().as_secs()` on request.

**Don't:** Read `/proc/{pid}/stat` directly. This is Linux-only and the app runs on
Windows via Tauri. The `sysinfo` crate handles all platforms.

**Touches:** new `application/stats_service.rs`, new `presentation/handlers/stats.rs`

---

## Deferred — TODO Comments Only

The features below are intentionally out of scope for this plan. Add `// TODO` comments
in the exact locations listed so future implementors know where to hook in.

### World Backups
**File:** `application/instance_service.rs` or `instance_status_service.rs`
Near any code that reads `data_dir`:
```rust
// TODO(backups): Implement world backup — create a timestamped zip of {data_dir}/world/
// See .plan/active/features.md for full backup scope and .plan/core_rewrite/README.md
```

### Scheduled Backups
**File:** same file, or near a future `application/backup_service.rs` stub:
```rust
// TODO(backups/scheduled): Auto-backup on a cron schedule — tokio-cron-scheduler crate
```

### Playit.gg Tunnel
**File:** `application/instance_status_service.rs` near server start logic:
```rust
// TODO(networking/playit): Start Playit.gg tunnel on instance start for public access
// without requiring port forwarding. See https://playit.gg/api-docs
```

### UPnP Port Forwarding
**File:** Same location as Playit TODO:
```rust
// TODO(networking/upnp): Request UPnP port mapping on router at instance start
// Use the igd2 crate
```

### File Manager
**File:** `presentation/router.rs` comment block near instance routes:
```rust
// TODO(filemgr): Add file browser endpoints (browse, read, write, upload, download)
// for instance directory. See .plan/active/features.md
```

### Auto-Restart on Crash
**File:** The process exit handler in `instance_status_service.rs`:
```rust
// TODO(resilience): If exit_code != 0 and restart_on_crash is enabled in config,
// automatically restart the instance with a backoff delay
```

---

## AGENTS.md Update

Add a `## Deferred Features` section to `apps/core/AGENTS.md`:

```
## Deferred Features

The following are planned but not yet implemented. Each has a TODO comment in source.

- World backups (manual zip of world/ folder)
- Scheduled auto-backups (cron style)
- Playit.gg tunnel integration
- UPnP port forwarding
- File manager (browse/read/edit/upload instance files)
- Import existing server directory into an instance
- Auto-restart on crash with configurable backoff
```

---

## Full Router Reference After This Plan

```
POST   /instances                              create instance
GET    /instances                              list instances
GET    /instances/:id                          get instance
DELETE /instances/:id                          delete instance
POST   /instances/:id/start                    start server
POST   /instances/:id/stop                     stop server
POST   /instances/:id/kill                     force kill
POST   /instances/:id/restart                  restart (NEW)
GET    /instances/:id/stats                    cpu/ram/players (NEW)
GET    /instances/:id/properties               read server.properties (NEW)
PATCH  /instances/:id/properties               edit server.properties (NEW)
GET    /instances/:id/mods                     list mods (NEW)
POST   /instances/:id/mods                     add mod from Modrinth (NEW)
POST   /instances/:id/mods/upload              add mod from file (NEW)
DELETE /instances/:id/mods/:filename           remove mod (NEW)
PATCH  /instances/:id/mods/:filename           enable/disable mod (NEW)
PUT    /instances/:id/mods/:filename/update    update one mod (NEW)
POST   /instances/:id/mods/update-all          update all mods (NEW)
GET    /instances/:id/logs                     list log files (NEW)
GET    /instances/:id/logs/:filename           read log file (NEW)
GET    /instances/:id/crash-reports            list crash reports (NEW)
GET    /instances/:id/crash-reports/:filename  read crash report (NEW)
POST   /instances/:id/modpack                  install modpack (extended)
GET    /instances/:id/modpack/export           export to .mrpack (NEW)
WS     /instances/:id/console                  live console stream (existing)
POST   /instances/:id/console/command          send command (existing)
GET    /instances/:id/macros                   list macros (existing, fixed)
POST   /instances/:id/macros                   spawn macro (existing)
DELETE /instances/:id/macros/:macro_id         kill macro (existing)
GET    /health                                 health check (existing)
GET    /version                                version info (existing)
```
