# Lodestone — Planning Context

Rolling summary of all decisions made during planning conversations.

---

## Decision Log

### Platform Decision
- **Decision:** Panel is web-only, self-hosted dashboard
- **Rationale:** No desktop app exists in this repo. Tauri is fully mocked. Panel is served alongside Core on the same machine.
- **Impact:** Delete all Tauri-specific helpers (process.js, jre.js, skins.ts). No local process spawning, no OS keychain — localStorage only.

### Auth Model
- **Decision:** Three-layer auth: Modrinth OAuth (identity) + API Keys (access) + Agent Server (discovery)
- **Rationale:** Modrinth OAuth solves global identity (usernames, friend codes) without building a user DB. API Keys (owner vs member) control Core access. Agent Server handles invite delivery and URL registry.
- **Impact:** Remove existing PASETO/Argon2/username-password system from Core. Replace with API key validation middleware.

### API Key Format
- **Decision:** `ms_<type>_<24_random_bytes_base64url>` (e.g., `ms_owner_a3f9c2e1...`)
- **Rationale:** Human-readable prefix prevents accidental paste. 192 bits of entropy is cryptographically secure. No UUIDs (predictable).
- **Two tiers:** Owner (full control) and Member (limited). Start simple, add Admin tier later if needed.

### Connection String Format
- **Decision:** `ms_<host>:<port>:<api_key>`
- **Rationale:** Single string contains everything needed to connect. Owner shares it once, Friend stores it, done forever.
- **Example:** `ms_192.168.1.100:16662:a3f9c2e1b8d47f6e`

### Agent Server Scope
- **Decision:** Minimal — only invites table and core_registry table
- **Rationale:** Owner wants 99% of data on Core. Agent Server is a thin relay that can crash without breaking existing connections.
- **Tech stack:** Rust/Axum (same as Core, share types/code). SQLite. ~50-100MB RAM.
- **If it crashes:** Existing connections work. New invites temporarily broken. No data loss for Core.

### Message Caching
- **Decision:** All notifications cached in Core SQLite. Friends poll Core directly.
- **Rationale:** If Core is offline (self-hosted Windows), Friend can't connect anyway — notifications don't matter. If Core is dedicated (Oracle), it's always online.
- **Exception:** Invites go through Agent Server for guaranteed delivery when Owner and Friend are never online simultaneously.

### Peer Backup (Dropped)
- **Decision:** Removed from plan
- **Rationale:** Standard backups + manual export is sufficient. If server crashes, world data is rarely lost. If losing access to server, Owner would know in advance and export. Distributed world sharding across browsers is over-engineered for the use case.

### Deferred Features
- **Decision:** The following are explicitly deferred to later phases:
  - Docker container management
  - Deno/JS macro system
  - Automated scheduled backups
  - Plugin manager UI
  - HTTPS/TLS auto-provisioning
  - ARM platform support
  - Lodestone Atom extension system
  - Console paging / history scrollback
  - Import existing server
  - Event viewer / audit log
- **Rationale:** Owner wants to focus on core friend-group functionality first. These features will be added once testing infrastructure exists.

### Dynamic IP Handling
- **Decision:** Deferred. Start with manual connection string sharing.
- **Rationale:** Agent Server core_registry table is in the schema for future use. Adding URL auto-update later is low effort (1 table + 1 endpoint + Panel button). Not blocking for MVP.

### Mod Caching (Web Browser)
- **Decision:** Use IndexedDB or OPFS for client-side mod caching
- **Rationale:** Web browser cannot use native SQLite. IndexedDB supports large binary blobs (mod JARs). Cache key = SHA256 hash of mod file.

### Existing Code to Reuse
- **Keep:** analytics.ts (event tracking), utils.js (pure utilities), storage/ (localStorage wrapper)
- **Replace:** auth.js, profile.ts, settings.ts, events.js, logs.js, worlds.ts, types.d.ts
- **Delete:** process.js, friends.ts (old), skins.ts, jre.js, ads.js, metadata.js, state.js, rendering/
- **Exclude (move to _modrinth/):** cache.js, mr_auth.ts, pack.ts, tags.js, import.js

### Frontend ANSI Colors
- **Note:** PTY implementation exists in Core (pty_spawner.rs) and preserves ANSI codes. Panel console helper (instances/console.ts) must parse ANSI → HTML using a library like ansi_up or xterm.js. This is covered in Phase 4 step 4.3.

---

## Open Items (Acceptable for MVP)

These are known gaps that are acceptable to ship without:

| Item | Status | Why It's OK |
|------|--------|-------------|
| HTTPS/TLS | No TLS support | Users can put behind nginx/caddy reverse proxy |
| Invite retry queue | No retry if Agent unreachable | Owner sees error, can retry manually or share string |
| Multiple permission tiers | Only Owner + Member | Add Admin tier later if needed |
| Heartbeat to Agent Server | Not implemented | Add when dynamic IP support is needed |
| Pre-download modpacks | Can't download when Core offline | Friend can only download when Core is online, which is when they'd play anyway |

---

## Architecture Stress Test Results

20 scenarios tested. Failures found and addressed:

| Failure | Fix Applied |
|---------|------------|
| Invite delivery when never online together | Agent Server stores invites |
| Dynamic IP breaks connection string | Deferred — core_registry table ready for later |
| Agent Server unreachable when pushing invite | Deferred — manual retry acceptable for MVP |
| Agent Server permanent shutdown | Acceptable risk — existing connections survive |

All other scenarios (direct Core connection, modpack sync, console streaming, member management, key revocation) work with Core-only architecture.
