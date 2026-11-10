# Architecture: one engine, one protocol

> Temporary design doc for the restructure. None of this is built yet: `packages/contracts`,
> `packages/engine` and `Engine` do not exist today. Once the restructure lands, real docs replace
> this page. Until then, AGENTS.md and `shared-rust.md` describe the code as it is.

This page records decisions, the reasons behind them, and what was rejected. Exact code shapes are
for the implementation and review to decide, as long as they keep these decisions.

## Why

The App and the Core do the same things to an instance: install content, edit files, read logs,
publish and follow shares. Today they do it with two codebases, two APIs and four definitions of
the protocol.

- Copal may not depend on app-lib, so Core rebuilt instances in app-lib's layout
  (`apps/core/src/state/servers`, about 10k lines) next to app-lib's `state/instances`. Every
  content or sharing fix is written twice, and upstream changes reach Core only by hand.
- `packages/bedrock` (about 6k lines) exists only to share pieces across that rule.
- The App exposes Tauri commands and the Core exposes Archon-shaped HTTP routes for the same
  operations.
- The protocol is defined in Rust in sharing-client, in hand-written TS in the backend, in
  generated TS in api-client, and in upstream's `shared-instances`. Clients are hand-written per
  route.
- The browser bridge (`packages/browser-bridge`) exists only because the UI talks to the App
  through `invoke`, so agents cannot use a plain browser tab.

## Rules

What a review checks a change against.

- The engine never imports a host (`apps/app`, `apps/core`). Shared logic goes in the engine, host
  features go in the host. The compiler enforces the direction.
- The engine tells, it never asks. It takes what it needs as arguments and reports through events.
  If engine code needs a lot of desktop data, it belongs in the App.
- No global state. The host creates `Engine` and passes it in.
- Every route we own is an `Endpoint` in contracts. No hand-written request or response types in
  any language, and no per-route hand-written clients.
- Engines never call each other.
- A Core never has desktop code paths or desktop tables.
- Upstream's frontend helpers keep their names and signatures. Only their bodies change.
- One way to do each thing. Desktop features are App routes in the native window and in a browser
  tab alike, not a dev-only second path.

## Packages

| Package                                | Role                                                                                                                                                                                                                                                                                                                                 |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `packages/contracts`                   | The protocol. Rust types and `Endpoint`s (method, path, request, response) for every route we own, plus `ApiError`, `Event` and the protocol version. ts-rs generates the TS, committed and checked in CI. The `client` feature holds the generic Rust caller `call::<E>()` (plus a streaming variant for file downloads). No logic. |
| `packages/engine`                      | Upstream's `packages/app-lib`, renamed. Owns instances on one machine: content, files, worlds, logs, game install and launch, sharing. Defines `Engine` and serves the engine API as an axum router. Knows nothing about desktops or servers.                                                                                        |
| `packages/sharing-client`              | Sharing logic: building a version, the install plan, the diff, crypto, the pool. Calls the backend and stores through `call::<E>()`. Its own crate because the store needs it without the engine.                                                                                                                                    |
| `packages/sharing-storage`             | The store. Serves `contracts::store`. Depends only on contracts and sharing-client. A separate process from Core on purpose: small, isolated, and not part of the Core.                                                                                                                                                              |
| `packages/modrinth-content-management` | Upstream, unchanged.                                                                                                                                                                                                                                                                                                                 |
| `apps/app`                             | The desktop host. Creates the engine, owns the desktop domains and desktop features, serves everything on 127.0.0.1, and runs the Core proxy.                                                                                                                                                                                        |
| `apps/core`                            | The server host. Creates the engine, adds server features, and serves everything over HTTPS.                                                                                                                                                                                                                                         |
| `apps/backend`                         | Serves `contracts::backend`, with handlers typed by the generated TS.                                                                                                                                                                                                                                                                |
| `packages/api-client`                  | The TS client. A generic `call(Endpoint, body)` with `client.local`, `client.core(id)` and `client.backend`, plus upstream's Modrinth modules.                                                                                                                                                                                       |
| `apps/app-frontend`, `packages/ui`     | The UI. Every call goes through api-client.                                                                                                                                                                                                                                                                                          |

Rust imports go one way: `contracts` <- `sharing-client` <- `engine` <- `apps/app`, `apps/core`.
`sharing-storage` imports `contracts` and `sharing-client`.

## The engine

`Engine` (today's `State`, renamed so it does not clash with axum's `State`) holds what one running
engine keeps open: folders, the `engine.db` pool, the content store, the process manager, instance
locks, the file watcher, and the download and disk limits.

There is no global. The host creates the engine with its own config and passes it to every
handler:

```rust
// apps/app/src/main.rs
let engine = Arc::new(Engine::new(EngineConfig { data_dir, .. }).await?);
let state = AppState { engine, accounts, friends, discord, db };
Router::new().merge(engine::routes()).merge(app_routes()).with_state(state);
```

- app-lib's `api/` layer becomes the HTTP handlers, so its `State::get()` calls (about 255 of 281)
  go away with it. The rest (events, install jobs, process, cache) take the engine as an argument
  or hold a clone of the `Arc<Engine>` they were started with.
- Each host starts the engine its own way. Core's start runs nothing desktop: no Discord, friends
  socket or Modrinth sign-in.

**Client and server.** Install and launch take a side. The client gets client.jar, assets and
natives and launches with the credentials the host passes in
(`launch(instance, side, credentials)`; Core passes none). The server gets the server jar and the
loader's server install. Client-only content is skipped on a server by a filter in the install
loop on the content's side, using the types that already exist. Console, backups, properties and
server-only steps such as `apply_pending_on_stop` stay in Core.

## Hosts

Each host owns one state struct, defined once, holding the engine plus its own features:

| Host        | State       | Owns                                                                                                                                                                                  | Database  |
| ----------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- |
| `apps/app`  | `AppState`  | Minecraft and Modrinth sign-in, skins, friends, Discord presence, screenshots, synced options, desktop features, the Core proxy                                                       | `app.db`  |
| `apps/core` | `CoreState` | Access and sessions, pairing, process supervision and restarts, console, backups, server properties, EULA, networking and port mapping, the daemon and self-update, store supervision | `core.db` |

Every machine has `engine.db`. Each database has its own migrations, so a desktop table never
exists on a Core. Nothing joins across databases: a host table refers to an instance by id, and
the host cleans up its own rows when the engine reports an instance deleted.

The desktop domains move out of app-lib into `apps/app`, one folder each (`apps/app/src/accounts`,
`friends`, ...), not into a package, since nothing else imports them. They react to the engine
through its events: friends and Discord presence follow process started and stopped. A new desktop
feature is a new folder and a new field in `AppState`, with no engine change.

## The protocol

Every route we own is an `Endpoint` in contracts: `engine`, `app`, `core`,
`backend::{accounts, social, cores, sharing}` and `store`. A shape is written once in Rust and
ts-rs writes the TS copy. CI fails when the copy is stale.

- **Rust callers** use `call::<E>()`: the engine and the store calling the backend and stores, and
  the Core calling the backend. This replaces sharing-client's hand-written backend functions,
  `pool/store.rs`, Core's `access/sync.rs` and `pairing.rs`, and the store's raw reqwest in
  `keys.rs`.
- **TS callers** use api-client. The backend types its handlers with the same generated TS.
- **Errors** are `ApiError` codes, not message text. The UI switches on the code (for example a
  start that fails because the EULA is not accepted).
- **Progress** for long operations (the commands that report progress through Tauri today) goes
  out as events.

**Names.** Contracts uses upstream's names where upstream has the concept: shared instance,
`InstanceVersion`, `ExternalFile`, `InstanceUser`, `Invite`. Our additions get plain names, not
jargon like `PoolGrant` and `StoreGrant`, and comments stop using single letters for hashes (R, P, C)
that mean nothing on their own. Backend table and column names follow the contract names
(`shared_instance_users`, not `shared_members`).

**Protocol version.** Cores run on other people's machines and can be older or newer than the
App. Copy T3 Code's handshake (in its repo: `packages/contracts/src/environment.ts`,
`packages/client-runtime/src/connection/compatibility.ts` and `outdatedHostUpdate.ts`), adjusted
to us:

- One integer in contracts, bumped only when old and new sides can no longer talk. Releases do
  not touch it.
- The Core reports it when the App connects, and the App compares. A Core that reports none counts
  as version 1.
- Core too old: the owner gets "Update this Core" through the existing self-update, and a member
  gets "Ask the owner to update". App too old: "Update Amberite".
- The backend always runs the latest, so it needs no check.

### Who calls whom

| Caller              | Client                           | Protocol                                 | Server                          |
| ------------------- | -------------------------------- | ---------------------------------------- | ------------------------------- |
| UI                  | `client.local`                   | `contracts::engine` + `contracts::app`   | The App on 127.0.0.1            |
| UI                  | `client.core(id)`                | `contracts::engine` + `contracts::core`  | A Core, through the App's proxy |
| UI                  | `client.backend`                 | `contracts::backend`                     | `apps/backend`                  |
| UI                  | upstream's labrinth modules      | Modrinth's                               | Modrinth                        |
| App desktop domains | `call::<E>()`                    | `contracts::backend::{accounts, social}` | `apps/backend`                  |
| Engine              | `call::<E>()` via sharing-client | `contracts::backend::sharing`            | `apps/backend`                  |
| Engine              | `call::<E>()` via sharing-client | `contracts::store`                       | Any Core's `/store/`            |
| Core                | `call::<E>()`                    | `contracts::backend::cores`              | `apps/backend`                  |
| Store               | `call::<E>()` via sharing-client | `contracts::store`, `/core/keys.json`    | Other stores, `apps/backend`    |
| Core and its store  | stdio JSON lines + local HTTP    | private to sharing-storage               | `copal store`                   |

Engines never call each other. A share moves through the backend (what a version is) and the
stores (the bytes), so a member can install while the publisher's App is closed.

## The Core proxy

A Core's certificate is self-signed, and a WebView cannot pin a certificate. The App's existing
proxy (`apps/app/src/api/cores/proxy.rs`) stays and carries UI calls to Cores:

- It is a pipe. It knows no routes, so a new Core route needs no App change. It carries WebSockets
  too.
- `client.core(id)` gets a base URL on the App's local address, with the same Endpoints as calling
  the Core directly.
- Core session tokens never enter the WebView. The proxy adds them on the way out.
- The fingerprint check is written once in Rust and shared by the proxy, the pool fetch and the
  stores.

## Access

```rust
pub trait Access {
    fn caller(&self, req: &Parts) -> Result<Caller, ApiError>;
    fn require(&self, caller: &Caller, instance: InstanceId, p: Permission) -> Result<(), ApiError>;
}
```

The host provides it when it builds the router. `AppAccess` accepts requests carrying the
loopback secret and allows everything. `CoreAccess` maps the session token to an account and
checks owner, Editor or Viewer. Nothing in a request can change which one runs. Share permissions
belong to the backend, the same for both hosts.

The App's local server is reachable by any program on the machine, and by websites through the
user's browser. Account for that the way the Core proxy already does (a secret in the path, the
`Origin` checked); it is a small amount of code, not a project.

## Events

Each host serves one WebSocket. The client subscribes to topics (an instance, a server's console,
install progress) and the host checks `Access` at subscribe time, and again when the caller's role
changes or their session is revoked or expires, ending what they may no longer see, as Core's
console stream does today. A Viewer never gets a console,
because it carries chat and player IPs. Every event is a variant of `Event` in contracts, so a new
live feature adds a variant, not a new socket. This replaces the Tauri `Channel` in
`app-lib/src/event` and Core's per-server socket.

## Browser tabs and dev

The UI runs fully in a plain browser tab, so agents and tests need no bridge.

- Engine calls, App routes and desktop features (dialogs, opener, updater, shortcuts) are all
  HTTP. The App's Rust performs the native action. A dialog started from a tab opens on the
  desktop.
- Only window chrome (minimize, maximize, close, dragging) stays on Tauri. It does nothing in a
  tab, because a tab has no window.
- The native window uses `TauriModrinthClient`, which sends Modrinth the App's User-Agent. A tab
  uses `GenericModrinthClient` with plain `fetch`.
- The backend's `CORS_ORIGINS` and the App's local server allow the dev origin.
- Dropping files from the desktop with their real paths works only in the native window.
- The dev runner opens the UI in a plain tab, and the `test-app` skill drives that tab. A
  scenario's data holds `engine.db` and `app.db`, copied together.

## Upstream

Upstream's code keeps merging. `packages/engine` is upstream's `packages/app-lib`, renamed, and
merges with `merge.directoryRenames=true`. A test merged 86 real upstream app-lib commits onto the
renamed folder with zero content conflicts. Rename the crate too (`theseus` to `engine`). If that
turns out to cause real merge pain, keeping `theseus` as the crate name is acceptable; the folder
rename stays.

`docs/internals/upstream.md` holds the rules an agent applies to upstream changes:

- Upstream adds an `api/` function, a `#[tauri::command]` and a helper calling `invoke`: add the
  `Endpoint` in contracts and a handler in the engine that takes `Engine`, and change only the
  helper's body to call `client.local`.
- Upstream adds an event: add an `Event` variant on the socket.
- Upstream adds a plugin call: add an App route.
- Upstream changes a desktop domain: port it into that domain's folder in `apps/app`, and its
  migrations into the App's migrations.
- Upstream's Archon calls in our screens go to the engine API.

Upstream's `archon` and `shared-instances` modules in api-client stay untouched for upstream's
own pages. Our screens, including the hosting panel for Cores, use our API.

## What goes away

The restructure is done when these are gone:

- `packages/bedrock`, its upstream clones and `ports.toml`. `limits` move to contracts,
  `disk::free` and `CoreAddress` to sharing-client, and the borrowed files back to the engine.
- The duplicated content, files and sharing half of `apps/core/src/state/servers`.
- Core's Archon-shaped file, content, log and share routes, and api-client's `copal` module.
- The second copy of sharing: app-lib's `api/instance/shared` and Core's share commands become one.
- The hand-written clients listed under The protocol, and the backend's hand-written TS types.
- `packages/browser-bridge`, the App's Tauri commands for engine and desktop features, and the
  Tauri `Channel` for events.
- The rule "Copal never depends on app-lib" in AGENTS.md, and `shared-rust.md`, replaced by real
  docs for this design.

## Rejected

Do not propose these again without new information.

- **Per-Core certificates or public hostnames.** Certificate rate limits, every Core's home IP in
  public DNS and certificate logs, and LAN Cores breaking. The proxy already works.
- **A global `State`, one combined state struct, or a generic `State<Host>`.** They let host code
  leak into the engine and hide what a function needs.
- **Desktop features over HTTP only in dev, with IPC in the native window.** Two paths drift, and
  agents would test the one users don't run.
- **Desktop domains in their own package.** Only the App imports them.
- **Engine-to-engine calls.** The publisher's App is often closed and never reachable.
- **A Core check after every upstream merge,** for now. Part of the upstream sync process if it is
  ever needed.

## Watch

- **Console backpressure.** A busy console shares the event socket with everything else. Once it
  is live, check whether console bursts delay other events. If they do, give each subscription a
  bounded buffer that drops old console lines.

## Unproven

Core creating an `Engine` with its own folders and running `list_content` on a server has not
been tried. app-lib was written for one engine per desktop App, so anything that assumes that
shows up there first. Try this before anything else.
