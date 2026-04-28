# Amberite Core — Complete Rewrite Plan

> Status: **SEMI IMPLEMENTED**
> Created: 2026-04-26
> Scope: Full `apps/core/src/` rewrite (delete existing, start clean)

---

## All Decisions Locked

| Topic | Decision |
|---|---|
| Migration strategy | Delete `src/`, start clean |
| DB migrations | Add `002_full_rewrite.sql` |
| Auth | Supabase JWKS (RS256 public key). No secrets in Core config. |
| First-run | Pairing code printed to terminal → `POST /setup` |
| Typestate | Removed. `InstanceStatus` enum instead. |
| Instance creation | Async (202 + progress via events) |
| `theseus/` silo | Dissolved entirely (see dissolution map below) |
| Macros | In scope. Lodestone pattern with `#[op2]` + `extension!()` |
| Java | Version registry (17/21/etc → path); detected on startup; auto-install is TODO |
| Port allocation | Auto-picked; user can override in advanced settings |
| WS auth | Short-lived ticket (`POST /ws-token` → 60s UUID, in-memory) |
| Core auto-restart | Yes — restore `Running` instances on startup |
| Spawner | `PtySpawner` only (fixed). `MockSpawner` for tests. |
| Graceful stop | `stop\n` → 30s timeout → SIGKILL |
| CORS | `amberite.dev` + `localhost` (restricted) |

---

## Theseus Dissolution Map

Delete the entire `src/theseus/` directory. Distribute salvageable pieces:

| Current File | Keep? | New Home | Notes |
|---|---|---|---|
| `theseus/pack.rs` types (`PackFormat`, `PackFile`, `PackFileHash`, `EnvType`, `SideType`, `PackDependency`) | ✅ | `domain/modpack.rs` | Pure domain types |
| `theseus/pack.rs` functions (`install_mrpack`, `extract_metadata`, `sha1_hash`) | ✅ | `infrastructure/minecraft/mrpack.rs` | Async I/O, reqwest |
| `theseus/profile.rs` → `ModLoader`, `MemorySettings` | ✅ | `domain/instance.rs` | Domain types |
| `theseus/profile.rs` → `LinkedData` | ✅ | `domain/modpack.rs` | Modrinth link concept |
| `theseus/profile.rs` → `Profile`, `ProfileInstallStage`, `DirectoryInfo` | ❌ DELETE | — | Client-side concepts. Core uses DB rows. |
| `theseus/state.rs` (`State`, `THESEUS_STATE: OnceCell`, second pool) | ❌ DELETE | — | Replaced by `AppState` |
| `theseus/error.rs` (`TheseusError`) | ✅ renamed | `infrastructure/minecraft/mrpack.rs` → `MrpackError` | Inline |
| `theseus/util.rs::io` | ❌ DELETE | — | Thin wrappers. Use `tokio::fs` directly. |
| `theseus/util.rs::fetch::download_with_sha1` | ✅ | Inline into `mrpack.rs` | No separate module |
| `theseus/mod.rs` | ❌ DELETE | — | — |

---

## Heroic File Structure (46 files, 12 directories)

```
apps/core/src/
├── main.rs                              # Entry: wires layers, Axum on :16662
├── config.rs                            # Config { data_dir, supabase_url, port }
│
├── domain/                              # Pure types — zero external deps
│   ├── mod.rs
│   ├── instance.rs                      # InstanceId, InstanceRecord, InstanceStatus, ModLoader, MemorySettings
│   ├── modpack.rs                       # PackFormat, PackFile, PackFileHash, EnvType, SideType, PackDependency, LinkedData
│   └── event.rs                         # Event enum (InstanceOutput, StatusChange, MacroOutput)
│
├── ports/                               # Trait definitions
│   ├── mod.rs
│   ├── instance_store.rs                # InstanceStore trait (CRUD)
│   ├── modpack_store.rs                 # ModpackStore trait
│   └── process_spawner.rs               # ProcessSpawner + ProcessHandle traits
│
├── application/                         # Orchestration
│   ├── mod.rs
│   ├── state.rs                         # AppState { pool, http, config, spawner, instances: DashMap, broadcaster, macro_executor, jwks_cache }
│   ├── instance_service.rs              # create, start, stop, kill, send_command, restore_on_startup
│   ├── modpack_service.rs               # install_mrpack, list_mods, sync_modpack
│   └── macro_service.rs                 # spawn_macro, kill_macro, list_tasks
│
├── infrastructure/
│   ├── mod.rs
│   ├── db/
│   │   ├── mod.rs
│   │   ├── instance_repo.rs             # SqlitePool → InstanceStore impl
│   │   └── modpack_repo.rs              # SqlitePool → ModpackStore impl
│   │
│   ├── process/
│   │   ├── mod.rs
│   │   ├── pty_spawner.rs               # PtyProcess spawner (kill/is_running FIXED)
│   │   ├── mock_spawner.rs              # MockSpawner for tests
│   │   └── instance_actor.rs            # Per-instance actor: health loop, stdin/stdout channels
│   │
│   ├── minecraft/
│   │   ├── mod.rs
│   │   ├── mrpack.rs                    # install_mrpack, extract_metadata
│   │   ├── modrinth_api.rs              # Modrinth HTTP client
│   │   ├── flavours.rs                  # Paper/Vanilla/Fabric/Forge/NeoForge URL + SHA1 (FIXED)
│   │   ├── server_jar.rs                # Download + verify server.jar
│   │   ├── java.rs                      # Java version registry; TODO: auto-install
│   │   └── server_properties.rs         # server.properties read/write
│   │
│   ├── macro_engine/
│   │   ├── mod.rs
│   │   ├── executor.rs                  # MacroExecutor: DashMap<MacroPID, IsolateHandle>
│   │   ├── loader.rs                    # TypescriptModuleLoader (deno_ast)
│   │   └── ops/
│   │       ├── mod.rs
│   │       ├── prelude.rs               # get_version op
│   │       ├── events.rs                # next_instance_output, next_state_change ops
│   │       └── instance_control.rs      # send_command, get_state, start/stop ops
│   │
│   ├── auth/
│   │   ├── mod.rs
│   │   └── jwks.rs                      # Fetch + cache Supabase JWKS; validate RS256 JWT
│   │
│   └── events.rs                        # EventBroadcaster (tokio::broadcast wrapper)
│
└── presentation/
    ├── mod.rs
    ├── router.rs                        # Axum router, CORS, middleware
    ├── error.rs                         # ApiError + IntoResponse
    ├── extractors.rs                    # AuthUser (JWKS), WsTicket extractor
    └── handlers/
        ├── mod.rs
        ├── setup.rs                     # POST /setup (first-run pairing)
        ├── auth.rs                      # POST /ws-token
        ├── instances.rs                 # GET /instances, POST /instances, GET/DELETE /instances/:id
        ├── instance_control.rs          # POST /instances/:id/{start,stop,kill,command}
        ├── modpack.rs                   # POST/GET/DELETE /instances/:id/modpack
        ├── macros.rs                    # GET/POST/DELETE /instances/:id/macros[/:pid]
        ├── console.rs                   # GET /instances/:id/console (WebSocket)
        └── diagnostics.rs               # GET /health, GET /stats
```

Each directory gets an `AGENTS.md` explaining its contents.

---

## Database Schema (`002_full_rewrite.sql`)

```sql
-- Java version registry
CREATE TABLE java_installations (
    version   INTEGER PRIMARY KEY,   -- 17, 21, etc.
    path      TEXT    NOT NULL
);

-- Server instances
CREATE TABLE instances (
    id              TEXT    PRIMARY KEY,
    name            TEXT    NOT NULL,
    game_version    TEXT    NOT NULL,
    loader          TEXT    NOT NULL,                  -- vanilla|fabric|forge|neoforge|quilt
    loader_version  TEXT,
    port            INTEGER NOT NULL,
    memory_min      INTEGER NOT NULL DEFAULT 512,
    memory_max      INTEGER NOT NULL DEFAULT 4096,
    java_version    INTEGER REFERENCES java_installations(version),
    status          TEXT    NOT NULL DEFAULT 'offline', -- offline|starting|running|stopping|crashed
    data_dir        TEXT    NOT NULL,
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL
);

-- Modpack manifest (one per instance)
CREATE TABLE modpack_manifests (
    id                    TEXT PRIMARY KEY,
    instance_id           TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    pack_name             TEXT NOT NULL,
    pack_version          TEXT NOT NULL,
    game_version          TEXT NOT NULL,
    loader                TEXT NOT NULL,
    loader_version        TEXT,
    modrinth_project_id   TEXT,
    modrinth_version_id   TEXT,
    installed_at          TEXT NOT NULL,
    UNIQUE(instance_id)
);

-- Core pairing (single row enforced)
CREATE TABLE core_config (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    supabase_url    TEXT NOT NULL,
    owner_user_id   TEXT NOT NULL,
    paired_at       TEXT NOT NULL
);
```

---

## Key Flows

### First-Run Pairing

```
1. Core starts → no core_config row found
2. Generate 6-digit pairing code → print: "Pairing code: 482910"
3. Owner opens App/Web → POST /setup { code, supabase_url } + JWT
4. Core verifies code → fetches JWKS → validates JWT
5. Stores (supabase_url, owner_user_id) → core_config
6. All future JWTs validated against this Supabase project's JWKS
```

### Auth Per Request

```
Authorization: Bearer <Supabase JWT>
→ AuthUser extractor → cached JWKS → validate RS256 → inject user_id + role
```

### WebSocket Auth (short-lived ticket)

```
1. POST /ws-token (with JWT) → returns { ticket: UUID }
2. GET /instances/:id/console?ticket=<uuid>
3. Validate: exists + not expired + single-use → upgrade to WS
```

### Instance Lifecycle

```
POST /instances → 202 Accepted + SSE progress stream
  → create DB row (status=offline)
  → spawn task: download server jar → emit progress
  → return 200 when ready

POST /instances/:id/start
  → actor: spawn PtyProcess → status=starting → wait for "Done" → status=running

POST /instances/:id/stop
  → actor: send "stop\n" → 30s timeout:
    → exits: status=offline, remove from DashMap
    → timeout: SIGKILL → status=offline

Core restart → query WHERE status='running' → restore each
```

### Macro Execution (Lodestone pattern)

```
POST /instances/:id/macros { name: "backup.ts" }
  → MacroExecutor::spawn(instance_id, macro_path)
  → std::thread::spawn (JsRuntime !Send)
    → LocalSet → JsRuntime with 3 extensions (prelude, events, instance_control)
    → inject __macro_pid + __instance_uuid globals
    → Arc<AppState> in op state (not global singleton)
  → return { pid }

DELETE /instances/:id/macros/tasks/:pid
  → lookup IsolateHandle → terminate_execution()
```

---

## Cargo.toml Changes

**Remove:**
- `pasetors`
- `bollard`
- `utoipa`, `utoipa-axum`
- playit.gg git deps

**Add:**
- `jsonwebtoken` (RS256 JWT validation)
- `base64` (JWKS key decoding)

**Keep pinned:**
- All `deno_*` crates at current versions (0.354/0.220/0.226/0.42)

---

## Implementation Order

1. **Foundation**
   - `config.rs`, `main.rs` skeleton
   - `domain/` all files (pure types, no deps)
   - `ports/` all trait definitions

2. **Infrastructure core**
   - `infrastructure/db/` (repositories)
   - `infrastructure/events.rs` (EventBroadcaster)
   - `infrastructure/auth/jwks.rs` (JWKS fetch + validate)
   - `infrastructure/process/` (spawners + actor)

3. **Application**
   - `application/state.rs` (AppState)
   - `application/instance_service.rs`
   - `application/modpack_service.rs`

4. **Presentation**
   - `presentation/error.rs`, `extractors.rs`
   - `presentation/router.rs`
   - `presentation/handlers/setup.rs`, `auth.rs`, `instances.rs`, `instance_control.rs`

5. **Minecraft**
   - `infrastructure/minecraft/mrpack.rs`
   - `infrastructure/minecraft/flavours.rs` (FIXED URLs + SHA1)
   - `infrastructure/minecraft/server_jar.rs`
   - `infrastructure/minecraft/java.rs`

6. **Macros**
   - `infrastructure/macro_engine/executor.rs`
   - `infrastructure/macro_engine/loader.rs`
   - `infrastructure/macro_engine/ops/` (prelude, events, instance_control)
   - `application/macro_service.rs`
   - `presentation/handlers/macros.rs`

7. **WebSocket**
   - `presentation/handlers/console.rs`

---

## TODOs (deferred, not in this rewrite)

- Java auto-download (like Theseus) — data model supports it, install logic is TODO
- `PATCH /instances/:id/properties` — full server.properties edit API
- Friend groups (Supabase tables: `friend_groups`, `group_members`, `group_invites`)
- Playit.gg tunnel (stub only)
- Full port allocation API with conflict detection
- Companion mod integration

---

## Reference Files

- `.plan/lodestone-ref/core/src/` — Lodestone source (macro pattern, instance_control ops)
- `.plan/mrpack-rust-file.md` — Modrinth API surface
- `PROJECT.md` — Canonical project vision
- `.plan/active/decisions.md` — Architectural decisions (update after implementation)