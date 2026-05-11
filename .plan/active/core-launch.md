# Core Auto-Launch & Connection Plan

## What We're Building

The Amberite App should start Core automatically, maintain a stable connection to it,
recover from crashes without bothering the user, communicate securely on the same machine,
and let external players connect through Supabase without any port forwarding.

---

## Current State (What's Broken)

- Core must be started manually. App assumes it's already running.
- Connection is plain HTTP on `localhost:16662` — unencrypted, no authentication.
- Any other program on the machine can talk to Core undetected.
- No watchdog. If Core crashes, the App just shows errors with no recovery.
- Port 16662 is hardcoded. If it's taken, Core silently fails.
- External players need to know the host's IP address and set up port forwarding themselves.
- The current pairing flow (type a 6-digit code) makes no sense when the App
  itself launched Core one second ago.

---

## 1. How the App Launches Core

### Problem
The App needs to find and start the Core binary, hand it off as an independent process,
and not kill it when the App closes or crashes.

### Options

**Option A — Detached/Orphan Process**
The App starts Core like opening a new program, then immediately "orphans" it — meaning
the App detaches from it and the OS takes ownership. Core is no longer a child of the App.
If the App exits, Core keeps running.
- Pros: Simple. Cross-platform. No admin permissions needed.
- Cons: Core running indefinitely after App exit may be unexpected. Needs a lock file
  to prevent duplicate launches.

**Option B — Platform OS Service (Windows Service / macOS launchd)**
Core installs itself as a system background service. The OS manages it: starts it on
reboot, restarts it on crash, keeps it always-on.
- Pros: Maximum reliability. Survives reboots. OS does the watchdog work.
- Cons: Requires elevated/admin permissions to install. Complex setup flow. Heavy for a
  desktop app. Bad first-run UX.

**Option C — App-Embedded Watchdog (Recommended)**
The App spawns Core as a detached process (Option A) and also runs a background loop
inside itself that pings Core's `/health` every 10 seconds. If Core stops responding,
the App relaunches it. The App is the watchdog.
- Pros: No admin permissions. Works everywhere. App controls the narrative
  (can show "Core is restarting..." in the UI). No OS integration complexity.
- Cons: If the App is closed, the watchdog stops. Core can still crash and stay
  down until App restarts it on next launch.

**Decision: Option A + C combined.** Orphan process for fault isolation,
App-embedded watchdog for recovery. Good enough for v1.

### Launch Sequence

```
App starts
  └─ Look for core.lock in ~/.amberite/
       ├─ Found + PID still running + /health responds → connect, done
       ├─ Found + PID dead → stale lock, delete it, relaunch Core
       └─ Not found → spawn Core as orphan process, wait up to 20s for core.lock
```

Core binary location: bundled inside the Tauri app package, extracted to a known
path on first launch (same pattern Tauri uses for sidecar binaries).

---

## 2. The Lock File (How App Finds Core)

On startup, Core writes `~/.amberite/core.lock` with:

```json
{
  "pid": 12345,
  "port": 16662,
  "session_secret": "256-bit-random-hex",
  "local_claim_token": "256-bit-random-hex-only-valid-once"
}
```

File permissions: **owner read/write only** (0600 on Unix, private ACL on Windows).

- `pid` — lets the App confirm Core is actually running before connecting.
- `port` — the actual port Core bound to (may differ from 16662 if taken).
- `session_secret` — the password the App includes on every local request.
- `local_claim_token` — one-time token for auto-pairing (see section 5). Deleted after use.

Core deletes `core.lock` on clean exit. The App treats a stale lock (PID not running)
as a crash and relaunches Core.

---

## 3. Securing Local Communication

### Problem
Plain HTTP on localhost means any process on the machine — including malware — can send
requests to Core as if it were the App. There is no encryption and no identity check.

### Options

**Option A — Session Secret Header (Quick Fix)**
Core writes a random secret to `core.lock`. Every request from the App includes
`X-Session-Secret: <value>` in the header. Core rejects requests without it.
- Pros: Very easy to add. Works with existing HTTP setup. Stops casual attacks.
- Cons: HTTP traffic is still unencrypted inside the OS. Any process that can read the
  lock file gets the secret. Not suitable if multiple OS users share the machine.

**Option B — Private OS Channel (Named Pipe / Unix Socket)**
Instead of a network port, the App and Core communicate through a "private mailbox"
created by the OS that only exists on this specific computer. On Windows this is called
a Named Pipe. On Mac/Linux it is a Unix socket (a special file in the filesystem).
No network port is opened at all.
- Pros: Zero network exposure. The OS enforces that only local processes with the right
  path can reach it. Fastest possible local communication.
- Cons: Tauri's frontend (Vue/WebView) cannot use this — browsers only speak HTTP/WebSocket.
  We would need two channels: this for Rust-to-Rust commands, HTTP for browser-facing things.
  Platform-specific code on Windows vs Mac/Linux. Doubles complexity.

**Option C — HTTPS with a Machine-Local Certificate (Recommended Long-Term)**
Core generates its own private certificate on first run and saves it to the data directory.
The App is told to trust only that specific certificate. All communication is encrypted and
the App can prove it's talking to the right Core and not an impostor.
- Pros: Proper encryption. Works for both Rust API calls and browser WebSocket streaming.
  Standard security model with no exotic OS features.
- Cons: Certificate generation and management in Rust adds complexity. Tauri's HTTP client
  must be configured to trust the local cert. Moderate implementation cost.

**Decision: Option A now, Option C in a follow-up pass.**
Ship with session secret (Option A) to unblock the launch flow. Plan HTTPS (Option C)
as a security hardening milestone. Option B is ruled out because the browser-side
WebSocket console streaming cannot use named pipes.

---

## 4. Watchdog Details

Inside the App's Tauri backend, a long-running `tokio::spawn` task:

```
loop every 10 seconds:
  1. Read core.lock → get port and PID
  2. Verify OS confirms PID is alive (sysinfo crate)
  3. GET /health on the port
  4. If steps 2 or 3 fail three times in a row:
       a. Emit "core:offline" Tauri event → frontend shows recovery UI
       b. Delete stale core.lock
       c. Re-spawn Core binary
       d. Wait up to 20s for new core.lock to appear
       e. Emit "core:ready" Tauri event → frontend resumes
```

All Core errors in `apps/app/src/api/amberite/mod.rs` must be caught and translated to
`AmberiteError::CoreOffline` instead of panicking. The frontend must handle this state
gracefully (spinner or "Reconnecting..." banner) rather than a crash screen.

---

## 5. Ownership and First-Time Pairing

### Problem
The current pairing flow shows a 6-digit code in Core's terminal and asks the user to
type it into the App. When the App launched Core one second ago, this is pointless friction.
Also, the concern is that an external authority (Supabase) should not be able to silently
assign itself as the owner.

### Auto-Pairing for Local Launch

The `local_claim_token` in the lock file is the solution:

```
App reads core.lock → sees local_claim_token
App sends POST /setup with { claim_token, supabase_url }
Core checks: did this request come from 127.0.0.1?
  YES → accept, record owner, consume token, delete from lock file
  NO  → 403 Forbidden (external callers cannot use this path)
```

Owner identity is decided entirely on the local machine at the moment Core is launched.
Supabase is not involved in the ownership decision. The App's logged-in Supabase user ID
becomes the owner in the SQLite `core_config` table — but that assignment was made by
the App locally, not by Supabase pushing data.

### External Pairing (Headless / Remote Setup)
If Core is set up without the App (server rack, headless Linux), the existing 6-digit
terminal code flow is kept as a fallback for manual setup. This path stays untouched.

---

## 6. Core Context Awareness

A new unauthenticated endpoint `GET /status` returns:

```json
{
  "context": "local",
  "paired": true,
  "version": "0.1.0",
  "owner_id": "supabase-user-uuid",
  "capabilities": ["instances", "mods", "macros", "console"]
}
```

`context` is `"local"` when the request came from `127.0.0.1`, `"external"` otherwise.
This is determined by Axum's built-in `ConnectInfo` extractor — no config needed.

The App calls this on every connection. If `context` is `"local"` and `paired` is `false`,
it triggers auto-pairing immediately. If `context` is `"external"`, the App shows the
server owner's display name (fetched from Supabase) instead of "You."

---

## 7. Supabase as the Relay (External Access Without Port Forwarding)

### Problem
For player B to connect to player A's Core, they need A's IP address and an open port.
Most home users cannot or do not want to set up port forwarding.

### Core Registration (Heartbeat)

Every 60 seconds, Core calls a Supabase Edge Function `POST /core-heartbeat`:
```json
{ "core_id": "uuid", "address": "1.2.3.4:16662", "version": "0.1.0" }
```
This is signed with the Core's Supabase JWT so only a legitimate paired Core can register.
Supabase stores this in a `cores` table. Records older than 3 minutes are considered offline.

### Player Discovery

When the App needs to connect to a friend's Core, it asks Supabase `GET /find-core?owner_id=X`.
Supabase returns the last known address and whether the Core is currently online.

### Two Connection Modes

**Mode 1 — Direct Connection**
App tries to connect directly to the `address` from Supabase. Times out after 3 seconds.
Works if the host has UPnP or has manually forwarded port 16662. Fast. No relay cost.
Core should attempt UPnP auto-port-forward on startup using the `igd` Rust crate.

**Mode 2 — Supabase Relay (fallback, no port forwarding needed)**
If direct fails, App and Core both connect outbound to a Supabase Realtime channel
identified by the `core_id`. All traffic is piped through Supabase.
Core never needs an open inbound port. Works behind any firewall or carrier-grade NAT.
- Trade-off: Higher latency (~50–150ms added). Console log streaming (high-frequency)
  may feel sluggish. Read-only operations (status, mod list) are unaffected.
  Start/stop commands and console input tolerate the latency well.

The App tries Mode 1 first on every session. It falls back to Mode 2 silently.
A small icon in the UI indicates which mode is active.

---

## 8. Changes Required

| File | What Changes |
|---|---|
| `apps/core/src/main.rs` | Write/delete `core.lock` on start/exit. Port auto-retry on bind failure. |
| `apps/core/src/config.rs` | Add `session_secret`, `local_claim_token` fields populated at startup. |
| `apps/core/src/presentation/handlers/setup.rs` | Add localhost-only auto-pair path using claim token. |
| `apps/core/src/presentation/handlers/diagnostics.rs` | Add `GET /status` with context + capabilities. |
| `apps/app/src/api/amberite/mod.rs` | Read lock file for URL + secret. Remove hardcoded fallback. |
| `apps/app/src/main.rs` | Spawn Core binary on startup. Start watchdog task. |
| New: `apps/app/src/core_manager.rs` | `CoreManager`: launch, watch, restart, read lock file, auto-pair. |
| Supabase | New `core-heartbeat` Edge Function. New `cores` table. New `find-core` Function. |

---

## 9. Implementation Order

1. **Lock file write/read** — Core writes it, App reads port from it. Removes hardcoded URL.
2. **App launches Core binary** — Tauri sidecar or bundled binary path.
3. **Session secret** — App sends header, Core validates. Blocks unauthorized local callers.
4. **Auto-pairing via claim token** — No more manual 6-digit code for local setups.
5. **`GET /status` endpoint** — App knows local vs external context immediately.
6. **Watchdog loop** — Health check, relaunch on failure, frontend recovery UI.
7. **UPnP auto-port-forward** — Best-effort open on startup before registering with Supabase.
8. **Core heartbeat to Supabase** — Enables external player discovery.
9. **Relay fallback** — Zero-config multiplayer. Players with no port forwarding can join.
10. **HTTPS on localhost** — Security hardening. Replace session secret with cert-pinning.
