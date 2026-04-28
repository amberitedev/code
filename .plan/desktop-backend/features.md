# Desktop Backend Features

All features that must be implemented in `amberite-backend` and the Tauri shell.
Last updated: 2026-04-27

---

## Backend Modules (`apps/app/backend/src/`)

| Module | Status | Description |
|--------|--------|-------------|
| `lib.rs` | placeholder | Re-exports, `AmberiteState` struct. Fix dual-error inconsistency. |
| `error.rs` | placeholder | Unified `AmberiteError` — replace current two-enum mess. |
| `auth.rs` | planned | Microsoft Xbox token → Supabase Edge Function → Supabase JWT → OS keychain |
| `core_launcher.rs` | planned | Download Core binary from GitHub Releases, spawn as background child process, register with OS startup, kill/restart |
| `core_client.rs` | planned | `reqwest` HTTP client for all Core REST endpoints; attaches Bearer JWT to every request |
| `console_stream.rs` | planned | WebSocket client → Core `/instances/:id/console` (gets ticket first via `/ws-token`) → emits Tauri `console-line` events |
| `progress_stream.rs` | planned | SSE client → Core `/instances/:id/progress` → emits Tauri `instance-progress` events |
| `pairing.rs` | planned | Localhost auto-pair on Core first start; remote: accept pairing code from UI → POST Core `/setup`; stores Core URL + paired flag in settings |
| `settings.rs` | planned | Persist app config to `{AppData}/amberite/settings.json`: Core URL, auth state, tunnel config, startup behavior |
| `tunnel.rs` | planned | Bundle + spawn Playit.gg agent; call Cloudflare DNS API to create `{name}.amberite.dev` CNAME; manage tunnel lifecycle (V2) |
| `groups.rs` | planned | Supabase REST calls for friend groups: create, list, get members, update member permissions |
| `friends.rs` | planned | Supabase REST calls for friends: add, remove, block, list; friend codes and username lookup |
| `mod_sync.rs` | planned | Export Theseus profile as `.mrpack` → POST to Core; subscribe to Supabase Realtime for sync events → emit Tauri events |
| `supabase_client.rs` | planned | Base Supabase HTTP + Realtime client (reused by groups, friends, mod_sync) |

---

## Tauri Commands (`plugin:amberite|*`)

All go in `apps/app/tauri/src/amberite.rs`.

### Core Process Management
- `hello` — **exists** (placeholder)
- `get_core_status` — is Core binary installed? is it running?
- `install_core` — download Core binary from GitHub Releases
- `start_core` — spawn Core process as background child
- `stop_core` — gracefully kill Core process
- `get_core_config` — return current Core URL, paired state

### Core Pairing
- `pair_with_code { code, core_url }` — POST Core `/setup`, save owner state
- `pair_local_auto` — first-run shortcut: auto-pair with localhost Core

### Instance Management (mirrors Core REST)
- `get_instances`
- `create_instance { name, game_version, loader, port, memory_min, memory_max }`
- `get_instance { id }`
- `delete_instance { id }`
- `start_instance { id }`
- `stop_instance { id }`
- `kill_instance { id }`
- `restart_instance { id }`
- `send_command { id, command }`

### Modpacks
- `install_modpack { instance_id, mrpack_path }`
- `get_modpack { instance_id }`
- `remove_modpack { instance_id }`

### Macros
- `list_macros { instance_id }`
- `create_macro { instance_id, name, script }`
- `delete_macro { instance_id, macro_id }`

### Console & Progress Streaming
- `start_console_stream { instance_id }` — opens WS, begins emitting `console-line` Tauri events
- `stop_console_stream { instance_id }` — closes WS connection

### Diagnostics
- `core_health` — GET Core `/health`
- `core_version` — GET Core `/version`
- `get_java_installations` — GET Core `/java`
- `core_metrics { instance_id }` — GET Core `/instances/:id/metrics` *(new Core endpoint needed)*

### Auth
- `login_microsoft` — trigger Theseus Microsoft/Xbox OAuth → call `microsoft-auth` Edge Function → store Supabase JWT
- `logout` — clear Supabase JWT from OS keychain
- `get_current_user` — return Supabase user profile

### Friends
- `get_friends`
- `add_friend { username_or_code }`
- `remove_friend { user_id }`
- `block_user { user_id }`
- `lookup_user { username_or_code }` — resolve username or friend code to UID

### Friend Groups
- `get_groups` — list all groups the current user is a member of
- `create_group { name }` — create a new group
- `get_group_members { group_id }`
- `update_member_role { group_id, user_id, role, permissions }`
- `create_invite_link { group_id }` — generate 24h invite token
- `join_via_invite { token }` — validate token → join group with default permissions

### Mod Sync
- `push_modpack_to_core { instance_id, profile_id }` — export Theseus profile as `.mrpack` → POST to Core
- `subscribe_to_sync_events { group_id }` — open Supabase Realtime subscription → emit `mod-sync-update` Tauri events

### Tunnel (V2)
- `setup_tunnel { server_name }` — provision Playit.gg + Cloudflare DNS CNAME
- `get_tunnel_status` — is tunnel running? what is the URL?
- `stop_tunnel`

### Settings
- `get_app_settings`
- `save_app_settings { settings }`

---

## Tauri Events (backend → frontend push)

Frontend listens with `listen('event-name', handler)` from `@tauri-apps/api/event`.

| Event | Payload | Trigger |
|-------|---------|---------|
| `core-status-changed` | `{ status: "running" \| "stopped" \| "crashed" }` | Core child process state change |
| `console-line` | `{ instance_id, line, timestamp }` | WebSocket message from Core |
| `instance-progress` | `{ instance_id, stage, percent }` | SSE from Core during instance creation |
| `mod-sync-update` | `{ group_id, instance_id, event_type, payload }` | Supabase Realtime |
| `friend-request` | `{ from_user_id, username }` | Supabase Realtime |
| `group-invite` | `{ from_user_id, group_id, group_name }` | Supabase Realtime |
| `tunnel-status` | `{ status: "connected" \| "disconnected", url? }` | Playit.gg agent stdout parsing |

---

## Frontend Changes (`apps/app/frontend/src/`)

### New Pages

| File | Purpose |
|------|---------|
| `pages/Onboarding.vue` | Welcome → Microsoft login → Core setup wizard (install local or paste remote pairing code) |
| `pages/ServerDashboard.vue` | Console log + send command, start/stop/restart controls, player list, TPS, CPU/memory/disk usage, mod sync status |
| `pages/FriendGroups.vue` | Group list, member list with roles, invite link generator, pending friend requests |

### Modified Pages

| File | Change |
|------|--------|
| `pages/library/index.vue` | Add filter chips: `All` / `Client` / `Server`. Server instance cards get status badge + quick-action row (start/stop/console). |
| `pages/Servers.vue` | Replace Modrinth billing UI with "My Servers" list (connected Core instances). Keep Modrinth community servers as a second tab — do not remove. |
| `pages/Settings.vue` | Add Amberite section: account info, Core URL, tunnel config, startup behavior preference. |

### New Helpers

| File | Purpose |
|------|---------|
| `helpers/amberite_auth.ts` | `login()`, `logout()`, `getCurrentUser()` — wraps `plugin:amberite|*` auth commands |
| `helpers/core.ts` | All Core management + instance commands — wraps `plugin:amberite|*` |
| `helpers/groups.ts` | Friend groups + invite helpers |
| `helpers/mod_sync.ts` | Push modpack, subscribe to sync events |
| `helpers/tunnel.ts` | Setup / status / stop tunnel (V2) |

### New Composables

| File | Purpose |
|------|---------|
| `composables/useCoreConsole.ts` | Reactive console log, like `useInstanceConsole` but driven by `console-line` Tauri events |
| `composables/useCoreStatus.ts` | Reactive Core connection state (running/stopped/connecting), auto-reconnect logic |

---

## Supabase Changes (`apps/supabase/`)

### New Tables

| Table | Key Columns | Notes |
|-------|------------|-------|
| `friends` | `user_a_id`, `user_b_id`, `created_at`, `blocked_by` | Alphabetically sorted pair to prevent duplicate rows |
| `friend_groups` | `id`, `name`, `owner_user_id`, `created_at` | One group per server/core |
| `group_members` | `id`, `group_id`, `user_id`, `role`, `permissions_json`, `joined_at` | Default role: view-only |
| `group_invites` | `id`, `group_id`, `inviter_user_id`, `token`, `expires_at`, `used_by`, `used_at` | 24h single-use tokens |
| `mod_sync_events` | `id`, `group_id`, `instance_id`, `event_type`, `payload_json`, `created_at` | Realtime sync event log |
| `core_registrations` | `id`, `user_id`, `tunnel_url`, `display_name`, `created_at` | Supabase as relay for remote Core discovery |

### New Edge Functions

| Function | Purpose |
|----------|---------|
| `microsoft-auth` | Accept Microsoft ID token → verify → create or link Supabase user → return session |
| `create-invite-link` | Generate 24h UUID token for group invite |
| `join-group` | Validate invite token, add user to `group_members` with default permissions |

---

## Core Additions Needed (`apps/core/`)

The following are NOT yet in Core and must be added:

1. **`GET /instances/:id/metrics`** — return `{ cpu_percent, memory_mb, disk_mb, tps, uptime_seconds, player_count }`
   - CPU/memory/disk via OS process stats on the Java PID
   - TPS parsed from PTY stdout ("Can't keep up!" lines or `/tps` RCON)
   - Player count parsed from PTY stdout (join/leave events)

2. **Supabase Realtime notification on modpack change** — after successful modpack install,
   Core inserts a row into `mod_sync_events` via Supabase REST API so group members get notified

---

## V1 Scope (get it working)

- Microsoft login → Supabase session (via Edge Function)
- Local Core install + auto-start as background process
- Localhost auto-pair + remote pairing via code
- Instance CRUD + start/stop/kill
- Console streaming (WebSocket → Tauri events)
- Merged Library page (client + server instances with filter chips)
- Onboarding wizard (Welcome → Microsoft → Core setup)
- Basic mod sync: owner pushes modpack to Core, friends download it

## V2 Scope (make it great)

- Granular Discord-like permissions per group member
- Vote-to-add-mod feature
- Playit.gg tunnel + `{name}.amberite.dev` DNS branding
- Full monitoring dashboard (TPS, CPU, memory, disk)
- Multi-account switcher
- Century (AI log/crash explainer)
- Companion Mod (runtime config injection)
- Peer-to-peer failover
