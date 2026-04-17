# Amberite — Full Project Documentation

> Plain English. No code. Everything explained as if you've never seen the repo.
> Update this file whenever something changes. Feed it to any AI at the start of every session.

---

## What This Project Is

Amberite is a free, open source platform that makes playing modded Minecraft with friends completely seamless. The owner of a server never has to think about setting one up, and their friends never have to think about downloading or installing anything. You install a modpack, push a button, and everyone in your friend group can join. That's the entire vision.

It is directly competing with Modrinth's upcoming hosting service, except Amberite is free and self-hosted. The desktop app is a fork of the Modrinth desktop app. The Core backend is fully custom, originally inspired by Lodestone but completely rewritten from scratch. The license is GPL-3, which means the entire project must stay open source forever.

Modrinth's API is used heavily for mod discovery and modpack installation. The app visibly credits Modrinth in the UI and links to Modrinth mod pages — this is intentional. Modrinth's revenue comes from hosting and donations, not API usage, and the Amberite user demographic (people who want to self-host) largely would not pay for hosted services anyway.

---

## The Five Parts of the Project

### 1. The Core (the backend engine)
A fully custom Rust backend, originally inspired by Lodestone but completely rewritten. It runs on the owner's machine or a cloud server and manages Minecraft server instances — starting, stopping, installing mods, and keeping them running. It communicates with the app over the network using JWT tokens for authentication and connects to Supabase to register itself as online.

Every friend group has exactly one Core. There is no friend group without one. The Core runs on Windows and Linux. Mac is not supported.

### 2. The App (the main client)
A fork of the Modrinth desktop app, written in Vue 3, TypeScript, and Tailwind. This is what 99% of users interact with. Members use it to see their friend groups, browse available modpacks, download them, and launch the game. Owners use it to manage their server. Built with Tauri for desktop; a web version is planned but not a priority until the desktop version is feature-complete. Linux support is not a priority for the app in the near term.

### 3. Supabase (the centralized backend)
The central hub for accounts, friends, friend groups, and which cores are online. Handles login including OAuth with Google, GitHub, and Modrinth. Issues JWTs that the Core validates locally without a network call. Edge Functions handle things like friend requests and Core heartbeat registration.

### 4. The Website
A Cloudflare Pages site at amberite.dev. Acts as the download hub and marketing page. Also hosts the remote dashboard at amberite.dev/dashboard — users log in and connect to their Core from anywhere.

### 5. The Companion Mod
A small Minecraft mod (Gradle build) that runs inside the game client. Handles applying personal preferences — keybinds, settings, configs — dynamically when joining a server, and saving changes back on exit. Cannot simply copy config files because that breaks things; they must be applied at runtime. The most experimental part of the project. **Not started. Built last.**

---

## Repo Structure

The project lives in a **GitHub organization called Amberite**. Everything is in a single monorepo at **amberite/code** (following Modrinth's naming convention). This works because the entire stack shares Vue, Tailwind, and components across the app and website — a shared component library only makes sense in one repo.

```
amberite/code/
│
├── apps/
│   ├── app/                    — desktop client (Modrinth fork)
│   │   ├── upstream/           — SUBMODULE: modrinth/code (never edited)
│   │   ├── src/                — your Vue frontend
│   │   │   ├── components/
│   │   │   ├── pages/
│   │   │   ├── stores/
│   │   │   ├── composables/
│   │   │   └── assets/
│   │   ├── src-tauri/          — Tauri shell
│   │   │   ├── src/
│   │   │   │   └── main.rs
│   │   │   └── tauri.conf.json
│   │   ├── vite.config.ts
│   │   ├── tsconfig.json
│   │   └── package.json
│   │
│   ├── core/                   — fully custom Rust backend (no submodule)
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── auth/
│   │   │   ├── api/
│   │   │   ├── instances/
│   │   │   ├── friends/
│   │   │   └── db/
│   │   ├── migrations/
│   │   ├── Cargo.toml
│   │   └── Cargo.lock
│   │
│   └── mod/                    — companion Minecraft mod (not started, built last)
│       ├── src/
│       ├── build.gradle
│       └── settings.gradle
│
├── packages/
│   ├── ui/                     — shared Vue + Tailwind components (app, web)
│   │   ├── src/
│   │   │   ├── components/
│   │   │   └── index.ts
│   │   └── package.json
│   │
│   ├── types/                  — shared TypeScript types (app, web, supabase)
│   │   ├── src/
│   │   │   ├── api.ts
│   │   │   ├── user.ts
│   │   │   ├── instance.ts
│   │   │   └── index.ts
│   │   └── package.json
│   │
│   └── config/                 — shared Tailwind, ESLint, tsconfig base
│       ├── tailwind.base.js
│       ├── eslint.base.js
│       ├── tsconfig.base.json
│       └── package.json
│
├── supabase/
│   ├── functions/
│   │   ├── friend-request/
│   │   ├── core-heartbeat/
│   │   └── group-invite/
│   ├── migrations/
│   └── config.toml
│
└── web/                        — Cloudflare Pages (amberite.dev)
    ├── src/
    │   ├── pages/
    │   │   ├── index/          — marketing + download
    │   │   └── dashboard/      — remote dashboard (amberite.dev/dashboard)
    │   ├── components/
    │   └── stores/
    ├── vite.config.ts
    └── package.json
```

The Modrinth app is included as a **git submodule** inside `apps/app/upstream/`. It is never edited. When Modrinth releases a new version, the submodule pointer is manually updated after reviewing what changed. The Core has no submodule — it is entirely original code.

Supabase and web are in the same repo but can be made private via GitHub org settings — there is no GPL obligation to publish infrastructure config.

---

## Shipping and CI

Users download and install one single executable. It contains the Amberite app, the compiled Core binary, and everything needed to run. There is an auto-updater; when a new version is released, the entire package updates at once.

The build pipeline (GitHub Actions) does the following on each release:
1. Checks out the monorepo including the Modrinth app submodule
2. Compiles the Rust Core from source (Rust compilation is cached to keep build times fast)
3. Builds the Vue app with Tauri
4. Bundles everything into a single installer
5. Publishes the release to GitHub Releases

When Modrinth releases a new version, it is manually reviewed and the submodule pointer is updated after confirming nothing breaks. This is by design — full control over what ships.

---

## Development Environment

Development happens across two environments and this is intentional:

**WSL (Linux)** — Core development. WSL simulates the Oracle cloud environment. All Rust compilation and server testing happens here. Keep the repo cloned inside WSL's filesystem (not `/mnt/c/`) for performance — Rust compilation through the translation layer is noticeably slow.

**Windows** — App development. The Tauri app and Vue frontend are developed on Windows. The app connects over the network to the Core running in WSL exactly as it would to a real remote Core.

Both environments point at the same monorepo. No special setup is needed — each side just works in its own subfolder.

---

## Peer-to-Peer Failover (Planned Feature)

This is a planned feature that makes Amberite structurally different from any paid hosting service.

Each member of a friend group can opt in to caching a local copy of the server — the world data, modpack config, and server properties — stored in a standardized format. If the owner's Core goes offline, any member with caching enabled can spin up a temporary instance from their local copy and the group plays on that until the owner returns.

The hard problems to solve before shipping this:
- **World state conflict** — the cached copy may be hours old. When the real owner comes back online, their version is canonical and the temporary session's progress needs to be handled gracefully.
- **Host authority** — a protocol is needed to prevent two members from spinning up simultaneously with different world states. Only one temporary host should be elected.

This feature means Amberite is not just free hosting — it is inherently more resilient than paid hosting. A paid service is one machine and one point of failure. Amberite with failover has no single point of failure as long as at least one member is online.

---

## How Mod Syncing Works

**Owner side:** selects a modpack from Modrinth, hits "Push to server." An algorithm finds the correct server-side version, installs it on the Core, and starts the Minecraft server.

**Member side:** the modpack appears in their library. They click Download, the correct client-side version installs automatically, they click Play and are in the game with no manual server address needed.

---

## Personal Preferences System

Each user defines preferred client-side mods (performance mods, minimaps, etc.) stored as their defaults. On every modpack launch the system automatically adds compatible versions of their preferred mods, applies their keybinds and settings dynamically, and applies their resource packs and shaders. The owner can mark settings as required (locked), recommended (suggested), or open (player's choice). The Companion Mod handles the in-game config application. Users can define different preference sets per Minecraft version.

---

## Other Systems

**Friend system** — follows Steam's model. Add friends by username, friend code, or invite link. See what friends are playing. Send group invitations.

**Friend groups** — a private group tied to one Core. Joining grants automatic access to all modpacks on that Core's servers.

**Mod voting** — members propose mods, a notification goes to the group, if enough agree the owner gets a one-click install prompt. Threshold is owner-configurable.

**Permissions** — Discord-style roles controlling who can suggest mods, start/stop servers, invite members, and access the console.

**Connections** — same machine needs no setup. Same network works over LAN. Remote access uses Playit.gg or Cloudflare Tunnel to bypass firewall restrictions, especially important for Oracle cloud which blocks most ports by default.

**Authentication** — Supabase issues JWTs on login. The Core validates them locally via signature check with no network call. First-time Core setup generates a one-time owner key that binds the Core to the owner's Supabase account permanently.

---

## What Is Currently Broken or Incomplete

- **Authentication on the Core is completely missing.** Any request works without login. Most urgent fix.
- **Stopping and killing servers doesn't actually stop the process.** Minecraft keeps running as a ghost process.
- **The Desktop App has leftover Tauri code being faked on web.** Many things are mocked out.
- **The macro/scripting system is unreachable.** Nothing connects to it.
- **There are no real tests.** The one existing test checks that `true === true`.
- **Real Supabase credentials are committed to the repo.** Rotate and purge from git history immediately.

---

## Current Build Priority

1. Fix Core authentication — JWT validation so only logged-in users can control servers
2. Fix process kill/stop — servers actually stop when told to
3. Build friend group and mod sync tables in Supabase
4. Build mod sync flow end to end — owner pushes modpack → members see it → members download it
5. Build the preferences system
6. Build the website
7. Build the Companion Mod
8. Design and implement the peer-to-peer failover system

---

## Tech Stack Quick Reference

| Part | Technology |
|---|---|
| Core | Rust + Axum (fully custom) |
| App | Vue 3 + TypeScript + Tailwind + Tauri |
| Shared UI | Vue 3 + Tailwind components (packages/ui) |
| Shared Types | TypeScript (packages/types) |
| Local database | SQLite |
| Cloud database | Supabase (Postgres) |
| Auth | Supabase Auth (JWT) |
| Mod API | Modrinth API |
| Tunneling | Playit.gg / Cloudflare Tunnel |
| Website | Cloudflare Pages |
| Cloud hosting option | Oracle Cloud (free tier) |
| Companion mod | Java + Gradle |

---

*Last updated: manually. Update this file when features are added, removed, or changed.*
