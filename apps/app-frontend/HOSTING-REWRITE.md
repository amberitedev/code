# Hosting → Core Rewrite — Stage 1 Audit

## How the System Works (Route Architecture)

There are **two separate route trees** for server management. Both reuse the same `@modrinth/ui` components.

| Route | File | Target | Status |
|---|---|---|---|
| `/hosting/manage/:id` | `pages/hosting/manage/Index.vue` | Modrinth cloud (Archon/Kyros APIs) | Works for cloud |
| `/server/:id` | `pages/server/Index.vue` | Local Core server | **Broken** |

The user lands on `/server/:id`. This is the one we're fixing.

---

## How `/server/:id` Works — The Proxy Architecture

`Server.Index.vue` boots a translation layer on every mount:

1. `route.params.id` = a **Tauri profile path** (e.g. `/home/.amberite/profiles/abc`), NOT a UUID
2. Calls `getProfile(profilePath)` → `GameInstance` → reads `.core_instance_id` (the Core UUID)
3. Calls `core_get_url()` (Tauri invoke) → e.g. `"http://localhost:16662"`
4. Creates `CoreWebSocketClient` — polls Core HTTP every 2s (state) and 3s (stats) instead of a real Archon WebSocket
5. Creates `createCoreClient(realClient, sockets, baseUrl)` — a JS `Proxy` that intercepts `archon.*` and `kyros.*` calls and routes them to Core HTTP
6. Calls `provideModrinthClient(coreClient)` — replaces the injected API client for all children
7. Installs a fake `WebSocket` intercept before `ServersManageRootLayout` mounts, because the layout runs a `/pingtest` WS connectivity check against `serverData.node.instance` — which doesn't exist for local Core servers. The fake WS fakes a successful pong, bypassing this check.
8. Passes `serverId = profile.core_instance_id` to `ServersManageRootLayout`

The proxy maps:
- `archon.servers_v0` → `buildServersV0(baseUrl)` — calls `core_get_instance()` via Tauri, maps to `Archon.Servers.v0.Server`
- `archon.servers_v1` → `buildServersV1()` — same, maps to `Archon.Servers.v1.ServerFull`
- `archon.content_v1` → `buildContentV1(baseUrl)` — calls Core's `/instances/{id}/mods` REST endpoints
- `archon.backups_queue_v1` → `buildBackupsQueueV1()` — **stubbed, all noops and empty responses**
- `archon.sockets` → `CoreWebSocketClient` — polls instead of real WS
- `kyros.files_v0` → `buildKyrosFiles()` — **stubbed, broken shape, no real impl**

The `ServersManageRootLayout` (from `packages/ui`) never knows the difference — it just uses whatever client is injected.

---

## What `ServersManageRootLayout` Provides to Children

All child pages get this context via `injectModrinthServerContext()`:

| Field | Type | Source |
|---|---|---|
| `serverId` | `string` | Passed prop — Core instance UUID |
| `worldId` | `Ref<string \| null>` | From `servers_v1.worlds[0].id` — currently = `inst.id` (same UUID) |
| `server` | `Ref<Archon.Servers.v0.Server>` | TanStack query via `servers_v0.get(serverId)` |
| `powerState` | `Ref<PowerState>` | Polled from Core every 2s via `CoreWebSocketClient.statePolls` |
| `isServerRunning` | `ComputedRef<boolean>` | `powerState === 'running'` |
| `stats` | `Ref<Stats>` | Polled from Core `/instances/{id}/stats` every 3s |
| `uptimeSeconds` | `Ref<number>` | From WS state event `uptime` field |
| `isConnected` | `Ref<boolean>` | True when Core WS console opens |
| `busyReasons` | `ComputedRef<BusyReason[]>` | Populated during content sync operations |
| `fsAuth` | `Ref<{url, token} \| null>` | From `getFilesystemAuth()` — currently returns `{ url: '', token: '' }` |
| `uploadState` | `Ref<UploadState>` | Managed by Files/Content pages |

---

## Confirmed Bugs — Crash-Level

### Bug 1: Content page — `mods.map is not a function`
**File:** `apps/app-frontend/src/helpers/core-client.ts:84-88`

Core `GET /instances/{id}/mods` returns:
```json
{ "mods": [ { "filename": "...", "display_name": "...", ... } ] }
```

The proxy does:
```ts
const mods: Mod[] = res.ok ? await res.json() : []
// mods = { mods: [...] }  ← an object, NOT an array
return { ..., addons: mods.map(toAddon) }
// TypeError: mods.map is not a function  ← CRASH
```

**Fix:** Change to `const data = await res.json(); const mods = data.mods ?? []`

### Bug 2: Toggle mod enable/disable sends wrong field name
**File:** `apps/app-frontend/src/helpers/core-client.ts:93-94`

The proxy sends:
```json
{ "disabled": true }   // or { "disabled": false }
```

Core `PATCH /instances/:id/mods/:filename` expects:
```json
{ "enabled": boolean }
```

The field name is inverted and flipped. Core silently ignores the unknown `disabled` field.

**Fix:** Send `{ "enabled": !disabled }` instead.

### Bug 3: Status "offline" and "crashed" not handled
**File:** `apps/app-frontend/src/helpers/core-client.ts:8-10` and `core-ws-client.ts:28-33`

Core's `InstanceStatus` serializes as:
- `Offline` → `"offline"` (NOT `"stopped"`)
- `Crashed` → `"crashed"`

The TypeScript type annotation in `core.ts` incorrectly documents `"stopped"` instead of `"offline"`.

Effect on `STATUS_TO_POWER` (used every 2s state poll):
- `"offline"` → `STATUS_TO_POWER['offline']` → `undefined` → falls back to `?? 'idle'` (accidentally correct)
- `"crashed"` → `STATUS_TO_POWER['crashed']` → `undefined` → falls back to `?? 'idle'` → **server crash shows as "stopped"** instead of "crashed"

Effect on `STATUS_TO_V0` (used by `coreToV0`):
- `"offline"` → `undefined` → `?? 'available'` (fine)
- `"crashed"` → `undefined` → `?? 'available'` (fine, v0 status doesn't matter much)

**Fix:** Add `"offline"` → `"idle"` and `"crashed"` → `"crashed"` to both maps. Fix type annotation.

---

## Confirmed Bugs — Functional (Non-Crash)

### Bug 4: Files page — wrong response shape
**File:** `apps/app-frontend/src/helpers/core-client.ts:114-116`

```ts
buildKyrosFiles().listDirectory = async () => ({ entries: [], total: 0 })
```

The UI reads `.items` (Kyros `DirectoryItem[]`) — gets `undefined`. The Files page renders empty and may throw in child components that expect an array.

No Core file manager API exists. This is a **gap** as much as a bug.

### Bug 5: RAM total hardcoded to 4GB
**File:** `apps/app-frontend/src/helpers/core-ws-client.ts:149`

```ts
ram_total_bytes: 4 * 1024 * 1024 * 1024,  // always 4GB
```

Core `/instances/{id}/stats` does not return memory total. Core `GET /instances/{id}` returns `memory: { min_mb, max_mb }`. The stats poll would need to also fetch instance details (or cache them from the initial load) to get the real `max_mb`.

### Bug 6: Crash log analysis — empty download
**File:** `apps/app-frontend/src/helpers/core-client.ts:115`

```ts
buildKyrosFiles().downloadFile = async () => ''
```

`ServersManageOverviewPage` calls `downloadFile('/logs/latest.log')` when `powerState === 'crashed'`. Gets empty string → sends to mclogs → analysis is empty/broken.

Core has `GET /instances/{id}/logs/latest.log` that returns the raw log file. This needs to be plumbed through `downloadFile`.

### Bug 7: `addAddon` sends wrong request body
**File:** `apps/app-frontend/src/helpers/core-client.ts:89-91`

```ts
addAddon: async (id, _w, req: Archon.Content.v1.AddAddonRequest) => {
    await fetch(base(id), { method: 'POST', headers, body: JSON.stringify(req) })
}
```

`Archon.Content.v1.AddAddonRequest` shape (from Archon API) likely contains `{ project_id, version_id, ... }`. Core `POST /instances/{id}/mods` expects only `{ "version_id": "string" }`. The extra fields cause no hard error (serde ignores unknown fields) but the intent is `version_id` — should only send that.

### Bug 8: `player_count` and `uptime_seconds` from Core stats ignored
**File:** `apps/app-frontend/src/helpers/core-ws-client.ts:143-155`

Core returns `{ cpu_percent?, memory_mb?, player_count?, uptime_seconds? }`.

`player_count` and `uptime_seconds` are never forwarded to the WS stats event — they're silently dropped. The UI could display player count on the Overview panel.

### Bug 9: Modpack install is a noop
**File:** `apps/app-frontend/src/helpers/core-client.ts:100`

```ts
installContent: noop
```

The "Install from modpack" flow in the Content tab calls `installContent(...)`. This does nothing. Core has `POST /instances/{id}/modpack` (multipart `.mrpack` upload). This flow is completely missing.

---

## Complete Gap Map (Missing Features)

| Feature | Core API exists? | Current state |
|---|---|---|
| **Files tab** | ❌ No file manager API | Stub returns wrong shape; page is empty/broken |
| **Backups tab** | ❌ No backup API | Stub returns empty list; page appears "no backups" |
| **Logs tab** | ✅ `GET /instances/{id}/logs` + filenames | Route doesn't exist in `/server/:id` at all |
| **Crash log analysis** | ✅ `GET /instances/{id}/logs/latest.log` | `downloadFile` stub returns `''` |
| **Modpack install** | ✅ `POST /instances/{id}/modpack` (multipart) | `installContent` is noop |
| **Modpack info** | ✅ `GET /instances/{id}/modpack` | `getModpackUpdate/getAddonUpdate` return null stubs |
| **Modpack export** | ✅ `GET /instances/{id}/modpack/export` | Not wired |
| **Server properties** | ✅ `GET/PATCH /instances/{id}/properties` | No UI page; not wired anywhere |
| **Macros (scripts)** | ✅ `GET/POST/DELETE /instances/{id}/macros` | Not wired; no UI |
| **RAM total in stats** | ✅ From `GET /instances/{id}` → `memory.max_mb` | Hardcoded 4GB |
| **Player count** | ✅ From `GET /instances/{id}/stats` → `player_count` | Not forwarded to UI |
| **Crash status display** | ✅ Core sends `"crashed"` status | Maps to `'idle'`/stopped; crash overlay never shows |

---

## Core API — Full Route Table for Reference

All routes require `Authorization: Bearer <token>` (or bypass in dev mode).

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/instances` | — | `{ instances: Instance[] }` |
| POST | `/instances` | `{ name, game_version, loader, loader_version?, port, memory? }` | `201 Instance` |
| GET | `/instances/:id` | — | `Instance` (with `data_dir`, `java_version`) |
| DELETE | `/instances/:id` | — | `{ ok: true }` |
| POST | `/instances/:id/start` | — | `{ ok: true }` |
| POST | `/instances/:id/stop` | — | `{ ok: true }` |
| POST | `/instances/:id/kill` | — | `{ ok: true }` |
| POST | `/instances/:id/restart` | — | `{ ok: true }` |
| POST | `/instances/:id/command` | `{ command: string }` | `{ ok: true }` |
| GET | `/instances/:id/console` | WS (ticket in query) | Stream of text lines |
| GET | `/instances/:id/progress` | SSE | `{ type: "creation_progress", progress: 0–1, message }` |
| GET | `/instances/:id/stats` | — | `{ cpu_percent?, memory_mb?, player_count?, uptime_seconds? }` |
| GET | `/instances/:id/mods` | — | `{ mods: ModInfo[] }` |
| POST | `/instances/:id/mods` | `{ version_id: string }` | `ModInfo` |
| POST | `/instances/:id/mods/upload` | multipart `file` field | `{ ok: true, filename }` |
| DELETE | `/instances/:id/mods/:filename` | — | `{ ok: true, restart_required: bool }` |
| PATCH | `/instances/:id/mods/:filename` | `{ enabled: bool }` | `{ ok: true }` |
| PUT | `/instances/:id/mods/:filename/update` | — | `{ updated: ... }` |
| POST | `/instances/:id/mods/update-all` | — | `{ updated[], already_latest[], failed[] }` |
| POST | `/instances/:id/modpack` | multipart `mrpack` field | `ModpackManifest` |
| GET | `/instances/:id/modpack` | — | `ModpackManifest` or 404 |
| DELETE | `/instances/:id/modpack` | — | `{ ok: true }` |
| GET | `/instances/:id/modpack/export` | — | Binary `.mrpack` download |
| GET | `/instances/:id/logs` | — | `{ logs: LogEntry[] }` |
| GET | `/instances/:id/logs/:filename` | — | Raw text (gzip if `.gz`) |
| GET | `/instances/:id/crash-reports` | — | `{ crash_reports: LogEntry[] }` |
| GET | `/instances/:id/crash-reports/:filename` | — | Raw text |
| GET | `/instances/:id/properties` | — | `{ properties: { [key]: string } }` |
| PATCH | `/instances/:id/properties` | `{ [key]: string }` | `{ updated_keys: string[] }` |
| GET | `/instances/:id/macros` | — | macro list |
| POST | `/instances/:id/macros` | macro spawn body | macro info |
| DELETE | `/instances/:id/macros/:pid` | — | `{ ok: true }` |

### Response shapes

**`Instance` (summary from `GET /instances`):**
```ts
{ id, name, game_version, loader, loader_version, port, memory: { min_mb, max_mb }, status }
```

**`Instance` (detail from `GET /instances/:id`):**
```ts
{ id, name, game_version, loader, loader_version, port, memory, java_version, status, data_dir, created_at, updated_at }
```

**`InstanceStatus` values:** `"offline"` | `"starting"` | `"running"` | `"stopping"` | `"crashed"`

**`ModLoader` values:** `"vanilla"` | `"paper"` | `"fabric"` | `"forge"` | `"neoforge"` | `"quilt"`

**`ModInfo`:**
```ts
{ id: string|null, filename, display_name, version_number, enabled: bool, tracked: bool,
  client_side, server_side, modrinth_project_id, update_available: null }
```

**`LogEntry`:**
```ts
{ filename, size_bytes: number, modified_at: string }  // modified_at = RFC 3339 or ""
```

**Stats:**
```ts
{ cpu_percent?: number, memory_mb?: number, player_count?: number, uptime_seconds?: number }
```

---

## What Needs to Change — Files That Must Be Modified

| File | Changes needed |
|---|---|
| `helpers/core-client.ts` | Fix `getAddons` (unwrap `{ mods: [...] }`), fix `disableAddon`/`enableAddon` (flip field), fix `addAddon` body, wire `installContent`, wire `downloadFile` to Core logs |
| `helpers/core-ws-client.ts` | Fix `STATUS_TO_POWER` (add `offline`/`crashed`), fix `ram_total_bytes` (fetch from instance), forward `player_count`/`uptime_seconds` |
| `helpers/core.ts` | Fix `CoreInstanceDetail.status` type annotation (`"offline" \| "starting" \| "running" \| "stopping" \| "crashed"`) |
| `pages/server/Index.vue` | Pass `memory.max_mb` to WS client for accurate RAM display |
| `pages/server/Files.vue` | Either: (a) connect to Core file API, (b) hide tab, or (c) Tauri FS |
| `pages/server/Backups.vue` | Either: (a) connect to Core backup API, (b) hide tab |
| `routes.js` | Add `/server/:id/logs` route if we add a Logs page |

---

---

## Stage 2 — Full Rewrite Plan

Execution order follows the declared priority: Core first (missing APIs), then proxy correctness, then wire missing tabs.

---

### Phase 1 — Fix Existing Proxy Bugs

All changes in `apps/app-frontend/src/helpers/`. No Core changes needed.

#### 1.1 `helpers/core.ts`
- Add `memory?: { min_mb: number; max_mb: number }` to `CoreInstanceDetail`.
- Fix `status` type annotation: `"offline" | "starting" | "running" | "stopping" | "crashed"`.

#### 1.2 `helpers/core-ws-client.ts`
- **STATUS_TO_POWER**: add `offline → 'idle'` and `crashed → 'crashed'`.
- **`CoreInstanceDetail` cache**: in the state poller, cache `inst.memory?.max_mb` in a per-server `Map<string, number>`. Use it as `ram_total_bytes` in the stats event.
- **Forward `player_count` / `uptime_seconds`**: parse the stats response fully; include `player_count` and `uptime_seconds` fields in the emitted `WSStatsEvent` (even if the stock event type ignores extras, forwarding them keeps the door open for later).

#### 1.3 `helpers/core-client.ts`
- **Bug 1 — `getAddons`**: unwrap `data.mods ?? []` instead of treating the whole object as an array.
- **Bug 2 — `disableAddon` / `enableAddon`**: send `{ enabled: false }` / `{ enabled: true }` (not `disabled`).
- **Bug 3 — `STATUS_TO_V0`**: add `offline → 'available'` and `crashed → 'available'` (v0 status doesn't render a crash overlay; only `power_variant` matters).
- **Bug 7 — `addAddon`**: only send `{ version_id: req.version_id }` to Core.
- **Bug 6 — `downloadFile`**: delegate to `GET ${baseUrl}/instances/${id}/logs/latest.log` when path is `/logs/latest.log`; for all other paths, fetch `${baseUrl}/instances/${id}/fs/download?path=${encodeURIComponent(path)}` (will work once Phase 2 lands) and return a `Blob`.
- **Bug 9 — `installContent`**: call `POST ${baseUrl}/instances/${id}/modpack` with a FormData containing the `.mrpack` file from the request.

#### 1.4 `pages/server/Index.vue`
- **Bug 5 — RAM total**: after resolving `coreInstanceId`, call `core_get_instance(coreInstanceId)` once and pass `instance.memory?.max_mb` to `CoreWebSocketClient` via a new `setRamTotal(mb: number)` method. The WS client stores it and uses it in every stats emission.

---

### Phase 2 — Core: File Manager API

**New files:**
| File | Role |
|---|---|
| `apps/core/src/application/fs_service.rs` | File system service |
| `apps/core/src/presentation/handlers/fs.rs` | HTTP handlers |

**Modified files:**
| File | Change |
|---|---|
| `apps/core/src/application/mod.rs` | `pub mod fs_service;` |
| `apps/core/src/presentation/handlers/mod.rs` | `pub mod fs;` |
| `apps/core/src/presentation/router.rs` | Register all `/instances/:id/fs/*` routes |

#### 2.1 Route table

| Method | Path | Query | Body | Response |
|---|---|---|---|---|
| GET | `/instances/:id/fs/list` | `path`, `page` (def 1), `page_size` (def 100) | — | `DirectoryResponse` |
| GET | `/instances/:id/fs/download` | `path` | — | file bytes + Content-Type |
| POST | `/instances/:id/fs/create` | `path`, `type` (`file`\|`directory`) | multipart `file` field (file only, optional) | `{}` |
| PUT | `/instances/:id/fs/update` | `path` | raw bytes (`application/octet-stream`) | `{}` |
| POST | `/instances/:id/fs/move` | — | `{ "source": string, "destination": string }` | `{}` |
| DELETE | `/instances/:id/fs/delete` | `path`, `recursive` (bool, def false) | — | `{}` |
| POST | `/instances/:id/fs/extract` | `src`, `trg` (def `/`), `override` (bool), `dry` (bool) | — | `ExtractResult` |

All require `AuthUser`.

#### 2.2 Response types (Rust structs, serialized to JSON)

```rust
// mirrors Kyros.Files.v0.DirectoryItem
pub struct DirectoryItem {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,          // "file" | "directory" | "symlink"
    pub path: String,          // relative to data_dir root, leading slash
    pub modified: i64,         // Unix seconds
    pub created: i64,          // Unix seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

// mirrors Kyros.Files.v0.DirectoryResponse
pub struct DirectoryResponse {
    pub items: Vec<DirectoryItem>,
    pub total: usize,    // TOTAL PAGES — not total items
    pub current: usize,  // current page number
}

// mirrors Kyros.Files.v0.ExtractResult
pub struct ExtractResult {
    pub modpack_name: Option<String>,
    pub conflicting_files: Vec<String>,
}
```

#### 2.3 `fs_service.rs` — service functions

```rust
// Resolves a relative path against data_dir; rejects traversal (MUST canonicalize).
fn resolve_path(data_dir: &Path, rel: &str) -> Result<PathBuf, FsError>

pub async fn list_directory(
    data_dir: &Path,
    path: &str,
    page: usize,        // 1-indexed
    page_size: usize,
) -> Result<DirectoryResponse, FsError>

pub async fn download_file(data_dir: &Path, path: &str) -> Result<Vec<u8>, FsError>

pub async fn create_directory(data_dir: &Path, path: &str) -> Result<(), FsError>

pub async fn create_file(
    data_dir: &Path,
    path: &str,
    content: Option<Vec<u8>>,
) -> Result<(), FsError>

pub async fn update_file(
    data_dir: &Path,
    path: &str,
    content: Vec<u8>,
) -> Result<(), FsError>

pub async fn move_item(
    data_dir: &Path,
    source: &str,
    destination: &str,
) -> Result<(), FsError>

pub async fn delete_item(
    data_dir: &Path,
    path: &str,
    recursive: bool,
) -> Result<(), FsError>

pub async fn extract_archive(
    data_dir: &Path,
    src: &str,
    trg: &str,
    override_existing: bool,
    dry_run: bool,
) -> Result<ExtractResult, FsError>
```

#### 2.4 Path traversal guard — `resolve_path`

```rust
fn resolve_path(data_dir: &Path, rel: &str) -> Result<PathBuf, FsError> {
    // Join without canonicalize first (file may not exist yet)
    let stripped = rel.trim_start_matches('/');
    let joined = data_dir.join(stripped);
    // Collapse .. components lexically
    let mut canonical = PathBuf::new();
    for component in joined.components() {
        match component {
            std::path::Component::ParentDir => { canonical.pop(); }
            c => canonical.push(c),
        }
    }
    if !canonical.starts_with(data_dir) {
        return Err(FsError::Forbidden("path escapes instance directory".into()));
    }
    Ok(canonical)
}
```

#### 2.5 `list_directory` — sorting and pagination

Sort order: directories first, then files, both groups sorted alphabetically by name (case-insensitive). Symlinks sorted with files. `total` = `ceil(total_items / page_size)`. If `total_items == 0`, `total = 1` (avoid 0-page edge case). `current = page`.

#### 2.6 `extract_archive` — ZIP extraction

Use the `zip` crate already in Cargo.toml:
1. Open the archive at the resolved `src` path.
2. If `dry_run = true`: list all entries, check which exist at their target path under `trg`; return `conflicting_files` list without writing anything.
3. If `dry_run = false`: for each entry, resolve target path (path traversal guard applies to each entry), optionally overwrite if `override_existing`, write file.
4. Detect `modpack_name` by looking for a `modrinth.index.json` entry; if found, parse `"name"` field.
5. Only supports `.zip` archives (mrpack is zip). Return `FsError::UnsupportedArchive` for other formats.

---

### Phase 3 — Core: Backup API

**New/modified files:**
| File | Change |
|---|---|
| `apps/core/src/domain/backup.rs` | Backup domain types |
| `apps/core/src/domain/mod.rs` | `pub mod backup;` |
| `apps/core/src/application/backup_service.rs` | Backup service |
| `apps/core/src/application/mod.rs` | `pub mod backup_service;` |
| `apps/core/src/presentation/handlers/backups.rs` | HTTP handlers |
| `apps/core/src/presentation/handlers/mod.rs` | `pub mod backups;` |
| `apps/core/src/presentation/router.rs` | Register backup routes |
| `apps/core/migrations/007_backups.sql` | New migration |
| `apps/core/src/presentation/error.rs` | `BackupError` → `ApiError` |

#### 3.1 Migration `007_backups.sql`

```sql
CREATE TABLE IF NOT EXISTS backups (
    id           TEXT    PRIMARY KEY,
    instance_id  TEXT    NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    name         TEXT    NOT NULL,
    created_at   TEXT    NOT NULL,
    status       TEXT    NOT NULL DEFAULT 'pending',
    locked       INTEGER NOT NULL DEFAULT 0,
    automated    INTEGER NOT NULL DEFAULT 0,
    size_bytes   INTEGER,
    archive_path TEXT    NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS backup_operations (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    backup_id      TEXT    NOT NULL REFERENCES backups(id) ON DELETE CASCADE,
    operation_type TEXT    NOT NULL,   -- 'create' | 'restore'
    state          TEXT    NOT NULL DEFAULT 'pending',
    scheduled_for  TEXT    NOT NULL,
    completed_at   TEXT,
    has_parent     INTEGER NOT NULL DEFAULT 0,
    error          TEXT
);

CREATE TABLE IF NOT EXISTS backup_schedules (
    instance_id   TEXT    PRIMARY KEY REFERENCES instances(id) ON DELETE CASCADE,
    cron          TEXT    NOT NULL,
    retain_count  INTEGER NOT NULL DEFAULT 5,
    enabled       INTEGER NOT NULL DEFAULT 1
);
```

#### 3.2 Domain types `domain/backup.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub id: String,           // UUID
    pub instance_id: String,
    pub name: String,
    pub created_at: String,   // RFC 3339
    pub status: String,       // "pending" | "in_progress" | "error" | "done"
    pub locked: bool,
    pub automated: bool,
    pub size_bytes: Option<i64>,
    pub archive_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupOperation {
    pub id: i64,
    pub backup_id: String,
    pub operation_type: String,    // "create" | "restore"
    pub state: String,             // "pending" | "ongoing" | "completed" | "failed"
    pub scheduled_for: String,
    pub completed_at: Option<String>,
    pub has_parent: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSchedule {
    pub instance_id: String,
    pub cron: String,
    pub retain_count: i32,
    pub enabled: bool,
}
```

#### 3.3 Route table

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/instances/:id/backups` | — | `BackupsQueueResponse` |
| POST | `/instances/:id/backups` | `{ "name": string }` | `{ "id": string }` |
| DELETE | `/instances/:id/backups/:bid` | — | `{ "ok": true }` |
| POST | `/instances/:id/backups/:bid/restore` | `{ "name": string }` | `{ "ok": true }` |
| POST | `/instances/:id/backups/delete-many` | `{ "backup_ids": string[] }` | `{ "ok": true }` |
| POST | `/instances/:id/backups/:bid/ack` | `{ "operation_id": number, "operation_type": string }` | `{ "ok": true }` |
| GET | `/instances/:id/backups/schedule` | — | `BackupScheduleResponse` |
| PUT | `/instances/:id/backups/schedule` | `{ "cron": string, "retain_count": number }` | `BackupScheduleResponse` |
| DELETE | `/instances/:id/backups/schedule` | — | `{ "ok": true }` |

`BackupScheduleResponse`: `{ "enabled": bool, "cron": string | null, "retain_count": number }`

All require `AuthUser`.

**Route ordering note:** `/instances/:id/backups/delete-many`, `/instances/:id/backups/schedule` must be registered BEFORE `/instances/:id/backups/:bid` so Axum doesn't route `"delete-many"` / `"schedule"` as a backup ID.

#### 3.4 `BackupsQueueResponse` — response shape (matches Archon exactly)

```rust
pub struct BackupsQueueResponse {
    pub active_operations: Vec<ActiveOperation>,
    pub backups: Vec<BackupQueueBackup>,
}

pub struct BackupQueueBackup {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub status: String,
    pub locked: bool,
    pub automated: bool,
    pub history: Vec<BackupQueueOperation>,
}

pub struct ActiveOperation {
    pub backup_id: String,
    pub operation_type: String,
    pub operation_id: Option<i64>,
    pub has_parent: bool,
    pub scheduled_for: String,
    pub synthetic_legacy: bool,   // always false
}

pub struct BackupQueueOperation {
    pub operation_type: String,
    pub operation_id: Option<i64>,
    pub state: String,
    pub scheduled_for: String,
    pub completed_at: Option<String>,
    pub has_parent: bool,
    pub error: Option<String>,
    pub should_prompt: bool,      // always false — Core never requires ack
    pub synthetic_legacy: bool,   // always false
}
```

#### 3.5 `backup_service.rs` — service functions

```rust
// Returns the full list response: all backups + active ops for an instance.
pub async fn list_backups(
    state: &Arc<AppState>,
    instance_id: &InstanceId,
) -> Result<BackupsQueueResponse, BackupError>

// Creates a new backup record (status=pending), spawns a tokio task to ZIP
// the data_dir (excluding logs/ and crash-reports/), updates status to
// in_progress → done or error.
pub async fn create_backup(
    state: Arc<AppState>,
    instance_id: InstanceId,
    name: String,
    automated: bool,
) -> Result<String /* backup UUID */, BackupError>

// Stops instance if running, restores from archive, restarts if was running.
// Creates an auto-backup of the current data_dir first (with the given name).
pub async fn restore_backup(
    state: Arc<AppState>,
    instance_id: InstanceId,
    backup_id: String,
    pre_restore_backup_name: String,
) -> Result<(), BackupError>

pub async fn delete_backup(
    state: &Arc<AppState>,
    backup_id: &str,
) -> Result<(), BackupError>

pub async fn delete_many_backups(
    state: &Arc<AppState>,
    backup_ids: &[String],
) -> Result<(), BackupError>

// No-op — Core never sets should_prompt=true.
pub async fn ack_operation(
    state: &Arc<AppState>,
    _backup_id: &str,
    _operation_id: i64,
) -> Result<(), BackupError>

pub async fn get_schedule(
    state: &Arc<AppState>,
    instance_id: &InstanceId,
) -> Result<Option<BackupSchedule>, BackupError>

pub async fn set_schedule(
    state: &Arc<AppState>,
    instance_id: &InstanceId,
    cron: String,
    retain_count: i32,
) -> Result<BackupSchedule, BackupError>

pub async fn delete_schedule(
    state: &Arc<AppState>,
    instance_id: &InstanceId,
) -> Result<(), BackupError>
```

#### 3.6 Backup archive strategy

- **Location**: `{AMBERITE_DATA_DIR}/backups/{instance_id}/{backup_uuid}.zip`
- **Contents**: entire `data_dir` as a ZIP, **excluding** `logs/` and `crash-reports/` subdirectories.
- **Restore**: unzip archive into a fresh temp dir, stop the running instance, replace `data_dir` contents (preserving `logs/` and `crash-reports/`), restart.
- **Cron execution**: schedule table is loaded at startup; a background tokio task checks every minute and fires `create_backup(..., automated=true)`. On create, if `retain_count` is exceeded, the oldest automated backup (by `created_at`, `status=done`, `locked=false`) is deleted automatically.
- **Cron format**: standard 5-field cron (`* * * * *`). Parse with the `cron` crate (already present in Cargo.toml as a transitive dep, or add `cron = "0.12"` — check Cargo.lock first).

---

### Phase 4 — Frontend: Wire File Manager Proxy

**File: `apps/app-frontend/src/helpers/core-client.ts`** — replace `buildKyrosFiles`.

#### 4.1 `buildKyrosFiles(baseUrl: string, instanceId: string)`

All methods call `${baseUrl}/instances/${instanceId}/fs/*` directly. Auth header is omitted (dev-mode bypass; production will require a token — deferred to a later pass).

```ts
// listDirectory(path, page, pageSize) → DirectoryResponse
// downloadFile(path) → Blob
// createFileOrFolder(path, type) → void
// uploadFile(path, file, options?) → UploadHandle<void>
// updateFile(path, content) → void
// moveFileOrFolder(source, destination) → void
// renameFileOrFolder(path, newName) → delegates to moveFileOrFolder
// deleteFileOrFolder(path, recursive) → void
// extractFile(path, override, dry) → ExtractResult
// modifyOperation(opId, action) → void (noop — Core has no long-running op IDs)
// WithAuth variants — ignore the auth param, call Core directly
```

**`uploadFile` implementation**: must return an `UploadHandle<void>` with progress tracking. Use `XMLHttpRequest` for progress events; wrap in a `{ promise, onProgress, cancel }` object matching the `UploadHandle<T>` interface.

```ts
function buildUploadHandle(url: string, formData: FormData): UploadHandle<void> {
    const callbacks: Array<(p: UploadProgress) => void> = []
    let xhr: XMLHttpRequest
    const promise = new Promise<void>((resolve, reject) => {
        xhr = new XMLHttpRequest()
        xhr.open('POST', url)
        xhr.upload.onprogress = (e) => {
            if (!e.lengthComputable) return
            const p: UploadProgress = {
                loaded: e.loaded, total: e.total, progress: e.loaded / e.total,
            }
            callbacks.forEach((cb) => cb(p))
        }
        xhr.onload = () => (xhr.status < 400 ? resolve() : reject(new Error(String(xhr.status))))
        xhr.onerror = () => reject(new Error('XHR error'))
        xhr.send(formData)
    })
    const handle: UploadHandle<void> = {
        promise,
        onProgress: (cb) => { callbacks.push(cb); return handle },
        cancel: () => xhr?.abort(),
    }
    return handle
}
```

#### 4.2 `getFilesystemAuth` fix

In `buildServersV0`, change:
```ts
getFilesystemAuth: async () => ({ url: '', token: '' })
```
to:
```ts
getFilesystemAuth: async () => ({ url: baseUrl, token: '' })
```

This gives `WithAuth` callers (server icon components) a usable base URL. Token is empty — dev mode bypass. Production will need a real token.

---

### Phase 5 — Frontend: Wire Backups Proxy

**File: `apps/app-frontend/src/helpers/core-client.ts`** — replace `buildBackupsQueueV1`.

```ts
function buildBackupsQueueV1(baseUrl: string) {
    const base = (id: string) => `${baseUrl}/instances/${id}/backups`
    const headers = { 'Content-Type': 'application/json' }
    return {
        list: async (id: string, _worldId: string) => {
            const res = await fetch(base(id))
            return res.ok ? res.json() : { active_operations: [], backups: [] }
        },
        create: async (id: string, _w: string, req: { name: string }) => {
            const res = await fetch(base(id), { method: 'POST', headers, body: JSON.stringify(req) })
            return res.ok ? res.json() : { id: '' }
        },
        delete: async (id: string, _w: string, backupId: string) => {
            await fetch(`${base(id)}/${backupId}`, { method: 'DELETE' })
        },
        deleteMany: async (id: string, _w: string, backupIds: string[]) => {
            await fetch(`${base(id)}/delete-many`, { method: 'POST', headers,
                body: JSON.stringify({ backup_ids: backupIds }) })
        },
        restore: async (id: string, _w: string, backupId: string, req: { name: string }) => {
            await fetch(`${base(id)}/${backupId}/restore`, { method: 'POST', headers,
                body: JSON.stringify(req) })
        },
        ackCreate: async () => {},   // Core never sets should_prompt=true
        ackRestore: async () => {},
        retry: async () => {},
    }
}
```

Also add a `backups_v1` stub to `buildServersV0` (the UI calls `client.archon.backups_v1.delete(...)` for non-done backups):
```ts
buildServersV0 additions:
    backups_v1: {
        delete: async (id: string, _w: string, backupId: string) =>
            fetch(`${base(id)}/${backupId}`, { method: 'DELETE' })
    }
```

Wait — `backups_v1` is on `archon`, not `servers_v0`. It needs its own namespace in the proxy. Add `backups_v1` to the archon proxy in `createCoreClient`:
```ts
if (prop === 'backups_v1') return buildBackupsV1LegacyStub(baseUrl)
```

where `buildBackupsV1LegacyStub` delegates deletes to the same Core endpoint as `backups_queue_v1.delete`.

---

### Phase 6 — Console.vue: Historical Logs

**File: `apps/app-frontend/src/pages/server/Console.vue`** — enhance; keep under 200 lines.

#### 6.1 Design

Two-tab layout: **Live Console** | **Log Files**.

- **Live Console** tab: existing WS-backed console (unchanged behavior). Disabled command input when not connected.
- **Log Files** tab: lists log files fetched from `GET ${baseUrl}/instances/${serverId}/logs` and crash reports from `GET .../crash-reports`. Clicking a log entry fetches and displays the raw text inline.

#### 6.2 Behavior rules

1. On mount: fetch log file list from Core (both `logs` and `crash-reports` lists).
2. If WS connects successfully → stay on Live Console tab.
3. If WS fails to connect (server stopped) → auto-switch to Log Files tab and auto-load `latest.log` (the most recently modified `.log` file).
4. When WS disconnects after being connected → switch to Log Files tab and auto-load latest log.
5. When the user navigates to Live Console tab while the server is stopped → show "Server is not running — switch to Log Files to view history."
6. No re-opening of WS except via unmount/remount.

#### 6.3 New state refs

```ts
const mode = ref<'live' | 'logs'>('live')
const logFiles = ref<Array<{ filename: string; size_bytes: number; modified_at: string; kind: 'log' | 'crash' }>>([])
const selectedLogFilename = ref<string | null>(null)
const selectedLogContent = ref<string>('')
const loadingLog = ref(false)
```

#### 6.4 Log loading function

```ts
async function loadLogFile(filename: string, kind: 'log' | 'crash') {
    loadingLog.value = true
    selectedLogFilename.value = filename
    const endpoint = kind === 'log' ? 'logs' : 'crash-reports'
    try {
        const res = await fetch(`${baseUrl}/instances/${serverId}/${endpoint}/${filename}`)
        selectedLogContent.value = res.ok ? await res.text() : `Error loading ${filename}`
    } finally {
        loadingLog.value = false
    }
}
```

#### 6.5 Auto-load on WS disconnect

```ts
ws.onclose = async () => {
    connected.value = false
    if (logFiles.value.length === 0) await fetchLogList()
    const latest = logFiles.value.find((f) => f.filename === 'latest.log') ?? logFiles.value[0]
    if (latest) { mode.value = 'logs'; await loadLogFile(latest.filename, latest.kind) }
}
```

#### 6.6 File size note

The current Console.vue is 83 lines. The enhanced version with the logs tab will be close to 200 lines — stay within limit.

---

### Execution Summary

| Phase | Files changed (Core) | Files changed (Frontend) |
|---|---|---|
| 1 — Bug fixes | — | `core.ts`, `core-ws-client.ts`, `core-client.ts`, `server/Index.vue` |
| 2 — File manager API | `fs_service.rs` (new), `handlers/fs.rs` (new), `router.rs`, `handlers/mod.rs`, `application/mod.rs` | — |
| 3 — Backup API | `domain/backup.rs` (new), `domain/mod.rs`, `backup_service.rs` (new), `application/mod.rs`, `handlers/backups.rs` (new), `handlers/mod.rs`, `router.rs`, `error.rs`, `007_backups.sql` (new) | — |
| 4 — Wire file manager | — | `core-client.ts` (`buildKyrosFiles`) |
| 5 — Wire backups | — | `core-client.ts` (`buildBackupsQueueV1`, `backups_v1` stub) |
| 6 — Console logs | — | `server/Console.vue` |

Total new Core files: 5. Total modified Core files: 5. Total modified frontend files: 5.
