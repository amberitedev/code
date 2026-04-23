# Theseus Integration Plan

## Goal

Get mod sync working by copying Theseus code directly into Core. Theseus is NOT a dependency — we copy the source files so we can modify them freely without upstream changes breaking us. Refactor for clean architecture later.

---

## Architecture (Plain English)

ONE Core per friend group. The Core runs on the owner's machine (or cloud server). Friends connect to it via their App (Tauri client). Friends do NOT have their own Core — they just have the App.

Supabase handles: auth (JWT), friend group membership, Core address registry, and Realtime notifications. All messages relay through Supabase to ensure delivery — we don't build our own message queue.

---

## Mod Sync Workflow

### Owner Pushes Modpack

1. Owner has a Minecraft instance on their App (local, not on Core)
2. Owner's App exports that instance as a .mrpack file (Theseus has this function)
3. Owner's App sends the .mrpack to Core via HTTP POST with JWT auth
4. Core unpacks the .mrpack and installs it using Theseus code
5. During install, Core downloads mods from Modrinth API (not from owner's computer)
6. Core starts the Minecraft server
7. Core stores the ORIGINAL .mrpack that was sent (for serving to friends later)
8. Core broadcasts to Supabase: "new instance added" with metadata (name, version, icon, game_version, loader)

### Supabase Relays

9. Supabase Realtime broadcasts to all connected Apps in that friend group
10. Message contains instance metadata (extracted from .mrpack's modrinth.index.json)

### Friend Downloads

11. Friend's App receives notification, caches metadata locally, displays in library
12. Friend clicks Download
13. Friend's App checks if it has Core address cached — if not, asks Supabase for it
14. Supabase returns Core URL, App caches it
15. Friend's App connects to Core via HTTPS with JWT auth header
16. Core validates JWT signature (no network call — uses Supabase JWT secret)
17. Core sends the stored .mrpack file
18. Friend's App imports into their local Minecraft instance (using Theseus code)

---

## What Theseus Code Does

Theseus (from modrinth/code repo) handles:

- **Installing .mrpack files** — unzip, parse modrinth.index.json, download mods from Modrinth, extract overrides
- **Exporting .mrpack files** — take an instance, pack it into .mrpack format
- **Managing profiles/instances** — track game_version, loader, loader_version, installed mods
- **Downloading Minecraft** — from Mojang, including Fabric/Forge/NeoForge loaders
- **Launching Minecraft** — build JVM args, spawn process

We copy these functions directly into Core. Skip the Microsoft auth module (not needed for server). Keep everything else.

---

## Implementation Steps

### Step 1: Copy Theseus Source

Clone modrinth/code repo. Copy `packages/app-lib/src/` into `apps/core/src/theseus/`. 

Folders to copy: launcher/, profile/, pack/, state/, util/. Skip: auth/.

Total: ~5,000 lines. This is fine — we'll trim later.

### Step 2: Merge Dependencies

Theseus uses: daedalus (MC version manifests), async_zip, sha1/sha2 (hash verification). Add these to Core's Cargo.toml. Core already has tokio, reqwest, serde, zip, sqlx, chrono, uuid.

### Step 3: Wire Theseus State

Theseus expects a global State singleton initialized at startup. Adapt this to Core's main.rs. Point Theseus's data directory to `.lodestone/` (not its default `~/.local/share/modrinth/`).

### Step 4: Replace Core's Instance System

Core's current GameInstance struct is minimal (just id + name). Delete it. Use Theseus's Profile struct instead, which has game_version, loader, loader_version, etc.

Core's API endpoints stay the same — they just call Theseus internally.

### Step 5: Add Modpack Endpoints

POST /instances — receive .mrpack from owner, install using Theseus
GET /instances/:id/modpack — serve stored .mrpack to friends
POST /instances/:id/start — start server using Theseus launch code
POST /instances/:id/stop — kill process (Theseus handles this)

### Step 6: Supabase Tables

instance_manifests table: stores metadata for each instance (name, version, game_version, loader, icon). This is what gets broadcast to friends via Realtime.

cores table: stores Core address so Apps can fetch it.

### Step 7: JWT Auth

Core validates JWT using Supabase's JWT secret (from Supabase dashboard). Signature check only — no database call. Extract user_id from JWT claims. Use this to verify the user is in the friend group before serving .mrpack.

---

## Storage Locations

All in `.lodestone/`:

- instances/{id}/ — instance directory
- instances/{id}/modpack.mrpack — original .mrpack sent by owner
- instances/{id}/server/ — Minecraft server files
- state.db — Theseus's SQLite (profiles, mod cache, etc.)

---

## Key Constraints

1. **Core must be online** for friends to download .mrpack. This is acceptable for an open source self-hosted project.

2. **JWT auth required** for all endpoints. Supabase issues JWT on login. Core validates locally.

3. **Messages relay via Supabase**. Core doesn't maintain WebSocket connections to Apps — it just posts to Supabase, Supabase broadcasts via Realtime.

4. **Core address cached on App**. App fetches once from Supabase, caches, re-fetches if connection fails.

---

## Open Questions

1. Icon storage: embed in .mrpack metadata, or upload to Supabase Storage? (Probably embed — simpler)

2. Core heartbeat: how often does Core ping Supabase to say "I'm online"? (Maybe every 30 seconds, update cores table)

3. Tunneling: if owner's Core is behind firewall, use playit.gg or Cloudflare Tunnel? (Core should auto-detect and configure)

---

## Future Features (Backlog)

- **Modpack backup history**: store multiple .mrpack versions per instance, allow rollback
- **Peer-to-peer failover**: members cache server world locally, can host temporarily if Core dies

---

## Estimated Time

~2 weeks to get working. Most time spent on: copying/adapting Theseus code, wiring state, testing flow.