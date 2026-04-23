# Amberite Features

**Status:** Active planning
**Last updated:** 2026-04-19

---

## Core (apps/core)
- Instance management (start/stop/kill) — done
- Console streaming (WebSocket) — done
- JWT auth via Supabase — planned (blocked: needs Supabase MCP)
- Supabase heartbeat sync — planned
- Friend groups support — planned
- Mod sync flow — planned (manifest-based, NOT binary transfer)
- CLI installer/runner — planned (late-stage, Linux only)
- Endpoint testing (axum-test) — planned
- **Century (log/crash AI explainer)** — planned

## Desktop App (apps/app)
- Fork of Modrinth App — done (tracking v0.13.1)
- Amberite-specific UI — in-progress
- Local Core launcher — planned
- Auto port-forwarding — planned
- Invoke test script — planned

## Web Dashboard (apps/web)
- Marketing/download page — planned
- Remote dashboard at amberite.dev/dashboard — planned (lower priority)
- Docs site — planned

## Companion Mod
- **Approach:** Likely port Essential mod, change UI, swap backend to use Core
- OR just use Essential directly — undecided
- Runtime config injection — planned (built last)

## Peer-to-Peer Failover
- Member caching of world data — planned
- Host election protocol — planned
- World state conflict resolution — planned

---

## Mod Sync Architecture (Clarified)

**NOT binary transfer.** Uses Modrinth's existing infrastructure:

1. **Modrinth API** contains all mods with versions — no need to send .jar files
2. **Sync is manifest-based:** Send config saying "download these mods from Modrinth API"
3. **Only custom/private mods** (not on Modrinth) need physical transfer
4. **.mrpack format** handles this automatically:
   - `modrinth.index.json` — list of mods to download (project ID, version, hash)
   - Physical files packed into ZIP for mods not on Modrinth
5. **Exporter/unexporter already exists** in Theseus — just use it

**Flow:**
- Owner pushes modpack → Core creates manifest (uses Modrinth API for most mods)
- Manifest uploaded to Supabase
- Member app fetches manifest → downloads mods from Modrinth API
- Custom mods bundled in .mrpack → extracted separately

---

## Reference Projects (Possible Inspiration)

**Note:** These are projects that *might* have useful patterns. Look at how they solve problems, evaluate if approach matches Amberite's needs.

### From Modrinth Awesome List (https://github.com/modrinth/awesome)

| Project | What It Does | Why Might Be Useful |
|---------|--------------|---------------------|
| **packwiz** | TOML-based modpack format, Git-friendly | Pattern for storing modpack metadata |
| **ferium** | CLI mod manager (Rust) | Rust-based mod downloading patterns |
| **mrpack-install** | Server deployment from .mrpack | How to extract/install on server side |
| **AutoModpack** | In-game modpack installer (Fabric mod) | Server-to-client mod sync pattern |
| **modrinth-rs/ferinth** | Rust API wrappers | How to call Modrinth API from Rust |

### Cross-Industry (Non-Minecraft)

| Project | Domain | Why Look At It |
|---------|--------|----------------|
| **Syncthing** | P2P file sync | Version vectors for tracking changes (if we ever need file sync) |
| **Consul** | Coordination | Session-based locks for leader election |
| **Uptime Kuma** | Dashboard | Vue 3 + Socket.IO stack match |
| **Sentry** | Error tracking | Seer AI for crash analysis inspiration |
| **Apollo/Nacos** | Config sync | Long-polling pattern for config push |
| **Mattermost** | Team chat | Teams + Channels pattern for friend groups |
| **DocKit** | Database GUI | Tauri + Vue 3 exact stack match |
| **GlobalProtect-openconnect** | VPN client | Tauri background service pattern |

### Direct Domain Matches

| Project | Why |
|---------|-----|
| **Lodestone** | Rust/Axum + Tauri exact stack, process management patterns |
| **Pterodactyl Wings** | WebSocket console, crash recovery, state machine |
| **Essential Mod** | Companion Mod approach — likely just port this |

---

**When evaluating references:** Ask "How do they solve X? Does their approach match Amberite's constraints? If yes, consider adopting. If no, learn why and adjust."