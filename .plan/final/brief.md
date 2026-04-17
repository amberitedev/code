# Lodestone — Final Plan

**Status:** Ready for implementation
**Vision:** Decentralized social platform for modded Minecraft friend groups

---

## What Lodestone Is

Each friend group hosts their own Core (Rust/Axum backend). The Core manages multiple Minecraft server instances, modpacks, and member access. Friends connect to the Core directly via API key. A self-hosted web Panel (Vue 3) is the UI.

**Key Innovation:** Multi-instance friend groups with automatic modpack sync, Steam-like friend system, and notification-based invites.

---

## Architecture

```
┌──────────────────────────────────────────────────┐
│              Agent Server (Minimal)              │
│  SQLite: invites table + core_registry table     │
│  ~50-100MB RAM, Rust/Axum                        │
│  If it crashes: existing connections still work  │
└────────────────────┬─────────────────────────────┘
                     │ Push invites + URL updates
                     │ Poll for invites
                     ▼
┌──────────────────────────────────────────────────┐
│              Core (Each Owner)                   │
│  Rust/Axum on port 16662                         │
│  SQLite: instances, members, notifications,      │
│          modpacks, API keys                      │
│  Handles 99% of all data and logic               │
└────────────────────┬─────────────────────────────┘
                     │ Direct connection (API Key)
                     ▼
┌──────────────────────────────────────────────────┐
│              Panel (Web-Only)                    │
│  Vue 3 + Tailwind, self-hosted                   │
│  Served by Core (same machine)                   │
│  No desktop app, no Tauri                        │
└──────────────────────────────────────────────────┘
```

---

## Auth Model

Three separate concerns:

| Concern | Solution |
|---------|----------|
| **Identity** (who are you?) | Modrinth OAuth |
| **Discovery** (where is your Core?) | Agent Server registry |
| **Access** (are you allowed?) | API Keys (Owner key + Member key) |

**Connection string format:** `ms_<host>:<port>:<api_key>`
Example: `ms_192.168.1.100:16662:a3f9c2e1b8d47f6e`

**API Key format:** `ms_<type>_<24_random_bytes_base64url>`
- `ms_owner_...` — Full access (create/delete instances, manage members)
- `ms_member_...` — Limited (start/stop, view console, download modpack)

**Flow:**
1. Owner creates Core → Core generates Owner API Key → saves to config
2. Owner logs into Panel with Modrinth OAuth
3. Owner invites Friend by Modrinth username
4. Core generates Member Key → pushes invite to Agent Server
5. Friend logs into Panel with Modrinth OAuth → polls Agent Server → receives invite
6. Friend accepts → stores connection string → connects directly to Core

---

## Agent Server Responsibilities (Minimal)

Stores ONLY two tables:

| Table | Purpose | Data Size |
|-------|---------|-----------|
| `invites` | Guaranteed invite delivery even if Core is offline | ~100KB for 10k invites |
| `core_registry` | Maps user_id → host:port for dynamic IP updates | ~50KB for 10k cores |

Everything else (notifications, modpack updates, crash alerts, member management) stays in Core SQLite. If Agent Server crashes, existing connections work fine — only new invites are temporarily broken.

---

## Message Caching

All notifications (modpack updates, server errors, status changes) are cached in Core SQLite. Friends poll Core directly. This works because:
- Dedicated servers (Oracle/cloud): Core is always online
- Self-hosted (Windows): If Core is offline, Friend can't connect anyway

The ONLY exception is invites — those go through Agent Server for guaranteed delivery when Owner and Friend are never online simultaneously.

---

## Scope

**IN:**
- Helper system (panel/src/helpers/) connecting Panel to Core
- Core API endpoints for instance management, auth, modpacks
- Agent Server (invites + URL registry)
- Friend system via Modrinth OAuth
- Modpack sync (download mods from Modrinth CDN by hash)
- Notification system (Core-cached)

**OUT (deferred):**
- Docker container management
- Deno/JS macro system
- Automated scheduled backups
- Plugin manager UI
- HTTPS/TLS auto-provisioning
- ARM platform support
- Lodestone Atom extension system

---

## Constraints

- Each helper file: max 200 lines, single responsibility
- Helpers organized in subfolders by domain, each with AGENTS.md
- Uses ofetch ($fetch) for HTTP, native WebSocket for streaming
- API keys stored in localStorage, attached via Authorization: Bearer header
- GET /health is PUBLIC (no auth) — lets Panel distinguish "server down" vs "bad key"
- All other endpoints require valid API key
- Panel is web-only: no Tauri, no desktop-only features
- Core can run dedicated (24/7 cloud) or self-hosted (Windows, online when owner is)
