# Lodestone — Build Steps

## Phase 1: Foundation (Zero Dependencies)

- [ ] 1.1 Write `helpers/api/types.ts` — TypeScript types for all Core API shapes (Instance, ModpackConfig, Notification, Friend, Group, Presence, ApiKey, errors)
- [ ] 1.2 Write `helpers/system/events.ts` — Global event bus (auth-failed, connection-lost, connection-restored, notification-received, sync-complete)
- [ ] 1.3 Write `helpers/auth/key-store.ts` — API key storage (localStorage under LODESTONE_API_KEY, getKey/setKey/clearKey/hasKey)
- [ ] 1.4 Write AGENTS.md files for api/, auth/, system/ subfolders

## Phase 2: Transport Layer

- [ ] 2.1 Write `helpers/api/client.ts` — ofetch wrapper with API key interceptor (onRequest attaches Bearer header, onResponseError catches 401 → auth-failed, network errors → connection-lost)
- [ ] 2.2 Write `helpers/api/websocket.ts` — WebSocket manager (auto-reconnect with exponential backoff, API key as query param, returns { send, close, state: Ref })
- [ ] 2.3 Write `helpers/settings/app-settings.ts` — localStorage for core_url, theme, notification prefs
- [ ] 2.4 Write AGENTS.md for settings/ subfolder

## Phase 3: Connection + Auth

- [ ] 3.1 Write `helpers/auth/connection.ts` — Connect to Core (parse connection string ms_host:port:key, store key + URL, call GET /health to verify reachability, call POST /api/auth/connect to validate key, return permissions)
- [ ] 3.2 Write `helpers/auth/modrinth-oauth.ts` — Modrinth OAuth flow (redirect to Modrinth, handle callback, store Modrinth session, get user_id + username)
- [ ] 3.3 Write `helpers/system/presence.ts` — Presence tracking (poll Core for friend presence via GET /api/friends/presence)

## Phase 4: Instance Management

- [ ] 4.1 Write `helpers/instances/crud.ts` — List/get/create/delete instances (GET /api/instances, GET /api/instances/:id, POST /api/instances, DELETE /api/instances/:id)
- [ ] 4.2 Write `helpers/instances/controls.ts` — Start/stop/kill/command (POST /api/instances/:id/start, /stop, /kill, /command)
- [ ] 4.3 Write `helpers/instances/console.ts` — WebSocket console streaming (connect to ws://core/instances/:id/console, parse JSON { timestamp, line, type }, emit events)
- [ ] 4.4 Write `helpers/instances/modpack.ts` — Modpack config get/update (GET/PUT /api/instances/:id/modpack)
- [ ] 4.5 Write `helpers/instances/sync.ts` — Sync status tracking (compare local mod hashes vs modpack config, download missing from Modrinth CDN, cache by hash)
- [ ] 4.6 Write AGENTS.md for instances/ subfolder

## Phase 5: Caching

- [ ] 5.1 Write `helpers/cache/mods.ts` — Mod file caching by SHA256 hash (check IndexedDB/OPFS cache, download from Modrinth CDN if missing, store by hash)
- [ ] 5.2 Write AGENTS.md for cache/ subfolder

## Phase 6: Friends + Notifications

- [ ] 6.1 Write `helpers/friends/list.ts` — Friend list via Modrinth user lookup (search by username, add/remove friends via Core API)
- [ ] 6.2 Write `helpers/friends/requests.ts` — Friend request handling (pending/accept/decline via Core API)
- [ ] 6.3 Write `helpers/notifications/list.ts` — Get notifications from Core (GET /api/notifications, mark read, mark all read)
- [ ] 6.4 Write `helpers/notifications/invites.ts` — Poll Agent Server for invites (GET /agent/invites, accept → store connection string, decline → delete)
- [ ] 6.5 Write AGENTS.md for friends/ and notifications/ subfolders

## Phase 7: Cleanup

- [ ] 7.1 Delete dead files: process.js, friends.ts (old), skins.ts, jre.js, ads.js, metadata.js, state.js, rendering/
- [ ] 7.2 Move excluded Modrinth files to helpers/_modrinth/: cache.js (old), mr_auth.ts, pack.ts, tags.js, import.js
- [ ] 7.3 Update all imports across components/pages/providers
- [ ] 7.4 Write root `helpers/AGENTS.md` — master index of all subfolders

## Phase 8: Core Backend — Auth Overhaul

- [ ] 8.1 Implement API key generation (ms_owner_ and ms_member_ format, 24 random bytes, stored in Core config + SQLite)
- [ ] 8.2 Replace PASETO/username-password auth with API key validation middleware (Axum extractor validates Bearer header, returns key type + permissions)
- [ ] 8.3 Add POST /api/auth/connect endpoint (validates API key, returns user info + permissions)
- [ ] 8.4 Add Modrinth OAuth token validation (Core calls Modrinth API to verify token, links modrinth_user_id to API key)
- [ ] 8.5 Remove old /login, /setup endpoints
- [ ] 8.6 Add api_keys table to SQLite (key_hash, type, permissions, modrinth_user_id, created_at, revoked_at)

## Phase 9: Core Backend — Missing Endpoints

- [ ] 9.1 Add GET /api/instances (list all instances)
- [ ] 9.2 Add GET /api/instances/:id (single instance detail)
- [ ] 9.3 Add POST /api/instances (create new instance)
- [ ] 9.4 Add DELETE /api/instances/:id (delete instance)
- [ ] 9.5 Add PATCH /api/instances/:id (update instance config)
- [ ] 9.6 Add GET /api/instances/:id/modpack (read modpack config)
- [ ] 9.7 Add PUT /api/instances/:id/modpack (write modpack config)
- [ ] 9.8 Add GET /api/notifications (list notifications for user)
- [ ] 9.9 Add PATCH /api/notifications/:id/read (mark notification read)
- [ ] 9.10 Add friend system endpoints (GET /api/friends, POST /api/friends/request, POST /api/friends/requests/:id/accept, DELETE /api/friends/:id)
- [ ] 9.11 Add invite push to Agent Server (POST /agent/invites when Owner invites Friend)
- [ ] 9.12 Add database tables: friends, friend_requests, notifications, modpack_configs

## Phase 10: Agent Server

- [ ] 10.1 Scaffold Rust/Axum project (separate crate or workspace member)
- [ ] 10.2 Add SQLite with 2 tables: invites (from_user_id, to_user_id, core_url, member_key, permissions, created_at) and core_registry (modrinth_user_id, core_url, last_heartbeat)
- [ ] 10.3 Add POST /agent/invites (Core pushes invite)
- [ ] 10.4 Add GET /agent/invites?user_id=X (Friend polls for pending invites)
- [ ] 10.5 Add POST /agent/invites/:id/acknowledge (Friend accepted, delete invite)
- [ ] 10.6 Add PUT /agent/registry (Core updates its URL on IP change)
- [ ] 10.7 Add GET /agent/registry/:user_id (Friend looks up Core URL)
- [ ] 10.8 Add Modrinth OAuth validation (verify tokens before accepting requests)
