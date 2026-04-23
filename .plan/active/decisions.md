# Architectural Decisions

**Last updated:** 2026-04-19

---

## Testing Approach
- **Core:** Use `axum-test` crate for endpoint testing (compiles + tests all 9 endpoints)
- **Frontend:** Tauri invoke test script (separate file, runs outside CI, reports problems for agent review)
- **CI:** Not implementing now — user will learn GitHub Actions when ready

---

## Dependency Strategy
- **Modrinth packages:** User prefers single style (remove catalog, use `workspace:*` only)
- **Fork strategy:** Stay on latest Modrinth release, periodic merges from modrinth/code
- **Controlled reference preferred** over local copy modification

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

## Supabase Auth
- **Wait for Supabase MCP:** User will add MCP server, then plan auth fix
- **Current approach:** Anon key for JWT validation (documented as risky)
- **Future:** Service role key or proper service-to-service auth

---

## Memory System
- **feature-memory skill:** Tracks session info, writes at end or on request
- **Files:** `.plan/active/` (current), `.plan/archive/` (history), `.plan/completed/` (done)
- **Agent prompts:** Baked into plan/build agent system prompts