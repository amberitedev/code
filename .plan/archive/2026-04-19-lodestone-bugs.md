# Lodestone Core Bug Report

**Status:** Phase 0 — Bug Audit  
**Date:** April 11, 2026  
**Scope:** Lodestone Core (Rust/Axum backend)

---

## 🔴 Critical Bugs (Broken Core Functionality)

### 1. Cannot add users even as owner (#439)
**Severity:** Critical  
**Status:** BROKEN - Missing API endpoints

**Description:**  
The backend has user registration logic in `auth_service.rs` and `sqlite_repo.rs`, but no API routes expose this functionality. Admins cannot create additional users through the dashboard.

**Root Cause:**  
- `auth_service.rs` has `register()` method (line 65)  
- `sqlite_repo.rs` has `create_user()` implementation (line 101)  
- **BUT:** No API route exposes this functionality  
- `router.rs` only has `/login` and `/setup` — no `/users/register` or `/users/invite`

**Expected Behavior:**  
Admin users should be able to create additional users via API endpoints with proper permission checks.

**Fix Required:**  
Add `POST /users/register` and `POST /users/invite` endpoints with admin permission checks via `AuthExtractor` middleware.

---

### 2. Core re-runs first-time setup on restart (#434)
**Severity:** Critical  
**Status:** BROKEN - Setup doesn't check existing admin

**Description:**  
After Lodestone Core is installed and configured, users report being prompted to go through setup again on next launch — losing their existing configuration state.

**Root Cause:**  
- `auth_api.rs:setup()` (line 67) creates admin user without checking if one exists  
- No `GET /setup/status` endpoint to check if setup is complete  
- Frontend has no way to know setup was already done

**Expected Behavior:**  
Setup should only run once. Subsequent calls should return an error indicating setup is already complete.

**Fix Required:**  
Add `GET /setup/status` endpoint that queries the database to see if any user exists. Update `POST /setup` to return 403 Forbidden if an admin user already exists.

---

### 3. Console loses all ANSI color formatting (#426)
**Severity:** Medium-High  
**Status:** BROKEN - No ANSI preservation

**Description:**  
The in-dashboard server console strips ANSI escape codes, displaying raw text without any color. This makes reading Minecraft server logs much harder since warn/error levels are indistinguishable from info.

**Root Cause:**  
- `instance_actor.rs` emits `ConsoleLine(String)` events (line 36)  
- `process_spawner.rs` spawns process with `sh -c` but doesn't allocate PTY  
- No ANSI escape code handling or preservation  
- Minecraft server likely strips colors when not running in proper terminal

**Expected Behavior:**  
Console should preserve ANSI color codes for proper log formatting (red for errors, yellow for warnings, etc.).

**Fix Required:**  
Implement PTY allocation or ANSI passthrough, preserve escape codes in WebSocket streaming.

---

## 🟡 Medium Bugs (UX Issues)

### 4. Console opens scrolled to top instead of bottom (#425)
**Severity:** Medium  
**Status:** FRONTEND ISSUE - Not in Core

**Description:**  
When the console panel is first opened, it shows the oldest log entries at the top instead of auto-scrolling to the most recent output at the bottom. The user has to manually scroll down every time.

**Root Cause:**  
- Core only broadcasts console events via WebSocket (`websockets.rs`)  
- Scroll behavior is Vue component responsibility  
- No `scrollToBottom()` call in Logs.vue `onMounted()` hook

**Expected Behavior:**  
Console should auto-scroll to the bottom when first opened and when new log entries arrive.

**Fix Required:**  
In panel `Logs.vue`, add `scrollToBottom()` in `onMounted()` lifecycle hook.

---

### 5. No error toast when instance creation fails (#430)
**Severity:** Medium  
**Status:** PARTIAL - Core has error handling, Frontend doesn't use it

**Description:**  
If creating a new game server instance fails (e.g. bad JAR, network error, insufficient disk), the dashboard shows no notification. The user has no idea what went wrong or that anything failed.

**Root Cause:**  
- Core has full error handling (`error.rs`, `ApiError` types)  
- `instance_api.rs` returns proper error responses (lines 31, 54)  
- Panel has `error.js` store but not properly catching/handling API errors

**Expected Behavior:**  
When instance creation fails, an error toast should appear with the error message.

**Fix Required:**  
Frontend error handling in instance creation flow to catch API errors and trigger `error.js` store.

---

### 6. Missing description for "accept-transfer" setting (#429)
**Severity:** Low  
**Status:** MISSING - Not in schema

**Description:**  
The accept-transfer Minecraft server property has no explanatory tooltip or description in the settings panel, leaving users unsure what enabling it does.

**Root Cause:**  
- `server_properties.rs` macro defines ~20 properties  
- `accept-transfer` is NOT listed in the `generate_server_properties!` macro

**Expected Behavior:**  
All server properties should have descriptive tooltips explaining their purpose.

**Fix Required:**  
Add `(AcceptTransfer, "accept-transfer", bool, "Accept player transfers from other servers")` to the `generate_server_properties!` macro in `infrastructure/server_properties.rs`.

---

### 7. playit.gg tunnel does not auto-start with server (#433)
**Severity:** Medium  
**Status:** MISSING - No lifecycle integration

**Description:**  
The playit.gg integration tunnel must be manually re-started each time the Lodestone Core restarts. There's no persistent option to have the tunnel automatically come up alongside the server.

**Root Cause:**  
- `PlayitTunnelManager` exists (networking.rs)  
- Only has `create_tunnel()` method  
- NOT integrated with `InstanceActor` lifecycle  
- `handle_start()` doesn't call tunnel manager

**Expected Behavior:**  
When a server starts, the playit.gg tunnel should automatically provision. When the server stops, the tunnel should be cleaned up.

**Fix Required:**  
Inject `PlayitTunnelManager` into `InstanceActor`, call `create_tunnel()` in `handle_start()` and cleanup in `handle_stop()`.

---

### 8. playit.gg integration out of date (#431)
**Severity:** Medium  
**Status:** OUTDATED - Using old API

**Description:**  
The playit.gg integration was built against an older version of the playit.gg API/agent. Newer versions of the playit.gg client have changed behavior, breaking or degrading the integration.

**Root Cause:**  
- Uses `https://api.playit.gg/v1/tunnel` (networking.rs:68)  
- Current playit.gg API is v2  
- No error handling for API changes

**Expected Behavior:**  
playit.gg tunnel should work with the latest API version and handle errors gracefully.

**Fix Required:**  
Update to v2 API, add proper error handling for API response changes.

---

## 🟢 Low Priority Bugs (Platform/Support Issues)

### 9. Intel Mac support deprecated
**Severity:** Low  
**Status:** DEPRECATED BY DESIGN

**Description:**  
The team deprecated Intel macOS support due to lack of test hardware. Only Apple Silicon is officially supported on macOS.

**Root Cause:**  
- No Intel-specific code found in Core  
- Likely a build configuration issue  
- No Intel Mac build targets in CI/CD

**Expected Behavior:**  
Intel Macs should be able to build and run Lodestone Core.

**Fix Required:**  
Add Intel Mac build targets, find community tester for macOS 10.12 Intel.

---

### 10. Browser "Mixed Content" / "Insecure Content" warning
**Severity:** Low  
**Status:** EXPECTED BEHAVIOR - No HTTPS support

**Description:**  
Because Lodestone doesn't auto-provision a TLS/SSL certificate, browsers report mixed content when the hosted dashboard (HTTPS) connects to a local Core (HTTP). Users must manually allow insecure content or host the dashboard themselves. Safari is completely unsupported.

**Root Cause:**  
- Core serves HTTP only (no TLS in router.rs)  
- No ACME/Let's Encrypt integration  
- No HTTPS server support

**Expected Behavior:**  
Core should serve HTTPS natively with auto-provisioned TLS certificates.

**Fix Required:**  
Add HTTPS support, integrate ACME/Let's Encrypt for auto-certificate provisioning.

---

### 11. Ports must be manually opened for remote access
**Severity:** Low  
**Status:** PARTIAL - UPnP stubbed out

**Description:**  
UPnP auto-port-forward is available in settings but not guaranteed to work. Most users need to manually open their router ports, which is a major friction point for new users.

**Root Cause:**  
- `UpnpManager` exists (networking.rs:18)  
- Implementation is STUBBED: "to avoid IGD API issues" (line 35)  
- `gateway: None` always (line 28)

**Expected Behavior:**  
Core should automatically configure port forwarding via UPnP or provide clear instructions for manual port forwarding.

**Fix Required:**  
Fix IGD integration or improve manual port forwarding UX with step-by-step instructions.

---

## Summary

| Severity | Count | Status |
|----------|-------|--------|
| Critical | 3 | Broken |
| Medium | 4 | Partial/UX |
| Low | 4 | Platform/Support |

**Total bugs identified:** 11  
**Core functionality bugs:** 3  
**UX issues:** 4  
**Platform issues:** 4

---

## What's Actually Working

1. **Basic instance start/stop/kill** - Fully functional
2. **Console command sending** - Works via WebSocket
3. **Live console streaming** - Works but loses colors
4. **User login** - Works with PASETO tokens
5. **Initial setup** - Works but doesn't prevent re-setup
6. **Health check endpoint** - `/health` returns 200
7. **System stats endpoint** - `/stats` returns CPU/memory info
