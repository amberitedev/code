# Hosting → Core Rewrite Plan

## What We're Building

The server management UI currently talks to Core through a translation layer that fakes
the Modrinth cloud API. It creates a JavaScript Proxy that intercepts every `archon.*` and
`kyros.*` call, maps it to Core HTTP endpoints, and lies to the UI about the shape of the
data. It was always a stopgap. This rewrites that entire layer from scratch.

The goal: the server management UI talks directly to Core through a clean typed API client.
No Modrinth interface to implement. No proxy tricks. No faking Archon. Both the desktop app
(Tauri) and a future web dashboard can use the same components with the same client, and the
components own their own data fetching.

---

## Why the Existing Proxy Approach Was Wrong

The old `core-client.ts` and `core-ws-client.ts` were built around one idea: trick
`ServersManageRootLayout` into thinking it was talking to Modrinth's cloud API.
This created several interrelated problems:

- Every Core response had to be massaged into an Archon shape. Gaps in the mapping caused
  real bugs: mods crashed because the proxy unwrapped `{ mods: [...] }` incorrectly,
  the mod toggle sent `{ disabled: true }` when Core expected `{ enabled: false }`, and
  server crashes silently showed as "stopped" because the status map was incomplete.

- The fake WebSocket replaced `window.WebSocket` globally during component mount to bypass
  a ping check that ServersManageRootLayout performed against a cloud node address that
  doesn't exist for local servers. Patching `window.WebSocket` at the right moment in the
  component lifecycle is fragile and hard to reason about.

- Stats polling, state polling, and the actual console WebSocket connection were three
  separate code paths, two of which ran on timers instead of using the real connection.
  RAM total was hardcoded to 4 GB.

- The backups tab and files tab were entirely stubbed — they returned empty arrays and
  noops. Core had no matching endpoints, so the stubs couldn't be wired even if someone
  tried.

- The architecture was only usable from the desktop app. A web dashboard would have had to
  reimplement everything from scratch.

---

## The Architectural Direction (User's Intent)

The user asked for:

> "The server management UI talks to Core directly, through a clean typed API helper, with
> no translation layer anywhere in the stack."

> "Write it as if it was designed this way from the start. Every time you face an
> architectural decision, pick the cleanest, best long-term option."

> "Both the desktop app and the web dashboard can use these same components, plugged
> directly into core-api.ts, with no Modrinth client, no abstract interface to implement,
> no injection tricks. The components own their own data fetching."

The key shifts this implies:

1. **No `AbstractModrinthClient`.** We are not implementing an abstract interface. We are
   writing a concrete typed client for Core. There is no inheritance, no proxy, no wrapping
   of the existing Modrinth client.

2. **`packages/ui` components can have their logic changed — templates cannot.** Visual
   output, layout, styling: untouched. Script blocks, data fetching, API calls: fully
   rewritten to use the Core client directly. The components stop calling
   `injectModrinthClient()` and start calling the Core client.

3. **One WebSocket connection per server instance.** Not polling. Not a WS wrapper class.
   Core emits all live data — console lines, stats, state changes — as structured JSON
   frames on the same WebSocket. The frontend parses and routes by type. This is the
   single source of truth for all live data while a server is running.

4. **Core shapes its responses for what the UI needs.** When the UI expects a particular
   field name or shape, Core returns that shape. The frontend does not transform data.
   If a mismatch exists, fix Core, not the frontend.

---

## New Package: `packages/core-client`

`core-api.ts` needs to live somewhere that both `packages/ui` and `apps/app-frontend` can
import it. It cannot live in `apps/app-frontend` because `packages/ui` cannot depend on an
app-level package — the dependency graph goes the wrong way.

It does not belong in `packages/api-client`. That package is the Modrinth cloud API client
(Archon, Kyros, Labrinth, their cloud services). Core is an entirely separate product —
Amberite's own server manager daemon. Mixing them would destroy the conceptual clarity of
both packages.

The correct home is a new package: **`packages/core-client`**, published as
`@amberite/core-client`. This package:

- Contains the `CoreApiClient` class, all typed HTTP functions, and the WebSocket connection
  helper
- Has no Tauri-specific code — pure `fetch` and `WebSocket`, portable to any JS environment
- Is added as a dependency of `packages/ui` and `apps/app-frontend`
- Will be usable by a future web dashboard without changes

The `CoreApiClient` is a class that takes a `baseUrl` in its constructor. The desktop app
gets the URL from a Tauri invoke and creates an instance. The web dashboard will get it
differently. Either way the class is the same. A Vue provide/inject pair
(`provideCoreClient` / `injectCoreClient`) lives in `packages/ui/src/providers/` so
components can reach the client.

---

## WebSocket: Structured JSON Events

Core currently sends raw stdout text frames on the console WebSocket. This is being changed
so that all WebSocket frames are structured JSON with a `type` field:

```
{ "type": "log",   "data": "Server started on port 25565." }
{ "type": "stats", "data": { "cpu_percent": 12.5, "memory_mb": 1024, ... } }
{ "type": "state", "data": { "status": "running" } }
```

Stats are emitted on a timer (every 3 seconds) while the WebSocket is connected and the
server is running. State is emitted immediately when the instance status changes. This is
Core's responsibility — not a polling loop on the frontend.

The `CoreWsConnection` type in `packages/core-client` wraps a native `WebSocket` and
exposes typed `on(type, callback)` / `send(command)` / `close()` methods. The frontend
never touches raw WebSocket frames — it sees only typed events.

WS ticket issuance (`POST /ws-token`) is a method on `CoreApiClient`. The component that
opens the connection calls `client.issueWsTicket()` first, then `client.connectWs()`.

---

## What Gets Deleted

- `apps/app-frontend/src/helpers/core-client.ts` — the JS Proxy translation layer
- `apps/app-frontend/src/helpers/core-ws-client.ts` — the polling WS wrapper class
- The fake `window.WebSocket` intercept in `pages/server/Index.vue`
- All `provideModrinthClient(coreClient)` calls that wrap Core in the Modrinth client shape

---

## packages/ui Changes (Script Only)

Templates, styling, and user-facing behaviour are frozen. Script blocks are rewritten.

`server-manage-core-runtime.ts` is the central composable that provides `ModrinthServerContext`
to all child components. It currently calls `injectModrinthClient()` and uses the Modrinth
client for everything. The rewrite removes the Modrinth client entirely. It calls
`injectCoreClient()` instead, opens a `CoreWsConnection`, routes events to reactive refs,
and calls `provideModrinthServerContext` with the same shape it always did. The context
structure stays identical — this is how all the downstream components (Overview, Content,
Files, Backups) see their data. Only the data source changes.

Each layout component (`root.vue`, `content.vue`, `files.vue`, `backups.vue`, `overview.vue`)
and any supporting composables that currently import from `@modrinth/api-client` and call
`injectModrinthClient()` get the same treatment: read the current logic, replace each
`client.archon.*` or `client.kyros.*` call with the equivalent `CoreApiClient` method call.
The supporting composables (`server-console.ts`, `server-content-manager.ts`,
`server-files-manager.ts`, `server-backups-queue.ts`) each need to be read before touching
them — they may have significant logic that interacts with the WS event stream.

---

## apps/app-frontend Changes

`pages/server/Index.vue` becomes much simpler. It gets the base URL from a Tauri invoke,
creates a `CoreApiClient`, and calls `provideCoreClient(client)`. No fake WebSocket, no
proxy setup, no Modrinth client wrapping. The `ServersManageRootLayout` is still the
layout shell — it renders the tabs and the slot where child pages live. The data for that
shell now comes through `server-manage-core-runtime.ts` calling Core directly.

`pages/server/Console.vue` is a full rewrite with two modes: **Live** and **Logs**.

Live mode is a terminal connected to Core's console WebSocket. Log lines from the WS
connection appear in the terminal. The user can type commands. This replaces the existing
direct WebSocket code, which was correct in concept but too minimal.

Logs mode shows the list of log files (`GET /instances/:id/logs`) and crash reports
(`GET /instances/:id/crash-reports`). Clicking a file loads its content inline. The page
auto-switches to logs mode when the WebSocket disconnects and auto-loads `latest.log` (or
the most recent crash report if the server crashed). This is new functionality — the old
console page had no logs view at all.

The file must stay under 200 lines. The design constraint forces a clean separation: state
management is minimal, the WS connection logic is delegated to the `CoreApiClient` helper,
and the template is straightforward.

---

## New Core Endpoints

Two full APIs need to be built in Core before the frontend can wire them up.

### File System API

The files tab needs a real file manager API. Core must expose endpoints to list directory
contents, download files, create and update files and directories, move and rename items,
delete items, and extract archives (for the mrpack install flow).

Every single endpoint must validate the requested path before touching the filesystem.
The path is resolved against the instance's `data_dir` and checked to confirm it does not
escape that directory. Any path that resolves outside the instance directory returns 403,
not a filesystem error. This is not optional — arbitrary path traversal in a server manager
is a serious security hole.

Directory listing must return exactly the shape the files UI component expects
(`{ items, total, current }` with `total` being pages not items). Sort order: directories
before files, both alphabetical.

File upload must support real progress reporting. The `CoreApiClient` `uploadFile` method
returns an `UploadHandle` with progress callbacks. XHR (not `fetch`) is used because only
XHR exposes upload progress events.

Archive extraction supports a dry-run mode that returns a list of conflicting files without
writing anything. This is used by the UI to prompt the user before overwriting.

### Backup API

The backups tab needs a full backup system. Core must expose endpoints to list backups,
create a backup, delete one or many, restore from a backup, and manage a cron schedule.

**Route ordering is critical**: the `delete-many` and `schedule` literal string routes must
be registered before the `/:bid` dynamic route or Axum will match those strings as backup
IDs. This is a known Axum footgun with path segments that look like parameters.

A backup is a ZIP of the entire instance `data_dir`, excluding `logs/` and `crash-reports/`
(those are not part of the playable server state). Backups are stored at
`{AMBERITE_DATA_DIR}/backups/{instance_id}/{uuid}.zip`.

Restore is not just a file swap. The restore flow:
1. Creates an automatic pre-restore backup of the current state first
2. Stops the instance if it's running
3. Extracts the archive
4. Restarts the instance if it was running

The cron scheduler is a background tokio task that wakes every minute, checks the
`backup_schedules` table, and fires automated backups when due. When automated backups
exceed `retain_count`, the oldest non-locked automated backups are deleted. Scheduled backup
tasks must be cleaned up when an instance is deleted.

The response shape for `GET /instances/:id/backups` must match exactly what the backups UI
component expects (`BackupsQueueResponse` with `active_operations` and `backups` arrays).
Read the backups component source before designing this shape.

Check `Cargo.lock` for the `cron` crate before adding it as a dependency — it may already
be present as a transitive dependency.

---

## File Size Discipline

All new Core files must stay under 200 lines. If an implementation grows beyond that, split
it into focused modules. This is not an arbitrary limit — it forces the implementation to
stay modular and prevents the accumulation of a single sprawling service file that becomes
hard to navigate.

---

## Error Policy

No silent failures. Every API method throws a `CoreApiError` on non-2xx responses. No
fallback empty arrays, no `|| []` rescue expressions in the API layer. The UI components
are responsible for handling errors — the client layer just surfaces them cleanly.

---

## Implementation Order

The order matters because some frontend work depends on Core endpoints existing.

1. **`packages/core-client` package** — types, HTTP functions, `CoreApiClient` class, WS
   connection helper. This unblocks all frontend work.

2. **Core: structured WebSocket events** — change the console WS handler to emit JSON
   frames. Stats on a timer, state on transitions, log lines wrapped. This unblocks the
   runtime composable rewrite.

3. **Core: FS API** — the 7 filesystem endpoints with path traversal guard. This unblocks
   the files tab.

4. **Core: Backup API** — migration, domain types, service, handlers, routes. This unblocks
   the backups tab.

5. **`packages/ui` script rewrites** — `server-manage-core-runtime.ts` first (it provides
   the context everything else depends on), then `root.vue`, then content/files/backups
   layout scripts and their supporting composables.

6. **`apps/app-frontend` rewrites** — `Index.vue` (simplified), `Console.vue` (full
   rewrite with live/logs modes).

7. **Delete** `core-client.ts` and `core-ws-client.ts` after confirming nothing imports
   them.

---

## Known Gaps at Planning Time

- The supporting composables in `packages/ui` (`server-console.ts`,
  `server-content-manager.ts`, `server-files-manager.ts`, `server-backups-queue.ts`) were
  not fully read during planning. Each must be read in full before rewriting. They likely
  have non-trivial logic interacting with the WS event stream or composing multiple API
  calls — understand that logic before changing anything.

- The `server-manage-core-runtime.ts` composable is 444 lines. Rewriting it will require
  careful reading. The uptime ticker, stale stats watchdog, and operation management logic
  are all entangled with the WS event handling. These behavioral details must be preserved
  exactly — only the data source changes.

- `packages/ui/src/layouts/wrapped/hosting/manage/root.vue` uses TanStack Query for server
  data. The TanStack Query dependency stays (it's useful for caching and refetch logic), but
  the query function changes from `client.archon.servers_v0.get(serverId)` to
  `coreClient.getInstance(serverId)`.
