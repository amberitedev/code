# Architectural Decisions

**Last updated:** 2026-04-26

---

## Testing Approach
- **Core:** Use `axum-test` crate for endpoint testing (compiles + tests all 9 endpoints)
- **Frontend:** Tauri invoke test script (separate file, runs outside CI, reports problems for agent review)
- **CI:** GitHub Actions for theseus sync; full CI/CD deferred

---

## Dependency Strategy
- **Modrinth packages:** User prefers single style (remove catalog, use `workspace:*` only)
- **Fork strategy:** Vendor `packages/app-lib/` directly into repo; leave `daedalus` and `path-util` as git deps from modrinth/code
- **Cargo `[patch]`:** Add `[patch."https://github.com/modrinth/code"] theseus = { path = "packages/app-lib" }` to root Cargo.toml — transparent to contributors (zero extra steps)
- **Controlled reference preferred** over local copy modification
- **All AMBERITE PATCH lines** marked with `// AMBERITE PATCH` comments for easy reapplication after upstream sync

---

## Vendor Scope (Milestone 1)
- **Vendor `packages/app-lib/` only** — this is where the namespace patch lives
- **Leave `daedalus` and `path-util` as git deps** from `modrinth/code` at `v0.13.4` — no Amberite changes needed in those crates
- **Source:** Copy from `modrinth/code` tag `v0.13.4`

---

## Theseus Namespace Patch (Milestone 1)
- **Keychain namespace:** Change `"com.modrinth.theseus"` → `"com.amberite.app"` in `packages/app-lib/src/state/legacy_converter.rs` `default_settings_dir()`
- **User-Agent header:** Change `"modrinth/theseus/"` → `"amberite/app/"` in `packages/app-lib/src/lib.rs` `launcher_user_agent()`
- **Credential import from Modrinth App:** **Skip for now** — requires cross-app SQLite read from `%APPDATA%\ModrinthApp\app.db`, blocked by macOS sandboxing and file locking concerns. Users sign into Microsoft fresh in Amberite. Future task.
- **App identifier:** Already `"Amberite"` (separate from Modrinth's `"ModrinthApp"`) — no change needed. Data dirs already separate.

---

## Backend Wiring (Milestone 1)

### Plugin Architecture
- **Namespace:** `plugin:amberite|` — frontend calls `invoke('plugin:amberite|hello')`
- **Error type:** Separate `AmberiteCommandError` in `api/amberite.rs` — does NOT touch `TheseusSerializableError` in `api/mod.rs`. This avoids merge conflicts.
  ```rust
  #[derive(Error, Debug, Serialize, Clone)]
  pub enum AmberiteCommandError {
      #[error("{0}")]
      Amberite(String),
  }
  ```
- **Command:** `hello()` returns `Result<String, AmberiteCommandError>`, calls `amberite_backend::get_placeholder()?.message`
- **Registration:** Add `pub mod amberite;` to `api/mod.rs` + `.plugin(api::amberite::init())` to `main.rs`
- **Test message:** Change `get_placeholder()` return from `"Amberite backend initialized"` to `"hello from Amberite"`. Update test assertion to match.

### Dependency Cleanup
- **`apps/app/backend/Cargo.toml`:** Convert all deps to `workspace = true` (tauri, serde, serde_json, thiserror, tokio, tracing)
- **`apps/app/tauri/Cargo.toml`:** Add `amberite-backend = { workspace = true }`
- **Root `Cargo.toml`:** Add `amberite-backend = { path = "apps/app/backend" }` to `[workspace.dependencies]`

---

## Product Boundaries
- **Core:** Independent backend, runs standalone or launched by desktop app
  - Windows + Linux support
  - Port-forwarded when app launches local core
  - Can run on NAS, cloud, or local machine
- **Web:** Dashboard + marketing + docs (lower priority than desktop app)
- **CLI:** Late-stage feature, Linux-focused installer/runner/updater

---

## Sync Patterns (from Cross-Industry Research)

### Client-to-Client Mod Sync
- **Binary files (.jar):** Use Syncthing's version vector pattern — track `{DeviceID, Counter}` per file
- **Config files (JSON/TOML):** Use Automerge/Yjs CRDT pattern — automatic merge, no lost changes
- **Manifest format:** TOML with `side: both/client/server` metadata (Packwiz pattern)
- **Resolution:** Hash-based (SHA1) via Modrinth API

### Core-to-Client Config Injection
- **Transport:** Nacos-style long-polling (blocking queries) for reliable push
- **Hot reload:** Companion Mod watches local files (Java NIO WatchService)
- **Validation:** MD5 checksum before applying
- **Rollout:** Apollo grayscale pattern — test on subset of players first

---

## Coordination Patterns

### Server Instance Management
- **State machine:** `Starting → Running → Stopping → Offline → Crashed`
- **Graceful stop:** Send `stop` command, wait 30s, then SIGKILL
- **Crash detection:** Consul-style health check + heartbeat timeout

### P2P Failover
- **Leader election:** Redis Redlock pattern — quorum-based lock acquisition
- **Lock-delay:** 15s delay after failover (Consul pattern) — prevents split-brain
- **Session-based:** Lock tied to health check/TTL, auto-releases on failure

---

## Social System (Friend Groups)

### Database Schema (from Mattermost + Keycloak)
```sql
-- Friend groups (teams/realms pattern)
friend_groups (id, owner_id, core_id, name)

-- Members with roles
group_members (group_id, user_id, role, permissions)

-- Invite codes
group_invites (code, group_id, uses_max, expires_at)
```

### Permission Model
- **Simple roles:** Owner, Admin, Member, Guest (GitLab pattern)
- **Bitfield permissions optional:** 0x1=view, 0x2=start, 0x4=invite, 0x8=console

---

## Century (Log/Crash AI Explainer)

### Architecture (from Sentry + Graylog)
1. **Parsing:** Graylog pipeline — regex/grok patterns for Minecraft logs
2. **Fingerprinting:** Sentry pattern — group similar crashes by error class + message
3. **Analysis:** Sentry Seer AI — LLM with context aggregation for root cause
4. **Storage:** Loki pattern — label-based indexing (server, player, mod as labels)

### Training Data Strategy
- Collect crash reports from users
- Manual clustering for known Minecraft errors
- Build pattern database (mcla-style) for common issues
- LLM for unknown/unusual crashes

---

## Dashboard Architecture (from Uptime Kuma + Grafana)

### Tech Stack
- **Vue 3 + Vite** (matches Amberite)
- **Socket.IO** for real-time (Uptime Kuma pattern)
- **Axum WebSocket** from Core backend

### Panel Components
1. Server status (live health, player count)
2. Log viewer (streaming with filtering)
3. Resource monitor (CPU/memory charts)
4. Mod grid (sync status, controls)
5. Activity timeline (events, joins, updates)

---

## Desktop App Patterns (from DocKit + RustDesk)

### Architecture
- **Tauri + Vue 3** — validated stack (DocKit, ServerMint, GlobalProtect)
- **Rust backend** — same pattern as RustDesk
- **Background service** — GlobalProtect pattern for persistent Core connection

### Remote Connection
- HTTP REST to Core (primary)
- WebSocket for console streaming
- System tray for background management

---

## Dev Endpoint Switching (Milestone 1)
- **Dev mode uses Modrinth staging:** `staging-api.modrinth.com` for all API URLs
- **Files to update:** `apps/app/.env` and `apps/app/.cargo/config.toml`
- **CSP in tauri.conf.json:** No change needed — `https://*.modrinth.com` wildcard covers staging subdomain
- **Production:** Revert to `api.modrinth.com` URLs (or use a separate `.env.production` in the future)

---

## Directory Restructure (Milestone 1)

### Target Structure
```
apps/app/
├── frontend/           # Vue 3 UI (was apps/app-frontend/)
│   ├── src/
│   ├── vite.config.ts
│   ├── package.json
│   └── .env            # Already has staging URLs + VITE_AMBERITE_API_URL
├── backend/            # Already in place
│   ├── src/lib.rs, error.rs
│   └── Cargo.toml
├── tauri/              # Tauri shell (was at apps/app/ root level)
│   ├── src/            # main.rs, api/, macos/
│   ├── capabilities/
│   ├── icons/
│   ├── nsis/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── tauri-release.conf.json
│   ├── tauri.linux.conf.json
│   ├── tauri.macos.conf.json
│   ├── App.entitlements
│   ├── Info.plist
│   ├── COPYING.md
│   └── LICENSE
├── .cargo/             # Build config (stays — Cargo walks up to find it)
├── .env                # Staging URLs
├── package.json        # @amberite/app workspace scripts
└── AGENTS.md
```

### Files that stay at `apps/app/` level
`.cargo/`, `.env`, `package.json`, `AGENTS.md`, `.gitignore`, `.prettierignore`, `gen/` (build artifact)

---

## Version Synchronization (Milestone 1)
- **Single source of truth:** `/MODRINTH_VERSION` at repo root
- **Sync targets:**
  - `apps/app/MODRINTH_VERSION.ts` (currently stale at `v0.13.3`)
  - `pnpm-workspace.yaml` catalog entries (currently stale at `v0.13.1`)
- **GitHub Actions workflow:** `.github/workflows/sync-theseus.yml`
  - Runs on schedule (weekly) + manual trigger
  - Checks modrinth/code for new tags
  - Opens PR with updated vendored copy + re-applied `// AMBERITE PATCH` lines
  - Uses `// AMBERITE PATCH` markers to detect and warn about conflicts

---

## Desktop Backend Architecture (2026-04-27)

Full detail in `.plan/desktop-backend/decisions.md`. Summary:

### Auth
- **Microsoft → Supabase via Edge Function:** Theseus Xbox token → `microsoft-auth`
  Edge Function → Supabase JWT stored in OS keychain. One login, no password.
- **Web OAuth (Google/Discord/GitHub):** Web dashboard only, not in desktop app.

### Core Connection
- **Local + Remote supported.** Most users will run Core on localhost.
- **Pairing via one-time code** printed by Core on first startup. Localhost auto-pairs
  silently. Remote requires user to paste URL + code.
- **Owner status is permanent** and stored in Supabase.

### Library Page
- **Merged client + server library** with filter chips (`All`, `Client`, `Server`).
  Unified view reinforces Amberite's core proposition (same mods, everywhere).
- Modrinth community servers stay as a separate tab.

### Tunneling (V2)
- **Playit.gg** for raw TCP tunneling (Minecraft). **Cloudflare DNS API** to provision
  `{servername}.amberite.dev` CNAME → Playit.gg address.
- Cloudflare Tunnels rejected: HTTP-only on free tier. Minecraft needs raw TCP.
- **V1 ships without tunnel.** Manual port forwarding in V1.

### Mod Sync
- **Hybrid:** Supabase Realtime events per change + full `.mrpack` snapshot every ~10 changes.
- Owner clicks "Push to Core" → export `.mrpack` → POST to Core → Core notifies Supabase.
- Friends receive Realtime event, see "Update available" badge, download pack from Core.

### Core Data Directory
- Core data (`{AppData}/amberite-core/`) is **separate from app data**.
  Uninstalling the app NEVER deletes server worlds or configs.

### Onboarding
- **Linear:** Welcome → Microsoft login → Core setup (local/remote/skip) → Main app.

### Multiaccounts
- **V1:** Single account. **V2:** Multi-account switcher.

---

## Supabase Auth
- **Wait for Supabase MCP:** User will add MCP server, then plan auth fix
- **Current approach:** Anon key for JWT validation (documented as risky)
- **Future:** Service role key or proper service-to-service auth

---

## Memory System
- **feature-memory skill:** Tracks session info, writes at end or on request
- **Files:** `.plan/active/` (current), `.plan/archive/` (history), `.plan/completed/` (done)
- **Agent prompts:** Baked into plan/build agent system prompts

---

## Core Rewrite (2026-04-26)

### Migration Strategy
- **Delete `apps/core/src/` entirely** — start clean with heroic file structure
- **DB migration:** Add `002_full_rewrite.sql` (drop + recreate all tables)
- **Keep `001_init.sql`** as historical record only

### Auth Model
- **Supabase JWKS (RS256)** — Core fetches public key from `.well-known/jwks.json`
- **No secrets in Core config** — fully open-source safe
- **First-run pairing:** 6-digit code printed to terminal → `POST /setup` with Supabase JWT
- **WebSocket auth:** Short-lived ticket (`POST /ws-token` → 60s UUID, in-memory, single-use)

### Typestate Removal
- **Remove `GameInstance<Stopped/Running>`** — conflicts with `DashMap`
- **Replace with `InstanceStatus` enum:** `Offline|Starting|Running|Stopping|Crashed`
- **Single `AppState`:** replaces `ServiceRegistry` + `TheseusState`

### Theseus Dissolution
- **Delete entire `src/theseus/` directory**
- **Salvage:** `PackFormat` types → `domain/modpack.rs`; `install_mrpack` → `infrastructure/minecraft/mrpack.rs`
- **Delete:** `Profile`, `ProfileInstallStage`, `DirectoryInfo` (client concepts); `State` + `OnceCell` (replaced by AppState)

### Instance Lifecycle
- **Creation:** Async (202 + SSE progress events)
- **Start:** Spawn PtyProcess → wait for "Done" → status=running
- **Stop:** `stop\n` → 30s timeout → SIGKILL
- **Auto-restart:** Restore `Running` instances on Core startup

### Macros (Lodestone Pattern)
- **In scope for rewrite**
- **`#[op2]` + `deno_core::extension!()`** — not old `#[op]` + `Extension::builder()`
- **`Arc<AppState>` in op state** — not global singleton
- **Std::thread::spawn + LocalSet** — JsRuntime is `!Send`
- **Inject globals:** `__macro_pid`, `__instance_uuid`

### Java Handling
- **Version registry:** `java_installations` table (version → path)
- **Detect on startup:** Scan PATH + common install dirs
- **Auto-install:** TODO (data model ready, logic deferred)

### Cargo.toml
- **Remove:** `pasetors`, `bollard`, `utoipa`, playit.gg git deps
- **Add:** `jsonwebtoken`, `base64`
- **Keep pinned:** all `deno_*` at 0.354/0.220/0.226/0.42

### CORS
- **Restricted:** `amberite.dev` + `localhost` only