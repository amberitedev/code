# Lodestone Core Feature Roadmap

**Status:** Phase 0 — Feature Audit  
**Date:** April 11, 2026  
**Scope:** Lodestone Core (Rust/Axum backend)

---

## ✅ Implemented Features

### Core Functionality
- **Basic instance management** - Start/stop/kill commands via API
- **Console streaming** - WebSocket-based live console output
- **Command execution** - Send console commands to running instances
- **User authentication** - PASETO token-based auth (login/setup)
- **Health monitoring** - `/health` and `/stats` endpoints

### Server Types
- **Vanilla** - Standard Minecraft server support
- **Paper** - PaperMC server support
- **Fabric** - Fabric mod loader support
- **Forge/NeoForge** - Forge mod loader support (mentioned in docs)

### Infrastructure
- **SQLite database** - Persistent storage for instances and users
- **SQLx migrations** - Database schema versioning
- **UPnP support** - Stub implementation for port forwarding
- **playit.gg tunneling** - Basic tunnel creation (outdated API)

---

## 🚧 In-Progress / WIP Features

### Docker Container Management
**Status:** Partially implemented  
**Issue:** #438  
**Description:** The partial Docker "Instance" type needs to be finished. The core can talk to the Docker daemon (via the Docker API or bollard crate in Rust), create/start/stop/remove containers, stream their stdout/stderr to the console UI, expose port bindings, and mount volumes through the file manager — all from the same unified Lodestone interface that manages Minecraft servers.

**Current State:**  
- No Docker dependencies in Cargo.toml  
- No bollard crate integration  
- No container management APIs

---

## 📋 Missing Features (High Priority)

### 1. Automated Server Backups
**Status:** NOT IMPLEMENTED  
**Issue:** #435  
**Priority:** High  
**Description:** The most-requested missing feature. Users want scheduled automatic backups of world data and server files — ideally with configurable intervals (hourly, daily), retention policies (keep last N backups), and the ability to restore from backup directly in the dashboard. No backup system currently exists in any form.

**Requirements:**
- Backup tables in database schema
- Scheduled job system (cron-like)
- Configurable backup intervals (hourly, daily, weekly)
- Retention policies (keep last N backups)
- Backup restore API endpoints
- UI for backup management

**Implementation Steps:**
1. Add `backups` table to database
2. Create backup API endpoints
3. Implement backup scheduling
4. Add retention policy logic
5. Build restore functionality

---

### 2. Custom Java Version Per Instance
**Status:** NOT IMPLEMENTED  
**Issue:** #436  
**Priority:** High  
**Description:** Different Minecraft versions require different Java versions (Java 8 for 1.16 and older, Java 17 for 1.17–1.20, Java 21 for 1.21+). Right now Lodestone uses the system Java globally. Users want to specify a custom Java executable path per server instance so they can run multiple Minecraft versions simultaneously.

**Requirements:**
- Java version tracking in database
- Per-instance Java executable paths
- JRE bundling or download
- Java version validation per Minecraft version
- Instance-specific Java configuration

**Implementation Steps:**
1. Add `java_versions` table to database
2. Add `java_path` field to instances table
3. Implement Java version detection
4. Update process spawner to use instance-specific Java
5. Add JRE bundling/download logic

---

### 3. Plugin and Mod Management UI
**Status:** NOT IMPLEMENTED  
**Priority:** High  
**Description:** A full package manager for server-side content. On the backend (Rust/core), this means implementing APIs to query Modrinth/CurseForge/Hangar, download JARs into the correct folders, track installed versions, and handle updates. On the frontend (React/TS), a browsable catalog with search, filters (by game version, loader type), install buttons, and a "manage installed" tab. Think of it like an app store built into the dashboard.

**Requirements:**
- Modrinth API integration
- CurseForge API integration (optional)
- Mod download service
- Mod tracking in database
- Version management
- Update detection
- Dependency resolution
- Loader type support (Fabric/Forge/Quilt)

**Implementation Steps:**
1. Add `mods` and `mod_versions` tables to database
2. Integrate Modrinth API client (ferinth or modrinth-rs)
3. Implement mod download service
4. Add mod list management APIs
5. Build update detection logic
6. Implement dependency resolution

---

### 4. Import Existing Server
**Status:** NOT IMPLEMENTED  
**Priority:** Medium  
**Description:** Allow importing an already-running or pre-configured server directory into Lodestone rather than only supporting fresh installs. This is critical for users migrating from other panels (Pterodactyl, AMP, etc.) or who already have a world they want to manage via Lodestone.

**PR Reference:** #348 (draft PR opened Oct 2023)

**Requirements:**
- Server directory scanner
- Existing config importer
- Migration wizard
- World save detection
- Mod list extraction from existing instances
- Import validation

**Implementation Steps:**
1. Add import API endpoints
2. Implement server directory scanner
3. Create import validation logic
4. Build migration wizard
5. Add world save import support

---

### 5. Console Paging / History Scrollback
**Status:** PARTIAL - Live only, no history  
**Priority:** Medium  
**Description:** A draft PR (#360) was opened for paginated console output. Currently the console only shows a limited live buffer. Users want to be able to scroll back through older log history, search within it, and page through large log files — especially useful after a crash or restart.

**PR Reference:** #360 (draft PR opened Dec 2023)

**Requirements:**
- Log file pagination API
- Historical log search
- Scrollback buffer management
- Search within logs
- Log file reading

**Implementation Steps:**
1. Add log file reading service
2. Implement pagination API
3. Add search functionality
4. Build scrollback buffer
5. Create log file parser

---

## 📋 Missing Features (Medium Priority)

### 6. Event Viewer / Audit Log
**Status:** PARTIAL - Basic events table only  
**Priority:** Medium  
**Description:** A structured, persistent event log separated from the raw server console. The core would emit events (typed structs in Rust) for things like: instance state changes, user authentication events, macro executions, file system operations, player join/leave. This is distinct from the raw console log — it's a structured, filterable event history for accountability and debugging.

**Requirements:**
- Structured event types
- Event emission system
- Filterable API
- Timeline UI
- Severity levels
- Event history storage

**Current State:**
- `events` table exists in database
- `InstanceEvent` enum exists in `instance_actor.rs`
- No structured event emission
- No filterable API

**Implementation Steps:**
1. Define structured event types
2. Implement event emission system
3. Add event filtering API
4. Build event history storage
5. Create timeline UI

---

### 7. Automatic TLS/SSL Certificate Provisioning
**Status:** NOT IMPLEMENTED  
**Priority:** Medium  
**Description:** The mixed-content browser issue stems from the lack of HTTPS on Lodestone Core. A widely-discussed solution is to integrate Let's Encrypt / ACME auto-cert provisioning so the core serves HTTPS natively — eliminating the need for users to manually configure certs or allow insecure content.

**Requirements:**
- HTTPS server support
- ACME client integration
- Let's Encrypt support
- Auto-renewal
- Certificate storage
- Certificate management API

**Implementation Steps:**
1. Add HTTPS server support to Axum
2. Integrate ACME client (e.g., `acme-client`)
3. Implement Let's Encrypt flow
4. Add auto-renewal logic
5. Create certificate management API

---

## 📋 Missing Features (Low Priority)

### 8. Full Docker Container Management
**Status:** NOT IMPLEMENTED  
**Priority:** Low  
**Description:** The partial Docker "Instance" type needs to be finished. This means the core can talk to the Docker daemon (via the Docker API or bollard crate in Rust), create/start/stop/remove containers, stream their stdout/stderr to the console UI, expose port bindings, and mount volumes through the file manager — all from the same unified Lodestone interface that manages Minecraft servers.

**Requirements:**
- Docker API client integration
- Container lifecycle management
- Port binding management
- Volume mounting
- Container stdout/stderr streaming

**Implementation Steps:**
1. Add `bollard` crate dependency
2. Implement Docker client
3. Add container management APIs
4. Build port binding logic
5. Create volume management

---

### 9. Linux/macOS ARM Platform Broadening
**Status:** PARTIALLY BLOCKED  
**Priority:** Low  
**Description:** The team discussed extending support to more platforms. Specifically: Intel Macs (deprecated but community interest exists), additional Linux ARM variants, and improving the Windows Desktop (Tauri) app to reach stable status. Currently the Desktop app is experimental and Windows-only.

**Requirements:**
- ARM build pipeline
- Intel Mac testing
- Cross-compilation support
- Additional Linux ARM variants

**Current State:**
- OpenSSL vendored feature exists (`vendored-openssl` in Cargo.toml)
- Intel Mac deprecated per issue tracker
- No ARM-specific build configs

**Implementation Steps:**
1. Add ARM build targets
2. Set up cross-compilation
3. Find community testers
4. Add Intel Mac support

---

## 📋 Roadmap Items (Planned but Not Started)

### 10. Lodestone Atom (Extension System)
**Status:** Partially shipped in v0.5.x  
**Description:** "Atoms" are TypeScript/JavaScript extensions that can define new instance types (e.g. a Valheim server, a Terraria server) without modifying the core Rust code. The lodestone-atom-lib and lodestone-macro-lib repos support this. The goal is a fully pluggable architecture where community members can publish Atoms for any game or service, and users install them like packages.

**Current State:**  
- `lodestone-atom-lib` and `lodestone-macro-lib` repositories exist  
- Partial implementation in v0.5.x  
- Not fully integrated into core

**Requirements:**
- Atom plugin system
- Runtime atom loading
- Atom lifecycle management
- Atom repository integration

---

### 11. Spring 2023 Roadmap Items

**Shipped (v0.4.x - v0.5.1):**
- Beautiful file manager
- Folder zip/download
- playit.gg integration
- Forge and Paper server type support

**Not Finished:**
- Full Docker integration
- Plugin manager
- Event viewer
- Import existing server

---

## Feature Priority Matrix

| Feature | Priority | Status | Impact |
|---------|----------|--------|--------|
| Automated Server Backups | High | Not Started | Critical for production |
| Custom Java Version | High | Not Started | Enables multi-version support |
| Plugin & Mod Management | High | Not Started | Core functionality gap |
| Import Existing Server | Medium | Not Started | Migration support |
| Console Paging | Medium | Partial | UX improvement |
| Event Viewer | Medium | Partial | Accountability feature |
| TLS/SSL Auto-Provision | Medium | Not Started | Security/UX |
| Docker Management | Low | Not Started | Platform expansion |
| ARM Platform Support | Low | Partial | Platform reach |

---

## Summary

**Total Features:** 11  
**Implemented:** 7  
**In-Progress:** 1  
**Missing (High Priority):** 3  
**Missing (Medium Priority):** 3  
**Missing (Low Priority):** 3  

**Missing Core Features:** 8  
**Missing UX Features:** 3  

**Your Lodestone fork is essentially rebuilding 70% of the planned features from scratch.** The existing codebase is a skeleton with basic server management but none of the advanced features that make it production-ready.

---

## Recommended Implementation Order

### Phase 1: Critical Foundation (Weeks 1-2)
1. Fix user management (#439, #434)
2. Add accept-transfer setting (#429)
3. Fix console ANSI colors (#426)

### Phase 2: Modpack Sync (Weeks 3-6)
4. Modrinth API integration
5. Mod list management
6. Mod download service
7. Sync protocol implementation

### Phase 3: Production Ready (Weeks 7-10)
8. Automated backups
9. Event viewer
10. Import existing server
11. Console paging

### Phase 4: Polish (Weeks 11-12)
12. TLS/SSL auto-provision
13. Custom Java versions
14. playit.gg API update

### Phase 5: Future (Weeks 13+)
15. Docker integration
16. ARM platform support
17. Extension system (Atoms)
