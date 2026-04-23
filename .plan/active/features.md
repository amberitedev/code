# Amberite Features

**Status:** Active planning
**Last updated:** 2026-04-19

## Core (apps/core)
- Instance management (start/stop/kill) — done
- Console streaming (WebSocket) — done
- JWT auth via Supabase — planned (blocked: needs Supabase MCP)
- Supabase heartbeat sync — planned
- Friend groups support — planned
- Mod sync flow — planned
- CLI installer/runner — planned (late-stage, Linux only)
- Endpoint testing (axum-test) — planned
- **Century (log/crash AI explainer)** — planned (NEW)

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