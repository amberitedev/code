# Amberite — Full Project Documentation
---

## What This Project Is

Amberite is a free, open-source platform that makes playing modded Minecraft with a friend group completely seamless. The server owner never has to think about server setup. Their friends never have to think about downloading or installing mods. You pick a modpack, push a button, and everyone in the group can join with the right mods already installed. That is the entire vision.

Amberite directly competes with Modrinth's upcoming hosting service, except Amberite is free and self-hosted. The desktop app is a **full fork of the Modrinth desktop app** (`modrinth/code`). The backend engine (the Core) is fully custom Rust, completely rewritten from scratch with no Modrinth code. The license is **AGPL-3**, meaning the entire project must stay open source forever.

Modrinth's API is used heavily for mod discovery and modpack installation. The app visibly credits Modrinth in the UI — this is intentional. Modrinth's revenue comes from hosting and donations, not API usage. The Amberite user base (people who want to self-host) would largely not pay for hosted services anyway.

---

## The Five Parts of the Project

### 1. The Core (`apps/core/`)

A fully custom Rust backend. Manages Minecraft server instances — starting, stopping, installing mods, watching the process, and keeping it running. Communicates with the App over the network using JWT tokens, and connects to Supabase to register itself as online so friends can find it.

Every friend group has exactly one Core. **Core runs on Windows and Linux. Mac is not supported and is not planned.**

**Authentication:** JWKS RS256 — Core fetches Supabase public key from `.well-known/jwks.json`. No secrets stored in Core config. **First-time setup:** Core prints a 6-digit pairing code on first start. The App uses this code to claim ownership via `POST /setup`.

**Instance state machine:** `Offline → Starting → Running → Stopping → Crashed`

**Graceful stop:** Send `stop` command, wait 30s, then SIGKILL.

**WebSocket auth:** Short-lived ticket (`POST /ws-token`, 60s UUID, in-memory, single-use) — no long-lived credential in the WebSocket connection.

### 2. The App (`apps/app/`)

A full fork of the Modrinth desktop app, written in **Vue 3 + TypeScript + Tailwind + Tauri**. This is what virtually all users interact with. Members browse modpacks, download them, and launch the game. Owners manage their server.

The App has a custom Rust backend library (`packages/amberite-lib/`) that handles Core REST communication, WebSocket console streaming, Supabase auth, mod sync, and friend groups. This library mirrors the role that `packages/app-lib/` (theseus) plays for Modrinth's launcher logic.

All Amberite-specific changes to Modrinth code are marked with `// AMBERITE PATCH` comments. The library (`packages/amberite-lib/`) never modifies theseus internals — any theseus behavior that needs changing is patched in a local copy.

A web version is planned but is not a priority until the desktop version is feature-complete. Linux support for the App is not a near-term priority.

### 3. Supabase (`apps/supabase/`)

The central hub for accounts, friends, friend groups, and Core online status. Handles login via Microsoft (Xbox/MSAL — the same flow Modrinth uses for Minecraft login). Issues JWTs that the Core validates locally with no network call (JWKS signature verification only). Edge Functions handle friend requests, group invitations, Core heartbeat registration, and the Microsoft token exchange.

### 4. The Website (`apps/web/`)

A Cloudflare Pages site at **amberite.dev**. Serves as the download hub and marketing page. Also hosts the remote dashboard at **amberite.dev/dashboard** — users can manage their Core from any browser without installing the desktop app.

Lower priority than the desktop app. Build after the desktop app is stable.

### 5. The Companion Mod (`apps/mod/`)

A small Minecraft mod (Java, Gradle) that runs inside the game client. Applies personal user preferences — keybinds, settings, configs — dynamically when joining a server, and saves changes back on exit. Config files cannot simply be overwritten at the filesystem level without breaking things; they must be applied at runtime via mod hooks.

**Not started. Built last, after all other systems are complete and stable.**

## How Mod Syncing Works

**Owner side:** Selects a modpack from Modrinth, hits "Push to server." The install pipeline:
1. Resolves the correct server-side version via the Modrinth API.
2. Downloads the `.mrpack` archive.
3. Unpacks it: reads `modrinth.index.json`, downloads mods, applies config overrides.
4. Filters to server-compatible mods only (rejects `client`-only mods).
5. Runs stop → install → start on the server instance.
6. Stores the original `.mrpack` so friends can download the canonical pack.
7. Streams progress events (`queued → resolving → downloading → applying → completed/failed`) back to the App.

**Member side:** The modpack appears in their App library. They click Download, the correct client-side version installs automatically. They click Play and are in the game — no manual server address input.

**Compatibility validation:** Before writing any files, Core validates Minecraft version + mod loader (Fabric/Forge/NeoForge) compatibility. Incompatible packs are blocked with an error, never silently installed.

**Mod sync events:** Hybrid model — every modpack change emits a Supabase Realtime event (instant push to all group members) plus a full `.mrpack` snapshot every ~10 changes (so offline members stay in sync). Friends who join late download the full current snapshot as their baseline.

---

## Personal Preferences System

Each user defines preferred client-side mods (performance mods, minimaps, shaders, etc.) stored as a personal profile in Supabase. On every modpack launch:
1. Find compatible versions of preferred mods for that pack's MC version + loader.
2. Add them to the local client install automatically.
3. Apply keybinds and settings at runtime via the Companion Mod.
4. Apply resource packs and shaders.

The server owner can mark settings as **required** (all players must use), **recommended** (suggested on first join), or **open** (player's full choice). Users can define different preference profiles per Minecraft version.

---

## Other Systems

**Friend system** — Steam-style. Add friends by username, friend code, or invite link. See what friends are playing. Send group invitations.

**Friend groups** — a private group tied to one Core instance. Joining grants access to all modpacks on that Core's servers. Default join role is view-only.

**Mod voting** — members propose mods; a notification goes to the group. If enough members agree, the owner gets a one-click install prompt. Threshold is owner-configurable.

**Permissions** — Discord-style roles:
- Viewer: see status, read console (read-only), download pack
- Member: viewer + join/leave server
- Mod: member + start/stop server, add mods (with vote)
- Admin: mod + restart, manage members, delete instances
- Owner: full control, cannot be removed

**Connections:**
- Same machine: no setup needed.
- Same network: works over LAN automatically.
- Remote V1: manual port forwarding.
- Remote V2: Playit.gg agent (raw TCP tunnel) + Cloudflare DNS API to create `{name}.amberite.dev` CNAME → Playit.gg address. Cloudflare Tunnels rejected — HTTP-only on free tier, Minecraft needs raw TCP.

**Authentication:** The App uses the Xbox/MSAL login flow (same as Modrinth) to get a Microsoft token. `packages/amberite-lib/` exchanges this with the `microsoft-auth` Edge Function to get a Supabase JWT, stored in the OS keychain. Core validates JWTs locally via JWKS RS256 — no network call. First-time Core setup uses a 6-digit pairing code.

**Core data directory:** Lives at `{AppData}/amberite-core/` — separate from app data. Uninstalling the App **never** deletes server worlds or configs.

**Onboarding flow:** Welcome → Microsoft login → Core setup (install local / connect to existing / skip) → Main app.

**Library page:** Unified client + server instances in one list, with filter chips: `All` / `Client` / `Server`. Modrinth community servers (public servers) stay as a separate tab — they are a different concept and are not removed.

---

## Peer-to-Peer Failover (V2 — Planned)

This planned feature makes Amberite structurally more resilient than any paid hosting service.

Each group member can opt in to caching a local copy of the server — world data, modpack config, server properties. If the owner's Core goes offline, any member with caching enabled can spin up a temporary instance from their local copy, and the group continues playing until the owner returns.

Hard problems to solve before shipping:
- **World state conflict** — cached copy may be hours old. When the owner returns, their version is canonical. Progress from the temporary session must be handled gracefully.
- **Host authority** — only one temporary host should be elected. Protocol needed to prevent two members spinning up simultaneously with different world states (split-brain prevention).

**Planned for the 2.0 update.**

---

## Current Known Issues

These are the most urgent engineering problems in priority order:

1. **Real Supabase credentials committed to the repo.** Rotate and purge from git history immediately. (SECURITY — CRITICAL)
2. **Core authentication is partially implemented.** JWKS auth is in the rewrite but not verified end-to-end. The App → Core auth flow has not been tested in full.
3. **`packages/amberite-lib/` does not exist yet.** Backend logic sits at `apps/app/backend/` and needs to move. The library has a dual-error type inconsistency (`AmberiteError` in `lib.rs` vs `BackendError` in `error.rs`) that must be resolved first.
4. **`apps/app/tauri/src/mod.rs` compilation blocker.** Duplicate declarations on lines 5–18 / 20–33, `oauth_utils` declared twice. App cannot compile.
5. **Modrinth version desync.** `apps/app/MODRINTH_VERSION.ts` says `v0.13.3`; root should say `v0.13.4`; `pnpm-workspace.yaml` catalog says `v0.13.1`. All stale.
6. **No real tests.** Core uses `axum-test` but coverage is minimal. App has no real tests.
7. **Process stop/kill may not reliably terminate the Minecraft process.** Needs verification after Core rewrite.

---

## Build Priority

### V1 — Get It Working

1. Rotate and purge committed Supabase credentials.
2. Fix `mod.rs` duplicate declarations so the App compiles.
3. Fix dual-error types in `apps/app/backend/` → merge into single `AmberiteError`.
4. Verify Core auth end-to-end (JWKS RS256 + pairing code flow + App calling Core with JWT).
5. Verify server stop/kill actually terminates the Minecraft process.
6. Implement `microsoft-auth` Edge Function (Microsoft token → Supabase session).
7. Wire `packages/amberite-lib/` auth module: Microsoft login → Edge Function → Supabase JWT → OS keychain.
8. Implement Core launcher in `packages/amberite-lib/`: download binary from GitHub Releases, spawn as background process, auto-pair on localhost.
9. Instance CRUD + start/stop/kill via App.
10. Console streaming (WebSocket → Tauri events → UI).
11. Onboarding wizard (Welcome → Microsoft login → Core setup).
12. Merged Library page (client + server instances with `All` / `Client` / `Server` filter chips).
13. Modrinth integration in Core: resolve project/version, download `.mrpack`, filter server-vs-client mods, compatibility validation.
14. `.mrpack` unpacker and packer in Core.
15. Build Supabase tables: `friends`, `friend_groups`, `group_members`, `group_invites`, `mod_sync_events`, `core_registrations`.
16. Mod sync flow end-to-end: owner pushes modpack to Core → Core notifies Supabase Realtime → members see "Update available" → members download and join.
17. Personal preferences system.
18. Website (amberite.dev — marketing + download page).

### V2 — Make It Great

19. Granular Discord-like permissions per group member.
20. Mod voting (propose → group vote → owner one-click install).
21. Playit.gg tunnel + `{name}.amberite.dev` DNS branding.
22. Full monitoring dashboard per instance (TPS, CPU, memory, disk, player count).
23. Multi-account switcher.
24. Century — AI log/crash explainer (parse Minecraft logs, fingerprint crash types, LLM root cause analysis).
25. Companion Mod (runtime config injection, keybinds, settings applied on server join).
26. Peer-to-peer failover (member caching, host election, world state conflict resolution).
27. Remote web dashboard (amberite.dev/dashboard).
28. Macros — Deno JS scripting engine inside Core for automation (Lodestone-style `#[op2]`).

---

## Key Architectural Decisions

**Auth model:** Microsoft login (Xbox/MSAL, reusing Modrinth's existing flow) → `microsoft-auth` Edge Function → Supabase JWT stored in OS keychain. No second login prompt. Google/Discord/GitHub OAuth is web-only. Single account in V1, multi-account switcher in V2.

**Core auth model:** JWKS RS256. Core fetches the Supabase public key once and validates all JWTs locally with no network call. No secrets in Core config — fully open-source safe.

**Pairing:** Core prints a 6-digit code on first start. Localhost auto-pairs silently. Remote requires user to paste URL + code. Owner status is permanent and stored in Supabase.

**Modrinth fork strategy:** Full fork of `modrinth/code`. Amberite patches are marked `// AMBERITE PATCH`. The `packages/amberite-lib/` crate (planned) never touches theseus internals — all theseus patches are in local copies. A GitHub Actions workflow handles upstream sync (opening PRs when Modrinth releases a new version).

**Backend library:** `packages/amberite-lib/` mirrors the role of `packages/app-lib/` (theseus). Currently lives at `apps/app/backend/` and will be moved. It handles all Amberite-specific desktop logic: Core client, auth, mod sync, groups, friends.

**Library page:** Unified client + server instances in one list. Server and client mod sets are the same in Amberite's model — separating them into two screens hides the core product story. Modrinth community servers stay as a separate tab.

**Mod sync:** Hybrid model — Supabase Realtime events per change (instant push) + full `.mrpack` snapshot every ~10 changes (for offline members). Owner clicks "Push to Core" → App exports Theseus profile as `.mrpack` → POST to Core → Core installs + inserts Realtime event row → friends receive push notification.

**Tunneling:** V1 ships with manual port forwarding. V2 uses Playit.gg (raw TCP, free tier works for Minecraft) + Cloudflare DNS API for `{name}.amberite.dev` CNAME. Cloudflare Tunnels explicitly rejected — HTTP-only on free tier.

**Core data directory:** `{AppData}/amberite-core/` is separate from app data. Uninstalling the App never deletes server worlds.

**Testing:** `axum-test` for Core endpoints. Tauri invoke test script for frontend (runs manually, not in CI). No full CI/CD yet — GitHub Actions is limited to the theseus sync workflow.

**CORS on Core:** `amberite.dev` + `localhost` only.

---

## Tech Stack

| Part | Technology |
|---|---|
| Core | Rust + Axum (fully custom, zero Modrinth code) |
| App frontend | Vue 3 + TypeScript + Tailwind + Vite + Pinia + Vue Router + Vue Query |
| App backend library | Rust (`packages/amberite-lib/`) |
| Desktop shell | Tauri (Rust + Vue 3 bridge) |
| Modrinth launcher library | Rust (`packages/app-lib/` — theseus, from upstream) |
| Shared UI components | Vue 3 + Tailwind (`packages/ui/`) |
| Core local database | SQLite (via SQLx with migrations) |
| Cloud database | Supabase (Postgres) |
| Auth | Supabase Auth + JWKS JWT validation on Core |
| Mod API | Modrinth API |
| Tunneling (V2) | Playit.gg + Cloudflare DNS API |
| Website | Cloudflare Pages (Vue 3 + Vite) |
| Cloud hosting option | Oracle Cloud (free tier) / any Linux VPS |
| Companion mod | Java + Gradle |
| Build tooling | pnpm workspaces + Turborepo |
| CI | GitHub Actions (theseus sync only, for now) |

---

## Decisions made on 2026-06-19

These override or clarify earlier sections where the two conflict.

### Identity model

**Amberite-first.** The Amberite account is the primary identity in the desktop app. It is linked to the user's Minecraft account and is used for friends, groups, Core access, and social features. A linked Modrinth account is required only for Modrinth-specific actions: managing uploaded mods/projects, viewing the user's Modrinth profile, and other Modrinth.com operations. If no Modrinth account is linked, those features are unavailable but the rest of the app keeps working.

### Modrinth API authentication

The Modrinth API client must use a real Modrinth bearer token obtained through a real Modrinth login/linking flow. It is not acceptable to send a placeholder string or to rely on public profile data for authenticated actions. The desktop app needs a path for the user to link/authorize their Modrinth account and for the app to obtain a valid Modrinth access token.

### Amberite session storage

Amberite session credentials are stored in the OS keychain (consistent with the existing auth decision in this doc). `localStorage` is not an acceptable place for session tokens or credentials. The implementation should use a Tauri keychain plugin or similar OS-backed secure storage, not `window.localStorage`.

### Dev-mode auth bypasses

All `__actAs`, `dev-owner`, `Bearer dev:<id>`, and similar auth bypasses are being removed. They were temporary scaffolding and must not ship in any build. Debug builds should not silently enable them either; if any dev helper is needed for local development it must be explicitly compile-time gated and impossible to enable in a release binary.

### Core settings persistence

Core settings live in Core's own storage (SQLite/settings file in the Core data directory). The client app may cache settings locally for offline use, but the Core backend is the source of truth. Client-side `localStorage` is not the primary persistence layer for Core settings.

### Automatic Amberite OAuth on Minecraft login

Yes. Adding or selecting a Minecraft account automatically starts the Amberite account setup/linking flow. The UI may show a small explanatory popup so the user understands why a browser window opened, but the flow itself should be automatic.

### Core backend discovery

Core discovery is handled through Convex (the current implementation direction). Convex stores Core registration/redirect metadata per friend group. For a local Core running on the same machine, the app already knows it is local and can short-circuit discovery. Access to a Core is gated by friend-group membership/authorization. Port forwarding / public exposure of a Core is opt-in and separate from Minecraft server access.

### Naming

Keep the `copal` rename. The Core package and product surface stay as Copal.

### Note on backend divergence

The current implementation has moved several subsystems toward Convex (auth, friend groups, Core presence, discovery), while this doc originally described Supabase for those roles. That divergence is acknowledged. The plan is to keep the Convex direction unless a future decision explicitly reverts it, at which point this section should be updated.

---

*Last updated: 2026-06-19. This is the single source of truth. `PROJECT-NEW.md` is now superseded by this file and can be deleted.*
