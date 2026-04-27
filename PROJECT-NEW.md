# Amberite — Full Project Documentation

> Plain English. No code. Everything explained as if you've never seen the repo.
> Feed this file to any AI at the start of every session. Update it whenever something significant changes.
> Companion files: `FEATURES.md` (every planned feature), `DECISIONS.md` (every architectural decision and open debate).

---

## What This Project Is

Amberite is a free, open-source platform that makes playing modded Minecraft with a friend group completely seamless. The server owner never has to think about server setup. Their friends never have to think about downloading or installing mods. You pick a modpack, push a button, and everyone in the group can join with the right mods already installed. That's the entire vision.

Amberite directly competes with Modrinth's upcoming hosting service, except Amberite is free and self-hosted. The desktop app is a fork of the Modrinth desktop app. The backend engine (the Core) is fully custom Rust, originally inspired by Lodestone but completely rewritten from scratch. The license is **AGPL-3**, meaning the entire project must stay open source forever.

Modrinth's API is used heavily for mod discovery and modpack installation. The app visibly credits Modrinth in the UI and links to Modrinth mod pages — this is intentional and respectful. Modrinth's revenue comes from hosting and donations, not API usage, and the Amberite user base (people who want to self-host) would largely not pay for hosted services anyway.

---

## The Five Parts of the Project

### 1. The Core

A fully custom Rust backend. It runs on the owner's machine or a cloud server and manages Minecraft server instances — starting, stopping, installing mods, watching the server process, and keeping it running. It communicates with the App over the network using JWT tokens for authentication, and connects to Supabase to register itself as online so friends can find it.

Every friend group has exactly one Core. There is no friend group without one. The Core runs on **Windows and Linux**. Mac is not supported and is not planned.

The Core is architecturally layered:
- `domain/` — business logic and entities (instances, mods, users)
- `application/` — services and orchestration (install pipeline, friend sync)
- `infrastructure/` — SQLite, process management, networking, file I/O
- `presentation/` — HTTP handlers and WebSocket (Axum)

### 2. The App

A fork of the Modrinth desktop app, written in **Vue 3 + TypeScript + Tailwind + Tauri**. This is what virtually all users interact with. Members use it to see their friend group, browse modpacks, download them, and launch the game. Owners use it to manage their server. The App has three layers:

- `frontend/` — Vue 3 + Tailwind UI
- `backend/` — Custom Rust/Axum process that handles local app logic, overriding and extending the Core's API for desktop-specific concerns
- `tauri/` — Tauri desktop shell (Rust + Vue 3 bridge)

A web version is planned but is not a priority until the desktop version is feature-complete. Linux support for the App is not a priority near-term.

The Modrinth app source is included as a **git submodule** at `apps/app/upstream/`. It is **never edited directly**. When Modrinth releases a new version, the submodule pointer is manually updated after reviewing what changed. All Amberite customization lives in the app's own `src/` tree.

### 3. Supabase

The central hub for accounts, friends, friend groups, and Core online status. Handles login including OAuth with Google, GitHub, and Modrinth. Issues JWTs that the Core validates locally without a network call (signature verification only). Edge Functions handle things like friend requests, group invitations, and Core heartbeat registration.

The database lives in Supabase Postgres. The app accesses it via the Supabase JS client. The Core validates JWTs independently.

### 4. The Website

A Cloudflare Pages site at **amberite.dev**. Serves as the download hub and marketing page. Also hosts the remote dashboard at **amberite.dev/dashboard** — users can log in from any browser and connect to their Core from anywhere in the world without installing the desktop app.

The website uses the same shared Vue 3 and Tailwind components as the App wherever possible.

### 5. The Companion Mod

A small Minecraft mod (Java, Gradle build) that runs inside the game client. It handles applying personal user preferences — keybinds, settings, configs — dynamically when joining a server, and saves changes back on exit. This cannot be done by simply copying config files because that breaks things; preferences must be applied at runtime via mod hooks.

This is the most experimental part of the project. **It is not started. It is built last**, after all other systems are complete and stable.

---

## Repo Structure

The project lives in a **GitHub organization called `amberitedev`**. Everything is in a single monorepo at `amberitedev/code`, following Modrinth's naming convention. This works because the App and website share Vue components and Tailwind config — a shared component library only makes sense in one repo.

```
amberitedev/code/
│
├── apps/
│   ├── app/                        — desktop client (Modrinth fork)
│   │   ├── upstream/               — SUBMODULE: modrinth/code (never edited)
│   │   ├── frontend/               — Vue 3 + Tailwind UI
│   │   │   ├── src/
│   │   │   │   ├── components/
│   │   │   │   ├── pages/
│   │   │   │   ├── stores/
│   │   │   │   ├── composables/
│   │   │   │   └── assets/
│   │   │   ├── vite.config.ts
│   │   │   └── package.json
│   │   ├── backend/                — Custom Rust/Axum backend (app-layer logic)
│   │   │   ├── src/
│   │   │   └── Cargo.toml
│   │   └── tauri/                  — Tauri desktop shell (READ-ONLY in agent sessions)
│   │       ├── src/
│   │       │   └── main.rs
│   │       └── tauri.conf.json
│   │
│   ├── core/                       — fully custom Rust backend (no submodule)
│   │   ├── src/
│   │   │   ├── domain/
│   │   │   ├── application/
│   │   │   ├── infrastructure/
│   │   │   └── presentation/
│   │   ├── migrations/
│   │   ├── Cargo.toml
│   │   └── Cargo.lock
│   │
│   ├── supabase/                   — Edge Functions + setup docs
│   │   ├── functions/
│   │   │   ├── friend-request/
│   │   │   ├── core-heartbeat/
│   │   │   └── group-invite/
│   │   ├── migrations/
│   │   └── config.toml
│   │
│   ├── web/                        — Cloudflare Pages (amberite.dev)
│   │   ├── src/
│   │   │   ├── pages/
│   │   │   │   ├── index/          — marketing + download
│   │   │   │   └── dashboard/      — remote dashboard (amberite.dev/dashboard)
│   │   │   ├── components/
│   │   │   └── stores/
│   │   ├── vite.config.ts
│   │   └── package.json
│   │
│   └── mod/                        — companion Minecraft mod (not started, built last)
│       ├── src/
│       ├── build.gradle
│       └── settings.gradle
│
├── packages/
│   ├── ui/                         — shared Vue 3 + Tailwind components
│   │   ├── src/
│   │   │   ├── components/
│   │   │   └── index.ts
│   │   └── package.json
│   │
│   ├── types/                      — shared TypeScript types
│   │   ├── src/
│   │   │   ├── api.ts
│   │   │   ├── user.ts
│   │   │   ├── instance.ts
│   │   │   └── index.ts
│   │   └── package.json
│   │
│   └── config/                     — shared Tailwind, ESLint, tsconfig base
│       ├── tailwind.base.js
│       ├── eslint.base.js
│       ├── tsconfig.base.json
│       └── package.json
│
└── scripts/                        — build and utility scripts
```

**Rules for contributors and AI agents:**
- Never overwrite existing files — always edit them.
- Max 200 lines per file (hard rule). Ask before exceeding.
- One component per file (single responsibility).
- All `.env` files are gitignored. Use `.env.example` as template.
- Tauri APIs are mocked in dev — do not import real `@tauri-apps/*` without mocking.
- Use `@` alias for `src/` in imports.
- `apps/app/tauri/` is read-only — do not modify Tauri app code.
- `apps/app/upstream/` is a submodule and is never edited.

---

## Development Environment

Development happens across two environments intentionally:

**WSL (Linux)** — Core development. WSL simulates the Oracle Cloud / Ubuntu server environment. All Rust compilation and server testing happens here. The repo must be cloned inside WSL's own filesystem (not `/mnt/c/`) — Rust compilation through the translation layer is noticeably slow.

**Windows** — App development. The Tauri app and Vue frontend are developed on Windows. The App connects over the network to the Core running in WSL, exactly as it would to a real remote Core.

Both environments point at the same monorepo. Each works in its own subfolder.

Dev commands:

| Task | Command | Directory |
|---|---|---|
| Frontend dev server | `pnpm dev` | `apps/app/frontend/` |
| Frontend build | `pnpm build` | `apps/app/frontend/` |
| App backend (Rust) | `cargo run` | `apps/app/backend/` |
| Core (Rust) | `cargo run` | `apps/core/` |
| Core tests | `cargo test` | `apps/core/` |

---

## Shipping and CI

Users download and install one single executable. It contains the Amberite App, the compiled Core binary, and everything needed. There is an auto-updater; when a new version is released, the entire package updates at once.

The GitHub Actions build pipeline on each release:
1. Checks out the monorepo including the Modrinth app submodule.
2. Compiles the Rust Core from source (cached).
3. Builds the Vue App with Tauri.
4. Bundles everything into a single installer.
5. Publishes to GitHub Releases.

When Modrinth releases a new version, it is manually reviewed and the submodule pointer is updated after confirming nothing breaks. This is by design — full control over what ships.

---

## How Mod Syncing Works (Core Flow)

**Owner side:** Selects a modpack from Modrinth, hits "Push to server." The install pipeline:
1. Resolves the correct server-side version of the modpack via the Modrinth API.
2. Downloads the `.mrpack` archive.
3. Unpacks it: reads `modrinth.index.json`, downloads listed mod files, applies config overrides.
4. Filters to server-compatible mods only (rejects `client`-only mods).
5. Runs a stop → install → start sequence on the server instance.
6. Stores the original `.mrpack` so friends can download the canonical pack.
7. Streams progress events (`queued → resolving → downloading → applying → completed/failed`) back to the App.

**Member side:** The modpack appears in their App library. They click Download, the correct client-side version installs automatically. They click Play and are in the game — no manual server address input required.

**Compatibility validation:** Before writing any files, the Core validates Minecraft version + mod loader (Fabric/Forge/NeoForge) compatibility. If compatibility is unknown, the default is to block with an error, not silently install.

**`.mrpack` format:** Standard Modrinth pack format. The unpacker and packer are both implemented in the Core. Exports can be generated from any installed server instance for sharing or recovery.

---

## Personal Preferences System

Each user defines their preferred client-side mods (performance mods, minimaps, shaders, etc.) stored as a personal profile in Supabase. On every modpack launch:

1. The system finds compatible versions of their preferred mods for that modpack's MC version + loader.
2. Adds them automatically to the local client install.
3. Applies their keybinds and settings at runtime via the Companion Mod.
4. Applies their resource packs and shaders.

The server owner can mark settings as **required** (locked — all players must use), **recommended** (suggested on first join), or **open** (player's full choice). The Companion Mod handles in-game config application because config files cannot simply be overwritten at the filesystem level without breaking things.

Users can define different preference profiles per Minecraft version.

---

## Other Systems

**Friend system** — follows Steam's model. Add friends by username, friend code, or invite link. See what friends are playing. Send group invitations.

**Friend groups** — a private group tied to one Core. Joining grants automatic access to all modpacks on that Core's servers. Managed via Supabase Edge Functions.

**Mod voting** — members propose mods; a notification goes to the group. If enough members agree, the owner gets a one-click install prompt. Approval threshold is owner-configurable.

**Permissions** — Discord-style roles controlling who can: suggest mods, start/stop servers, invite members, access the console.

**Connections:**
- Same machine: no setup needed.
- Same network: works over LAN automatically.
- Remote: uses **Playit.gg** or **Cloudflare Tunnel** to bypass firewall restrictions. Especially important for Oracle Cloud, which blocks most ports by default.

**Authentication:** Supabase issues JWTs on login. The Core validates them locally via signature check — no network call required. First-time Core setup generates a one-time owner key that permanently binds the Core to the owner's Supabase account.

---

## Peer-to-Peer Failover (Planned, Not Started)

This planned feature makes Amberite structurally more resilient than any paid hosting service.

Each group member can opt in to caching a local copy of the server — world data, modpack config, and server properties — in a standardized format. If the owner's Core goes offline, any member with caching enabled can spin up a temporary instance from their local copy, and the group continues playing until the owner returns.

Hard problems to solve before shipping this:
- **World state conflict** — the cached copy may be hours old. When the owner returns, their version is canonical. Progress from the temporary session must be handled gracefully.
- **Host authority** — a protocol is needed to prevent two members from spinning up simultaneously with different world states. Only one temporary host should be elected.

This feature makes Amberite's resilience argument: a paid service is one machine and one point of failure. Amberite with failover has no single point of failure as long as at least one member is online.

---

## Current Known Issues (Urgent)

These are the most urgent engineering problems in priority order:

1. **Core authentication is completely absent.** Any request works without login. This is the most critical security issue.
2. **Server stop/kill does not actually stop the Minecraft process.** The process continues running as a ghost after a stop command.
3. **Real Supabase credentials are committed to the repo.** These must be rotated and purged from git history immediately.
4. **The desktop App has leftover Tauri code being faked on web.** Many things are mocked out.
5. **The macro/scripting system is unreachable.** Nothing in the App connects to it.
6. **There are no real tests.** The one existing test checks `true === true`.

---

## Current Build Priority

1. Fix Core authentication — JWT validation so only authenticated users can control servers.
2. Fix process stop/kill — servers actually stop when commanded.
3. Rotate and purge committed Supabase credentials.
4. Build install action pipeline — accept install commands, execute asynchronously, stream status.
5. Build Modrinth integration — project/version resolution, server-side compatibility filtering, file download.
6. Build `.mrpack` unpacker and packer.
7. Build friend group and mod sync tables in Supabase.
8. Build mod sync flow end to end: owner pushes modpack → members see it → members download and join.
9. Build the preferences system.
10. Build the website (amberite.dev).
11. Build the Companion Mod.
12. Design and implement peer-to-peer failover.

---

## Memory / Session Tracking

At the start of every AI session, read:
1. `PROJECT.md` — this file, full project vision.
2. `.plan/active/features.md` — every planned feature and its status.
3. `.plan/active/bugs.md` — every known bug.
4. `.plan/active/decisions.md` — every architectural decision and open debate.

At the end of every session (or on request), update the `.plan/active/` files with anything new learned or decided. These files are the persistent memory of the project across AI sessions.

---

## Tech Stack Quick Reference

| Part | Technology |
|---|---|
| Core | Rust + Axum (fully custom) |
| App frontend | Vue 3 + TypeScript + Tailwind + Vite + Pinia + Vue Router + Vue Query |
| App backend | Rust + Axum (desktop app layer) |
| Desktop shell | Tauri (Rust + Vue 3) |
| Shared UI components | Vue 3 + Tailwind (`packages/ui`) |
| Shared types | TypeScript (`packages/types`) |
| Local database | SQLite |
| Cloud database | Supabase (Postgres) |
| Auth | Supabase Auth (JWT) |
| Mod API | Modrinth API |
| Tunneling | Playit.gg / Cloudflare Tunnel |
| Website | Cloudflare Pages (Vue 3 + Vite) |
| Cloud hosting option | Oracle Cloud (free tier) / any Linux VPS |
| Companion mod | Java + Gradle |
| Build tooling | pnpm workspaces + Turborepo |
| CI | GitHub Actions |

---

*Last updated: 2026-04. Update this file when features are added, removed, or changed. Companion files: `FEATURES.md`, `DECISIONS.md`.*
