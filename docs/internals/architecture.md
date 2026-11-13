# Architecture

> For maintainers and agents.

The App and a Core do the same things to an instance: install content, edit files, read logs,
publish and follow shares. They do it with one engine and one protocol. This page records the
decisions behind that design, the reasons for them, and what was rejected. The code holds the
details.

Some of the design is still landing. Until each slice below merges, its line is true and the text
that contradicts it is not:

- Until D5 to D7: a Core keeps content, sharing, server installs and files in `apps/core`, keeps
  `copal.db`, and does not mount `engine::routes`.
- Until D9: the Core's server routes keep Archon's shapes. Until D10: `packages/bedrock` and
  `shared-rust.md` exist.

## Rules

A review checks a change against these.

- The engine never imports a host (`apps/app`, `apps/core`). Shared logic goes in the engine. Host
  features go in the host. The compiler enforces the direction.
- The engine tells, it does not ask. It takes what it needs as arguments and reports through
  events. If engine code needs a lot of desktop data, it belongs in the App.
- No global state. The host creates the `Engine` and passes it in.
- Every route we own is an `Endpoint`. No hand-written request or response types in any language,
  and no hand-written client per route.
- Engines never call each other.
- A Core has no desktop code paths and no desktop tables.
- Upstream's frontend helpers keep their names and signatures. Only their bodies change.
- One way to do each thing. A desktop feature is an App route, in the native window and in a
  browser tab alike, not a dev-only second path.

## Crates import one way

Rust imports go `contracts` <- `sharing-client` <- `engine` <- `apps/app`, `apps/core`.
`sharing-storage` imports `contracts` and `sharing-client`. `contracts` imports only upstream crates
(`ariadne` for the friends socket, `modrinth-content-management`).

- `packages/contracts` is the protocol: the `Endpoint` machinery, `ApiError`, `PROTOCOL_VERSION`,
  limits, share rules, and every type we own outright (backend, Core, store). It holds no logic.
  Its `client` feature is the Rust caller `call::<E>()`, and its `server` feature is the axum glue
  (`route::<E>()`, `Req<E>`, `IntoResponse for ApiError`).
- `packages/engine` is upstream's `packages/app-lib`, renamed. It owns the instances on one
  machine and serves them as an axum router. It knows nothing about desktops or servers.
- `packages/sharing-client` is the sharing logic: versions, the install plan, the diff, crypto,
  the pool and the TLS pin. It is its own crate because the store needs it without the engine.
- `packages/sharing-storage` is the store, a separate process from the Core on purpose: small, with
  no identity, keys or database of its own.
- `packages/api-client` is the one TypeScript client. Every UI call goes through it.

Desktop domains (accounts, skins, friends, Discord, synced options, screenshots, settings,
onboarding, hidden instances) live in `apps/app/src/<domain>`, not in a package, because only the
App imports them. A new desktop feature is a new folder in the App and needs no engine change.

## The engine has no global

`Engine::new(EngineConfig)` ([`state/mod.rs`](../../packages/engine/src/state/mod.rs)) opens one
engine on the folders the host names. Its background tasks hold a `Weak` and end when the host
drops the last `Arc`, so tests run two engines side by side.

- The host resolves every path. The engine reads no environment variables for paths;
  `THESEUS_CONFIG_DIR` is read only by the App ([`data_dir.rs`](../../apps/app/src/data_dir.rs)),
  which also finishes a data-directory move and backs up its databases before the engine starts.
- Side is a property of the engine (`EngineConfig.side`). An App engine holds client instances and
  a Core engine holds server instances. One engine never holds both.
- The host sets the Modrinth session and one backend credential: the user's session on the App,
  the Core key on a Core. A Core calls Modrinth anonymously.
- Process resources stay process-wide: the Modrinth rate limit, its reqwest clients and user agent,
  and the icon and DNS semaphores. One process has one network identity. Anything keyed by an
  instance or a job lives on the engine.
- A launch takes its credentials, launch defaults and game options from the App
  ([`launch.rs`](../../apps/app/src/launch.rs)). A Core does not launch through the engine: the
  engine installs by side and returns the server's Java command, and the Core's supervisor runs it.
- The engine reaches its host only through `InstanceHooks` (`EngineConfig.hooks`), for the few
  orderings an event cannot express, such as capturing the user's options before a modpack update
  overwrites them. Everything else the App does in response to the engine follows engine events in
  [`reactions.rs`](../../apps/app/src/reactions.rs). A Core passes no hooks.

## Each host owns its state and its database

| Host        | State       | Owns                                                                                                                 | Database  |
| ----------- | ----------- | -------------------------------------------------------------------------------------------------------------------- | --------- |
| `apps/app`  | `AppState`  | The desktop domains, desktop features (dialogs, opener, updater), long operations, and the Core proxy                | `app.db`  |
| `apps/core` | `CoreState` | Access and sessions, pairing, process supervision, console, backups, properties, EULA, networking, the store, update | `core.db` |

Every machine also has `engine.db`, which holds only engine tables. Each database has its own
schema, so a desktop table never exists on a Core. Nothing joins across databases: a host row
refers to an instance by id, and the host deletes its rows when the engine reports the instance
removed. The one shared table is the share history, whose `SCHEMA` sharing-client owns and the
engine applies to `engine.db` at open, so the table has one definition.

Nothing is migrated. The schemas are fresh, a data folder from before them stops opening, and dev
data is reset rather than converted. The sqlx regeneration commands are in the comment in
[`packages/engine/.cargo/config.toml`](../../packages/engine/.cargo/config.toml).

## One protocol

An `Endpoint` ([`contracts/src/lib.rs`](../../packages/contracts/src/lib.rs)) is a unit struct per
method and path with its `Params`, `Query`, `Body` and `Response`. ts-rs writes the TypeScript, which
is committed and kept fresh by each crate's tests. All generated files are re-exported by
`@amberite/contracts` (`packages/contracts/ts/index.ts`), so an exported name must be unique across
them.

Endpoints are declared where their types are:

- Backend, Core-native and store Endpoints are in `packages/contracts`, generated to
  `ts/contracts.ts`.
- Engine Endpoints are in [`packages/engine/src/endpoints`](../../packages/engine/src/endpoints/mod.rs),
  generated to `ts/engine.ts`. The engine's wire types are upstream's (`Instance`, `ModLoader`,
  `InstallJobSnapshot`), and contracts sits below the engine, so contracts cannot name them. Moving
  them would rewrite upstream files on every sync.
- App Endpoints are in [`apps/app/src/server/endpoints`](../../apps/app/src/server/endpoints/mod.rs),
  generated to `ts/app.ts`.

A route whose body or answer carries a host filesystem path is an App route, never an engine route.
A Core serves the engine's routes to remote users, so an engine route that took a path would hand
them the Core's disk. Engine routes take paths relative to an instance and check them at the route
edge ([`routes/files.rs`](../../packages/engine/src/routes/files.rs)).

Errors are one `ApiError { error, description }` on the wire
([`error.rs`](../../packages/contracts/src/error.rs)). `ErrorCode` is one closed snake_case enum
for the backend, Cores, stores, the engine and the App. The UI switches on the code, never on the
message. The engine's codes keep the exact strings its helpers already switch on.

`PROTOCOL_VERSION` is one integer, bumped only when old and new sides can no longer talk. Releases
do not touch it. Cores run on other people's machines and can be older or newer than the App, so
a Core reports it in `/health` and `client.core(id)` compares it before the first call
([`core-session.ts`](../../packages/api-client/src/features/core-session.ts)). A Core that reports
none speaks version 1. The backend always runs the latest, so it needs no check.

### Who calls whom

| Caller              | Client                           | Server                                        |
| ------------------- | -------------------------------- | --------------------------------------------- |
| UI                  | `client.local`                   | The App's local server: engine and App routes |
| UI                  | `client.core(id)`                | A Core, through the App's Core proxy          |
| UI                  | `client.backend`                 | `apps/backend`                                |
| UI                  | upstream's labrinth modules      | Modrinth                                      |
| App desktop domains | `call::<E>()`                    | `apps/backend` accounts and social            |
| Engine              | `call::<E>()` via sharing-client | `apps/backend` sharing, and Cores' stores     |
| Core                | `call::<E>()`                    | `apps/backend` cores                          |
| Store               | `call::<E>()` via sharing-client | Other stores, `apps/backend`                  |

The getters are on `AbstractModrinthClient`
([`abstract-client.ts`](../../packages/api-client/src/core/abstract-client.ts)). A share moves
through the backend (what a version is) and the stores (the bytes), so a member can install while
the publisher's App is closed.

Account, social and upstream's `shared-instances` routes keep labrinth's paths and shapes, so
upstream's pages reach our backend through `SelfHostedFeature` unchanged. Our screens call
`client.backend`.

## The App serves everything on 127.0.0.1

The App's local server ([`server/mod.rs`](../../apps/app/src/server/mod.rs)) binds before Tauri
builds, so the native window and tabs know its address before the frontend loads. `/health` and
`/app/boot` answer from bind. The engine routes wait until the engine is up. `/events` opens at
once, so the page sees startup's loading events, and then sends how the start ended.

Any program on the machine can reach 127.0.0.1, and so can any website through the user's browser.
The guard ([`guard.rs`](../../apps/app/src/server/guard.rs)) therefore puts every route under a
secret first path segment. A request without the secret gets 404, a `Host` other than loopback
gets 421, a same-origin request gets 403, and an `Origin` other than the App's own gets 403. No
legitimate caller is same-origin, so the same-origin refusal stops a document served from the
App's origin from reading the file routes. The secret is the security boundary for the machine;
`AppAccess` then allows everything, because the App has one local user.

Long operations start and return: the route takes a caller-made op id and answers 202, progress
arrives as `AppEvent::Progress`, and the end as `AppEvent::OpEnded`
([`ops.rs`](../../apps/app/src/server/ops.rs)). No request stays open for an operation's length,
so no operation holds one of the six HTTP/1.1 connections the UI shares.

### The Core proxy has its own listener

A Core's certificate is self-signed, and a WebView cannot pin a certificate. The App's Core proxy
([`cores/proxy.rs`](../../apps/app/src/cores/proxy.rs)) carries UI calls to Cores over TLS pinned
to the fingerprint learned at pairing. The fingerprint check exists once, in
[`sharing_client::net::tls`](../../packages/sharing-client/src/net/tls.rs).

- It is a pipe. It knows no routes, so a new Core route needs no App change. It carries WebSockets
  too.
- Core session tokens never enter the WebView. The proxy drops the WebView's `Authorization` and
  adds the session on the way out.
- It listens on a port of its own, one for all Cores, behind the same secret and guard. Content
  from other people's Cores then never shares an origin with the local server's file routes, and
  the local server refuses the proxy's origin.
- Every answer carries `Content-Security-Policy: sandbox`, so a Core's file opened as a document
  runs no script and cannot call another Core through the proxy.

## Access is the host's decision

The engine asks its host who the caller is and what they may do through `Access`
([`access.rs`](../../packages/engine/src/access.rs)). Each engine handler declares its level with
the `InstanceAccess<View | Edit | Owner>` extractor. The App's `AppAccess` allows its one local
user everything. A Core's `CoreAccess` ([`access/mod.rs`](../../apps/core/src/access/mod.rs))
maps the session to an account and checks owner, Editor or Viewer: no role is 404, a role too low
is 403. Nothing in a request can change which `Access` runs. Sessions, tickets and revalidation
are Core concepts and stay on `CoreAccess`.

## Events, and resync when one is lost

Each source owns its event enum, all with the same JSON shape: the engine's `Event`, the App's
`AppEvent` and the Core's `CoreEvent`. One enum in contracts could not name the engine's payloads.
Each host serves one `/events` socket that carries the engine's events and its own.

- The App's socket ([`server/events.rs`](../../apps/app/src/server/events.rs)) forwards every event
  to every client. One local user sees everything, so there is no subscribe protocol.
- The Core's socket ([`api/events.rs`](../../apps/core/src/api/events.rs)) takes subscriptions to
  topics. It checks `CoreAccess` at subscribe time, and again when the caller's access changes,
  when the socket falls behind, and on a timer. It ends a topic the caller lost and closes the
  socket when the session ends. A Viewer never gets a console, because the console carries chat and
  player IPs.
- A socket that falls behind sends `Resync` and carries on. The UI refetches its queries, so a lost
  install completion or process exit is never silent.
- `Event::FilesChanged` stays in-process. The socket loops drop it.

## The UI runs in a plain browser tab

Every engine call, App route and desktop feature is HTTP, so the UI works in a browser tab and
agents need no bridge. A dialog or a folder open started from a tab acts on the desktop of the
machine that runs the App.

- The native window gets the local base from an init script. A tab gets it from the URL's
  `#local=` fragment, which [`local.ts`](../../apps/app-frontend/src/platform/local.ts) moves to
  `sessionStorage` and removes from the address bar.
- Only window chrome stays on Tauri. The one Tauri command the frontend calls is `show_window`, in
  [`platform/window.ts`](../../apps/app-frontend/src/platform/window.ts), and a lint rule in the
  root [`vite.config.ts`](../../vite.config.ts) keeps every other `@tauri-apps` import out.
- The native window uses `TauriModrinthClient`, which sends Modrinth the App's user agent. A tab
  uses `GenericModrinthClient` with plain `fetch`.
- Dropping files with their real paths works only in the native window.

## Rejected

Do not propose these again without new information.

- **Per-Core certificates or public hostnames.** Certificate rate limits, every Core's home IP in
  public DNS and certificate logs, and LAN Cores breaking. The proxy works.
- **A global `State`, one combined state struct, or a generic `State<Host>`.** They let host code
  leak into the engine and hide what a function needs.
- **Desktop features over HTTP only in dev, with IPC in the native window.** Two paths drift, and
  agents would test the one users do not run.
- **Desktop domains in their own package.** Only the App imports them.
- **Engine-to-engine calls.** The publisher's App is often closed and never reachable.
- **Engine types mirrored in contracts.** Every upstream shape written twice, with a conversion
  layer in the engine.
- **The Core proxy on the local server's listener.** A Core's content would share an origin with
  the routes that read this computer's files.
- **A Core check after every upstream merge,** for now. It can join the upstream sync if needed.

## Watch

- **Event backpressure.** Console and log bursts share a socket with every other event. A lagging
  client resyncs, but a burst can still delay other events. If it does, give each subscription a
  bounded buffer that drops old console and log lines.

## Glossary

- **Store:** `sharing-storage`, the process a Core runs as `copal store`. It keeps encrypted share
  objects named by hash and serves them to holders of a backend-signed ticket.
- **Pool:** every Core's store taken together, as readers and writers of shares reach it
  (`sharing_client::pool`).
- **Holdings:** which share versions each Core's store keeps. The backend tracks them in
  `core_holdings`.
- **Content store:** the engine's store of content files on one machine, kept once by hash and
  placed into each instance that uses them. Upstream's, unrelated to the share store.
- **Self-hosted:** in `self_hosted::*` and `SelfHostedFeature`, our account backend
  (`apps/backend`) in place of Modrinth's. Not a Core, although running a Core is self-hosting too.
