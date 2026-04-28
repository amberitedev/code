# Amberite Features

**Status:** Active planning
**Last updated:** 2026-04-27

## Core (apps/core)
- **Full rewrite** — planned (see `.plan/active/core-rewrite-plan.md`)
  - Delete `src/`, heroic file structure (46 files, 12 dirs)
  - JWKS auth (RS256), first-run pairing code
  - Dissolve `theseus/` silo, merge into proper layers
  - Macros in scope (Lodestone pattern)
  - Instance lifecycle: async creation, graceful stop, auto-restart
- Instance management (start/stop/kill) — **rewrite** (current broken)
- Console streaming (WebSocket) — **rewrite** (auth fixed)
- JWT auth via Supabase — **rewrite** (JWKS, no secrets)
- Supabase heartbeat sync — planned
- Friend groups support — planned
- Mod sync flow — planned
- CLI installer/runner — planned (late-stage, Linux only)
- Endpoint testing (axum-test) — planned
- **Century (log/crash AI explainer)** — planned (NEW)

## Desktop App (apps/app)
- Fork of Modrinth App — done (tracking v0.13.4)
- **Restructure `apps/app/` into `frontend/`, `backend/`, `tauri/`** — planned (milestone 1)
- **Switch to Modrinth staging endpoints for dev** — planned (milestone 1)
- **Vendor theseus + namespace patch** — planned (milestone 1)
- **Wire `amberite-backend` as Tauri plugin (`plugin:amberite|hello`)** — planned (milestone 1)
- **Fix version desynchronization** — planned (milestone 1)
- **GitHub Actions auto-sync workflow** — planned (milestone 1)
- Amberite-specific UI — in-progress
- Local Core launcher — planned
- Auto port-forwarding — planned (V2: Playit.gg + Cloudflare DNS CNAME)

### amberite-backend Modules (full plan: `.plan/desktop-backend/`)
- `auth.rs` — Microsoft → Supabase JWT via Edge Function — planned
- `core_launcher.rs` — download + spawn Core as background process — planned
- `core_client.rs` — HTTP client for Core REST API — planned
- `console_stream.rs` — WebSocket → Tauri `console-line` events — planned
- `progress_stream.rs` — SSE → Tauri `instance-progress` events — planned
- `pairing.rs` — localhost auto-pair + remote code pairing — planned
- `settings.rs` — persist config to `{AppData}/amberite/settings.json` — planned
- `tunnel.rs` — Playit.gg agent + Cloudflare DNS (V2) — planned
- `groups.rs` — Supabase friend groups CRUD — planned
- `friends.rs` — Supabase friends list management — planned
- `mod_sync.rs` — export .mrpack + push to Core + Realtime subscription — planned
- `supabase_client.rs` — base Supabase HTTP + Realtime client — planned

### Frontend New Pages
- `pages/Onboarding.vue` — Welcome → Microsoft login → Core setup wizard — planned
- `pages/ServerDashboard.vue` — console, controls, CPU/mem/disk/TPS, mod sync — planned
- `pages/FriendGroups.vue` — group list, members, permissions, invite links — planned

### Frontend Modified Pages
- `pages/library/index.vue` — merge client + server instances with filter chips — planned
- `pages/Servers.vue` — replace Modrinth billing UI → My Servers list — planned

### Supabase Tables
- `friends` — flat friends list, alphabetically sorted pairs — planned
- `friend_groups` — groups tied to Core instances — planned
- `group_members` — members with Discord-style permissions — planned
- `group_invites` — 24h UUID invite tokens — planned
- `mod_sync_events` — Realtime modpack change event log — planned
- `core_registrations` — Supabase relay for remote Core discovery — planned

### Supabase Edge Functions
- `microsoft-auth` — Microsoft token → Supabase session — planned
- `create-invite-link` — generate 24h group invite token — planned
- `join-group` — validate token, add to group_members — planned

## Web Dashboard (apps/web)
- Marketing/download page — planned
- Remote dashboard at amberite.dev/dashboard — planned (lower priority)
- Docs site — planned

## Companion Mod
- Runtime config injection — planned (built last)

## Peer-to-Peer Failover
- Member caching of world data — planned
- Host election protocol — planned
- World state conflict resolution — planned

---

## Reference Projects (Cross-Industry Standards)

### Client-to-Client Sync
| Project | Domain | Pattern |
|---------|--------|---------|
| **Syncthing** | P2P file sync | Block Exchange Protocol, version vectors |
| **Yjs/Automerge** | CRDT collaboration | Conflict-free merge, offline-first |
| **CouchDB/PouchDB** | Database sync | Revision tree, MVCC |
| **Figma** | Design collab | Property-level sync, fractional indexing |

### Distributed Coordination
| Project | Domain | Pattern |
|---------|--------|---------|
| **etcd** | Kubernetes store | Raft consensus, lease-based locks |
| **Consul** | Service mesh | Session locks, health check integration |
| **ZooKeeper** | Coordination | Ephemeral nodes, sequential znodes |
| **Redis Redlock** | Distributed lock | Quorum-based, TTL auto-release |

### Dashboards (Not Game Panels)
| Project | Domain | Pattern |
|---------|--------|---------|
| **Grafana** | Monitoring | Panel plugins, template variables |
| **Uptime Kuma** | Status monitoring | Vue 3 + Socket.IO (same stack!) |
| **Headlamp** | Kubernetes UI | React + WebSocket logs/terminal |
| **Netdata** | Real-time metrics | Per-second streaming, auto-discovery |

### Social/Team Systems
| Project | Domain | Pattern |
|---------|--------|---------|
| **Matrix/Synapse** | Federated chat | Rooms, power levels, federation |
| **Mattermost** | Team chat | Teams + Channels, Go backend |
| **Keycloak** | IAM | Realms, groups, role mappings |
| **GitLab** | Dev platform | Groups + Projects, access levels |

### Log Analysis/AI
| Project | Domain | Pattern |
|---------|--------|---------|
| **Sentry** | Error tracking | Fingerprinting, AI root cause (Seer) |
| **Graylog** | SIEM | Pipeline processing, stream routing |
| **OpenSearch** | Search | Anomaly detection, ML inference |
| **Wazuh** | Security | Rule-based decoding, correlation |

### Config Sync/Hot Reload
| Project | Domain | Pattern |
|---------|--------|---------|
| **Apollo/Nacos** | Config management | Long-polling, grayscale release |
| **Consul KV** | Service config | Blocking queries, render-and-reload |
| **Unleash** | Feature flags | SSE + polling, identity-based |
| **Flagsmith** | Remote config | WebSocket, local evaluation mode |

### Desktop Remote Control
| Project | Domain | Pattern |
|---------|--------|---------|
| **RustDesk** | Remote desktop | Rust backend, P2P, file transfer |
| **DocKit** | Database GUI | Tauri + Vue 3 (exact stack match!) |
| **GlobalProtect** | VPN client | Tauri + Rust, background service |
| **ServerMint** | Minecraft panel | Tauri + Vue, local server management |

---

### Top References by Feature

| Amberite Feature | Primary Reference | Secondary Reference |
|------------------|-------------------|---------------------|
| **Client sync** | Syncthing (version vectors) | Automerge (CRDTs) |
| **Core coordination** | Consul (session locks) | etcd (Raft) |
| **Web dashboard** | Uptime Kuma (Vue 3 + Socket.IO) | Grafana (panels) |
| **Friend groups** | Mattermost (Teams) | Keycloak (realms) |
| **P2P failover** | Redis Redlock | Consul sessions |
| **Log AI (Century)** | Sentry (Seer AI) | Graylog (pipelines) |
| **Config injection** | Apollo (long-polling) | Unleash (identity-based) |
| **Desktop app** | DocKit (Tauri + Vue 3) | RustDesk (Rust backend) |