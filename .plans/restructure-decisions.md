# Restructure decision log

This is the decision log for the restructure described in `.plans/restructure.md`. It is temporary
working material and is deleted when the restructure lands.

Merged from the five area designs (engine E-_, app-host H-_, protocol P-_, core C-_, tooling T-*)
and revised after three critic reviews. Binding inputs: `.plans/restructure.md` (Rules, Rejected,
What goes away) and the developer's decisions. Slice ids (F, A, B, C, D, Z) refer to
`restructure-slices.md`. "Lost:" names the losing position and why. Where a decision departs from
`restructure.md`, it says so and the item is in "Revisit with the developer".

## Process and lanes

- **D-1** Slices are PRs `restructure/NN-name`, squash-merged into `refactor/restructure-repo`; main
  merges and the final PR use merge commits. NN ranges: F 00-09, A 10-54, B 55-69, C 70-79, D 80-94,
  Z 95-99; inside a range, NN follows the dependency table's order. Reason: readable integration
  history, merge parents of upstream syncs survive (T-A13). Lost: T-A20's A range 10-39, because
  lane A has 42 slices.
- **D-2** Foundation F1-F7 is serial. Then three lanes run at once: A (engine and App host), B
  (protocol, stores, backend, Core-native routes), C (frontend platform seam, dev runner, later the
  bridge removal). D (Core on the engine) starts when A and B have both merged; C and D then run
  together. Never more than three lanes. Reason: developer's lane brief; two heavy Rust builds per
  disk are the ceiling.
- **D-3** Lane A is the critical path and cannot be split: every Tauri command conversion edits
  `apps/app/build.rs`, `apps/app/capabilities/plugins.json` and `apps/app/src/main.rs`, and every
  engine signature change touches `apps/app/src/api/*.rs`. B and C absorb everything that does not
  touch those files (B takes Core's CoreState rename and Core-native routes, which the Core design
  put in D). Lost: C2/C3 of the Core design in lane D; they need no engine and D would wait for
  them.
- **D-4** File ownership is by phase and changes hands only at named merge points (slices.md
  "Lanes"). Lockfiles and generated files (`Cargo.lock`, `pnpm-lock.yaml`, `.sqlx`,
  `packages/contracts/ts/*`) are regenerated on rebase, never hand-merged. Reason: the "two lanes
  never edit one file" rule.
- **D-5** During the restructure every slice, F1 included, checks with
  `vp run check:changed origin/refactor/restructure-repo` (T-A11). Plain `check:changed` and
  `origin/main` diff against main and would check the whole branch once F1 has merged.
- **D-6** The doc's Unproven item is done: the spike built a Core-side engine and listed content
  (verdict "works with changes"). Its findings are encoded in A2 (FileHash, SWR refresh, watcher
  globals), A29 (locale and synced-pack wakeups), A38 (no desktop tables) and D1-D4 (folders, ids,
  server content).
- **D-7** The UI runs in a plain browser tab in stages: C2 makes the runner pass each App a fixed
  local address and print `#local=<base>` on the bridge URL and a plain-tab URL; from A4 the App
  serves HTTP and every tab knows `localBase`; from A15 the shell, Library, settings and sign-in
  work in the plain tab. The rule for every green check: a screen whose commands are converted is
  checked in the plain tab, a step that still needs an unconverted command runs in the bridge tab
  until C4 deletes the bridge. Before A15 every check uses the bridge tab. Lost: H-43's bridge tab
  as the main verification surface until the end.
- **D-8** Before a module's Tauri commands are deleted, the implementer compares the old `invoke`
  and the new helper body on scenario 1 data by hand in the bridge tab's console, while both still
  exist in the working tree, and pastes the result in the PR. Route tests assert the JSON field
  names the helpers send (the real risk, A13 trap). Mutations are checked by the module's UI pass.
  No harness is committed. Lost: H-29's `window.__amberiteParity` harness, which is temporary
  machinery the route tests and UI pass already cover.

## Names and repository

- **D-9** "Core" is the internal name, "Copal" the shipped program (developer decision 1). AGENTS.md
  gets the one-line rule in F1. Lost: T6's end-of-restructure timing; the rule is true today.
- **D-10** `packages/app-lib` becomes `packages/engine`, crate `theseus` becomes `engine` (F4); the
  App crate keeps `theseus_gui` (developer decision 2) and AGENTS.md gets the theseus line in F4. If
  F4's replay merge shows crate-rename conflicts outside the manifests and lines naming the crate,
  keep crate `theseus` and the folder rename (T-A2).
- **D-11** F1 deletes `packages/moderation`, `.idea`, `packages/blog` and the blog-only changelog
  tooling (`scripts/collect-changelog.ts`, `scripts/build-theseus-release-notes.ts`,
  `.agents/skills/review-changelog`) and adds them to `upstream.json` ignore (developer decision 4,
  T-H7); see Revisit 14. The two `apps/core/docs/DO-NOT-READ-stale-*` files stay and are reported in
  Z3.
- **D-12** The root README becomes a short Amberite README in F1 (developer decision 6, T-A15).
  Never in `ignore`.
- **D-13** Core keeps "server" for host concepts (routes `/servers/{id}`, the `servers` table);
  engine routes use `/instances/{id}`; a Core server's instance id is its server id (C-43, T-H4). No
  mass rename.
- **D-14** `THESEUS_CONFIG_DIR` keeps upstream's name and is read only by the App when it resolves
  `data_dir` (E-41, T-Q5). The engine reads no env vars for paths.
- **D-15** Out-of-scope finds are listed in Z3's PR body, not acted on (developer decision 7; list
  in T-A23).

## Contracts and generated TS

- **D-16** Engine and App Endpoints are declared next to their types, not in `packages/contracts`.
  The engine's wire types are upstream's (`Instance`, `ModLoader`, `ProjectType`,
  `InstallJobSnapshot`, `SharedInstanceRole`, `ImportLauncherType` all live in upstream app-lib
  files), and contracts sits below the engine, so contracts cannot name them. Moving them into
  contracts rewrites upstream files on every sync; mirroring them in contracts writes every shape
  twice with a conversion layer. So:
  - `packages/contracts` holds the Endpoint machinery, `ApiError`, `PROTOCOL_VERSION`, limits,
    rules, `Hash`, `Side`, tickets, `store`, `backend::{accounts, social, cores, sharing, dev}` and
    the Core-native `core` module: every route whose types we own outright.
  - `packages/engine/src/endpoints/` declares the engine Endpoints (unit structs implementing
    `contracts::Endpoint`) over the engine's own types, which gain `cfg_attr(test, derive(TS))`
    lines (an additive attribute in upstream files).
  - `apps/app/src/server/endpoints/` declares the App Endpoints the same way.
  - A Core route whose types are engine types is declared in `apps/core`; Core-native routes stay in
    `contracts::core`.
    The Rules' "Every route we own is an Endpoint" holds; "in contracts" does not for engine and App
    routes. See Revisit 1. Lost: P-42's open "engine payload types in contracts", which does not
    compile (contracts is below the engine), and a mirror layer in contracts.
- **D-17** An Endpoint is a unit struct per (method, path) with consts `METHOD`, `PATH` (axum 0.8
  `{name}` template), `AUTH`, `DANGEROUS` (default false) and types `Params`, `Query`, `Body`,
  `Response` (P-02..P-04, C-40). `DANGEROUS` tells `client.core(id)` to attach a fresh ticket for
  Core's `Dangerous` extractor; `Auth::Ticket` is the store's read-ticket auth, a different thing,
  hence the different name. Bodies are `Json<T>`, `Empty`, `Raw`, `Stream`, `Upgrade<S, R>` (P-05,
  H-4). No per-endpoint headers.
- **D-18** Generated TS is one file per generating crate under `packages/contracts/ts/`:
  `contracts.ts` (from contracts), `engine.ts` (from the engine), `app.ts` (from `theseus_gui`),
  plus a three-line hand-written `index.ts` that re-exports them, in the private workspace package
  `@amberite/contracts`. api-client re-exports it, the backend imports it directly. Each crate's
  `cargo test` checks its own file is fresh; regenerate with
  `cargo test -p <crate> -- --ignored write_ts`. Duplicate type names across files fail `tsc` on the
  `export *` lines, so there is no uniqueness test. The files are regenerated on rebase (D-4). Lost:
  per-contracts-module files and empty pre-created modules (each crate now owns one file, so lanes
  never regenerate the same file), and T-Q11's ts-rs default `bindings/`.
- **D-19** ts-rs is a dev-dependency used through `cfg_attr(test, derive(TS))` with explicit
  `decl()` lists (P-19). Where a Rust name differs from an upstream TS helper type,
  `#[ts(rename = "...")]` keeps the helper's name (H-15). Upstream `packages/ariadne` gains a `ts`
  feature with `cfg_attr(feature = "ts", derive(TS))` on its two socket message enums, so the
  friends socket has one source (B7).
- **D-20** The Rust caller (`call::<E>()`, `call_stream::<E>()`, `Remote`, `CallError`, feature
  `client`) and the server helpers (`IntoResponse for ApiError`, `Req<E>`, `route::<E>()`, feature
  `server`) land in F5. The doc puts the client caller in contracts behind a feature; the server
  helpers are the same kind of transport glue with no domain logic, so they sit beside it. Access,
  which is domain logic, stays out (D-81). Lost: P1 and P4 introducing them, because A's first App
  routes (A4) need `server` before B1 merges.
- **D-21** The TS `ContractClient` (`call`, sync `url`, `stream`, throws `ApiCallError`) and the
  getters `client.backend`, `client.local`, `client.core(id)` on `AbstractModrinthClient` land in F5
  with injected transports. `backend` is built inside api-client from the AuthConfig and
  `accountApiUrl` that `SelfHostedFeature` already uses; `local` is injected by
  `apps/app-frontend/src/platform` (A4); `core(id)` rides today's Core routing (B6) and the App's
  `/app/cores` connect route (A24). Lost: P5 adding `ContractClient` and H1 adding `client.local`
  separately (both edit `abstract-client.ts` from two lanes, and H1 would wait on B).
- **D-22** Our paths keep their literal prefix; accounts and social Endpoints use the unprefixed
  labrinth path and the backend keeps stripping `/v2`, `/v3`, `/_internal` for those two modules
  only (P-06). Sharing routes that upstream's `shared-instances` module calls keep its
  `sharedinstances` v1 paths (D-90).
- **D-23** Endpoints are verb phrases, bodies `New*`/`*Update`, responses nouns; TS exports one
  const object per module (P-47). Upstream names where upstream has the concept (`InstanceVersion`,
  `ExternalFile`, `InstanceUser`, `InviteLink`, `Invite`); ours get plain names (`StoreHost`,
  `VersionDownload`, `VersionUpload`, `HostedServer`, `CoreServer`); comments never use R/P/C for
  hashes (P-39).
- **D-24** Limits and share rules (`SERVER_SHARE_FOLDER`, `CONFIG_EXTENSIONS`) live in contracts and
  are generated to TS in F5 (P-23). `contracts::Hash([u8; 32])` as lowercase hex replaces the
  store's newtype and the client's strings (P-46, B9). `Side { Client, Server }` moves from
  `sharing_client::install` to `contracts` in B3; the engine and sharing-client both use it.
- **D-25** Ticket claims and `PublicKeys` are `contracts::tickets`; `/core/keys.json` is the
  Endpoint `backend::cores::Keys`; verification stays in the store and Core (P-33). `StoreHost`
  requires the certificate fingerprint (P-34).

## Errors

- **D-26** One `ApiError { error: ErrorCode, description }` on the wire. `ErrorCode` is one closed
  snake_case enum with `#[serde(other)] Unknown`; shared generic codes take labrinth's spelling
  (`invalid_credentials`, `invalid_input`, `internal_error`); Rust servers derive status from
  `ErrorCode::status()`, the backend keeps explicit statuses (P-11..P-13).
- **D-27** F5 enumerates the whole union up front: backend kinds and codes, Core's 14, the store's
  11, the engine's `{code}` strings from `packages/engine/src/error.rs` (kept byte for byte), the
  known new Core codes (`eula_not_accepted`, `not_managed`, `already_running`, `installing`,
  `no_ports`), the friend codes (`already_friends`, `cannot_friend_self`, `own_friend_request`), and
  the App and adapter codes this plan names: `engine_unavailable` (A10b), `external_save_failed`
  (A21), `sign_in_cancelled` (A15) and `unsupported` (D5's adapter). `error.rs` is owned by B during
  lanes, then A, then D. A code no slice names yet is added by the owner's next slice; until then
  the asking slice uses an existing code. Lost: P-14's "owners add what they emit", which puts three
  lanes in one enum.
- **D-28** The TS `ApiCallError extends ModrinthApiError` exposes `status`, `code: ErrorCode` and
  `description`. Engine codes keep their current strings, so `error.code` switches in helpers
  (`helpers/install.ts:117-151`) keep working. Lost: P-24's property name `error` (it would rewrite
  every existing switch).
- **D-29** Message-text matches go from our code only: friend refusals get codes and keep labrinth's
  exact description text, so upstream's `access.vue` match (`packages/ui`, same text as
  `modrinth/main`) keeps working untouched while our screens switch on the code; Core's
  `instance/layout.vue:391` match switches in B6; Core's `Conflict` splits into its own codes in D9.
  Upstream 429 matches stay (P-15, C3 traps).

## Protocol version

- **D-30** `PROTOCOL_VERSION: u32 = 1` in contracts, bumped only when old and new sides cannot talk.
  Core `/health` answers `Health { protocol, version }` with `protocol` defaulting to 1 when absent.
  api-client compares on first use of `client.core(id)` and throws `ProtocolMismatch { side }`; B6
  shows "Update this Core" (owner, through the existing self-update), "Ask the owner to update"
  (member) or "Update Amberite". The backend needs no check (P-16, C-15).

## Engine structure

- **D-31** Principle for every engine-or-App question: the engine is what one machine does to the
  instances it holds and would do the same on a Core (content, files, worlds, logs, processes,
  installs, Java, the Modrinth metadata cache, attributes on the instance row); the App is what
  exists because a person sits at this desktop. Upstream code stays in its upstream file unless it
  carries a desktop table or a desktop-only dependency (E intro).
- **D-32** The global goes in steps: `State::new` with no global (A2); loops hold `Weak` and end
  when the engine drops (E-15); api functions take `&State` (A8, A9); global deleted and `State`
  renamed `Engine`, built only by `Engine::new(EngineConfig)` (A10); the engine starts in App setup
  and the Rust `initialize_state` command goes (A10b). Tauri commands reach the engine through an
  App-managed `EngineHandle` (E-37) until A34 deletes the last engine command and the handle with
  it. `State::start_desktop` (A2) is a temporary bundle of desktop startup; its parts leave in A28,
  A29 and A35, and A36 checks it is gone.
- **D-33**
  `EngineConfig { data_dir, instances_dir, logs_dir, side, max_concurrent_downloads, max_concurrent_writes, accounts, events, pack_hooks }`.
  `instances_dir` is required; the App passes `data_dir/profiles`, the Core `<root>/servers`. Lost:
  E-27's `Option` default (the host resolves paths).
- **D-34** Side is per engine (`EngineConfig.side`): an App engine holds client instances, a Core
  engine server instances (C-30). This narrows the doc's `launch(instance, side, credentials)`: the
  side comes from the engine, not the call. See Revisit 3. Lost: E's per-call `Side` on install; one
  engine never holds both.
- **D-35** Process-wide and unchanged: Modrinth rate limit, fetch fence, the Modrinth reqwest
  clients and user agent, regex statics, task-locals, the icon and DNS semaphores. They are process
  resources, not engine state: one process has one network identity, and only tests run two engines
  in one process. A server-side engine's download client is not among them: it lives on the engine
  (D-95). Everything keyed by instance or job id moves onto its owner (A6). Crypto provider and
  tracing are each host's first call (E-1, E-35). See Revisit 4.
- **D-36** The event sink is a host-created `broadcast::Sender<engine::Event>` (capacity 4096) plus
  loading bars, passed in `EngineConfig` so the host subscribes before startup emits (E-36). A
  lagging socket subscriber sends its client one `Resync` event and carries on; the frontend
  invalidates its queries on `Resync`, so a dropped install completion or process exit is never
  silent. Other in-process subscribers log `Lagged`. `Event::FilesChanged` stays in-process: the
  socket loops drop it (E-46). `Loading` events stay (E-11). See Revisit 13.
- **D-37** Dead state and one-time layout migrations are deleted, not run on new engines: legacy
  JSON importer, `content_store::migrate`, `migrate_instance_copies`, `synced_packs::migrate_store`,
  `migrate_legacy_icons`, `Settings::migrate`, the `cli` feature (E-14, E-31, E-43, C-23); see
  Revisit 9. The unused `AdsConsentRequired` event variant stays (upstream's; developer decision 7).
  A Tauri command nothing calls goes with its plugin, since the plugin is deleted and a route for it
  would be dead; the engine function behind it stays and is reported in Z3.
- **D-38** Sessions: the host sets the Modrinth session (`Engine::set_modrinth_session`) and one
  backend credential (`Engine::set_backend_session`: the user session on the App, the Core key on a
  Core). Calls made with the Core key carry the acting user id as an argument, taken from the
  request's `Caller` or stored with the install job. A Core sets no Modrinth session and calls
  Modrinth anonymously (E-9). Lost: C-10's `Access::backend_auth` and P-50's per-call `Remote`,
  because install jobs run sharing calls outside any request and after restarts.
- **D-39** `api/` functions stay library functions taking `&Engine` (or `&Arc<Engine>` when they
  spawn); HTTP handlers are thin wrappers in the engine's router; `engine::routes::<S>()` requires
  `Arc<Engine>: FromRef<S>` and `Arc<dyn Access>: FromRef<S>` (E-33, C shapes). Lost: H's `Host`
  trait (axum's `FromRef` already does it).
- **D-40** `MODRINTH_API_URL` stays compile-time from `packages/engine/.env` (E-40, P-43). Gradle
  stays unconditional; CI copies the `.env` and release runners set up a JDK (E-39, T-Q12).
- **D-41** The sqlx regeneration commands (E-16) are written by A38 as a comment in
  `packages/engine/.cargo/config.toml` next to its `SQLX_OFFLINE = "true"` (the root
  `.cargo/config.toml` has no sqlx setting). Lost: E-16/T-A18's scripts.md, which lane C owns while
  A writes this, and the comment sits where the trap is hit.
- **D-42** The share-history `SCHEMA` stays in sharing-client and the engine applies it at runtime
  to engine.db at open, next to its migrations; Core stops applying it in D5 (E-42). This bends the
  doc's "each database has its own migrations"; see Revisit 5. Lost: P-44/C-28 moving the table into
  the fresh engine migration, because sharing-client's own tests use the schema and a copy in the
  migration would be a second definition.
- **D-43** Support details redact JWT-shaped strings and `--accessToken` values by pattern instead
  of reading `minecraft_users` (E-38).

## Engine and App split

- **D-44** `Settings` is an App domain in app.db; the engine has no settings table and gets limits
  and dirs through `EngineConfig` and launch defaults per launch. `store_usage`, `store_cleanup`,
  `store_set_cache_limit` are engine routes; `settings_get/set` and `cancel_directory_change` are
  App routes (E-17).
- **D-45** Synced options, packs, servers, screenshots, the locale indexer and onboarding become App
  reactions to engine events (`Instance::{Created, Edited, Synced, FilesChanged, Removed}`,
  `Process::Finished`, InstallJob completed). The one ordering events cannot express, capturing the
  options base before a pack overwrites files, is a narrow `PackUpdateHooks` the App implements and
  the Core leaves empty (E-18, E-30, C-22). See Revisit 12.
- **D-46** Discord, friends status and window minimize/refocus follow
  `Process::{Launched, Finished}`; `emit_command`'s window focus moves to the App (E-20).
- **D-47** Launch:
  `launch(engine, id, LaunchRequest { credentials, defaults, mc_set_options, quick_play })`. One App
  function (`apps/app/src/launch.rs`) serves `instance_run`, `start_join_server` and
  `start_join_singleplayer_world`: ensure installed, resolve account and online profile, flush skin,
  synced packs and options, settings defaults, then launch (E-19, E-5, H-41). Quarantine and
  shared-availability checks stay in the engine.
- **D-48** Groups, group memberships and icon configs are instance attributes: they travel with the
  instance row like its name, the instance query joins them (`instance_rows.rs:716-741`) and
  `groups.rs:128` updates `instances`. They stay in engine.db with engine routes; on a Core they are
  empty and harmless, not a desktop code path. `join_log` stays in engine.db because engine log
  parsing writes it (`process.rs:840-870`). Recent icon configs are a desktop picker's history:
  app.db, App route. Hidden instances and the shared-library cache are App routes (hidden in app.db,
  the library cache a `CachedEntry` row keyed by the user id the App passes) (E-3, E-24). A38 writes
  the engine.db table list into a test, and D1 checks the Core's engine.db against the same list.
  Lost: H-37's App `library/` holding groups (it needs a grouping decoration over every instance
  read) and E-3's recent icon configs in engine.db.
- **D-49** cache, metadata and tags (25 commands) are engine routes over the engine.db cache (E-21,
  H-38). One cache in engine.db; App domains call `CachedEntry` with the engine handle (E-7).
- **D-50** users (13) and reports (1): helper bodies call upstream's labrinth modules through the
  Modrinth client (SelfHostedFeature already routes account paths); the commands and
  `engine/src/api/{users,reports}.rs` are deleted with `ported` entries (H-39). Lost: E-22's App
  routes over the engine's upstream files, a proxy route family for calls the UI can make itself.
- **D-51** jre is engine routes except the path-taking `jre_get_jre` and `jre_test_jre` (App
  routes); import discovery and `install_import_instance` are App routes calling engine code (E-23,
  H-40).
- **D-52** Any endpoint that takes or returns a host filesystem path is an App Endpoint
  (`apps/app/src/server/endpoints`) served only by the App (H-23, C-35). A Core never serves
  path-taking routes.
- **D-53** Onboarding is App-only (E-25). Deep links, `get_opening_command` and `RunEvent::Opened`
  live in `apps/app/src/desktop/` (H-28, H-37). Lost: E-26's separate `deep_link/` folder (one
  folder for OS integration).
- **D-54** The App finishes a pending data-directory move and backs up app.db and engine.db before
  `Engine::new`; the Core never moves or backs up (E-2, E-12). Playtime upload stays in the engine
  and runs only with a Modrinth session; `join_log` stays in the engine (E-8, E-10).
- **D-55** Fresh engine.db and app.db schemas, no conversion (developer decision 3). The table lists
  are E-13's. App tables keep `instance_id` columns without foreign keys and the App deletes its
  rows on `Instance::Removed`. Both databases keep a table named `app_metadata` with disjoint keys
  (E-6).
- **D-56** The engine `Instance` drops `synced_options`; `helpers/instance.ts` `get`/`list` merge
  the App's sync-preference route so their signatures do not change (E-4).
- **D-57** Desktop domain code moves from the engine crate into `apps/app/src/<domain>` (H-37
  folders: `server`, `desktop`, `accounts`, `skins`, `friends`, `discord`, `screenshots`,
  `synced_options`, `library`, `onboarding`, `settings`, `cores`, `import`) in pure-move slices
  (A35, A36) after the command conversions and before the database split. Every moved or deleted
  upstream file gets a `ported` entry.
- **D-58** App desktop domains call `contracts::backend::{social, accounts}` with `call::<E>()` and
  `self_hosted::accounts::route()` is deleted (P-49, A37), including the dev sign-in call in
  `apps/app/src/dev.rs` (`backend::dev`).
- **D-59** The engine links no desktop crate (tauri, discord-rich-presence, ariadne, winreg,
  windows); CI checks it with `cargo tree -i` (E-39, T-A5). No cargo features for headless builds.

## The App host and the plain tab

- **D-60** One local server: `apps/app/src/server/mod.rs` nests `engine::routes`, App routes,
  `/events` and `/health` under `/{secret}`, then the guard and CORS. The listener binds before
  Tauri builds. `/health` and `/app/boot` answer from the moment it binds; engine routes and
  `/events` wait for the engine to be up and dev sign-in to be done, so `main.js` never waits on
  engine startup and `initialize_state` stays the wait the UI shows. An engine that fails to start
  makes the waiting routes answer `engine_unavailable` (H-35, H-36). `AppAccess` returns
  `Caller::Local` and allows all. The Core proxy is not on this server (D-80). M1 is a gate in A4,
  A5a and A10b.
- **D-61** `localBase = http://127.0.0.1:<port>/<secret>` is known synchronously before `App.vue`
  loads: the native window gets `window.__AMBERITE_LOCAL__` from an init script, a tab gets
  `#local=<base>`, copied to `sessionStorage` and stripped;
  `apps/app-frontend/src/platform/local.ts` is the only reader (H-1). The init script runs in every
  webview the App creates, including the `signin` window on Microsoft's page, so it sets the global
  only in the top frame of the `main` window and only when `location.origin` is one of the App's own
  origins, the guard `webview-logs.js` already uses. A test or manual check confirms the `signin`
  window has no global.
- **D-62** Release binds `127.0.0.1:0` with a secret from the App. In dev the runner picks a free
  port and a 32-byte hex secret per scenario and passes `localPort`/`localSecret` in the dev config
  (C2, before the App reads them in A4; unknown dev-config fields are ignored); the App falls back
  to `:0` and its own secret when they are absent. The secret goes only to `.data/runtime.json` and
  `up`/`status` output, never `.data/dev.log` (H-30, H-31).
- **D-63** Guard order, on the local server and the Core proxy alike: secret prefix (404), `Host` is
  `127.0.0.1:<port>` or `localhost:<port>` (421), `Sec-Fetch-Site: same-origin` is refused (403),
  `Origin` when present must be allowed (403), WebSocket upgrades must carry an allowed `Origin`. No
  legitimate caller is same-origin: the native window, dev tabs and `<img>` loads are all
  cross-site, so the same-origin refusal stops a document served from the App's own origin from
  reading `/app/file`. Allowed origins: `tauri://localhost`, `http(s)://tauri.localhost`, and in
  debug builds the dev config's Vite origins (`*.localhost` and loopback at the Vite port) and the
  bridge origin until C4. Never any other loopback port, so the proxy's origin cannot call the local
  server (H-16, H-21). See Revisit 7.
- **D-64** Dev tab URL: `http://scenario-<n>.localhost:<vite>/#local=<encoded base>`; each scenario
  its own origin; Vite `server.allowedHosts: ['.localhost']` (H-32). The backend's `LOCAL_DEV` CORS
  accepts `*.localhost` (B1).
- **D-65** The native window stays config-created. Dev runs keep today's `hide_window`, so a hidden
  dev App still has a webview. C4 measures the hidden webviews' memory; release startup does not
  change. Lost: H-34's `"create": false` window built in `setup`, which changes release startup,
  window-state restore and the macOS config merge for a dev convenience.
- **D-66** Local files: `GET /app/file?path=` and `GET /app/thumbnail?path&size`;
  `localFileUrl(path)` replaces all `convertFileSrc`; Rust returns paths, not `asset://` URLs. Files
  in hash-named caches (the icon cache) are served `Cache-Control: immutable`; everything else gets
  an ETag from mtime and length with `no-cache`, a cheap 304. Core instance images come from the
  Core's engine routes (H-6, H-19).
- **D-67** Dialogs return paths (`/app/dialog/open|save`), `fs/read` (raw) and `fs/stat` replace JS
  `readFile`, `file_read_dragged_file`, `get_dragged_skin_data`, `file_local_info`; the file-picker
  adapter builds `File` objects so `packages/ui` does not change (H-5). From a tab, dialogs, links
  and folder opens act on the App's desktop (H-11). See Revisit 10.
- **D-68** `GET /app/boot` returns version, OS info, flags and the dev config; `platform/os.ts`
  serves plugin-os reads synchronously from it; the constant commands fold into it (H-11, H-27).
  `webview-logs.js` posts to `/app/webview-log` from the native window only.
- **D-69** Stays on Tauri, gated by `isTauri()`: window chrome, `saveWindowState`, `show_window`,
  `onFocusChanged`, native drag and drop (`FileDropProvider` only when native), macOS `edit-menu://`
  undo/redo (H-11, H-12, H-28). Native window uses `TauriModrinthClient`, a tab
  `GenericModrinthClient`.
- **D-70** Long operations start and return: `POST` takes a caller-made `op` id, registers the op
  and answers 202 at once; progress comes as `Event::Progress { op, stage, done, total }` and the
  end as `Event::OpEnded { op, error: Option<ApiError> }`; `DELETE /app/ops/{op}` cancels. An op
  whose caller needs output takes its destination as input (the save path), so the end event carries
  only an error. No request stays open for the op's length, so ops never hold one of the six
  HTTP/1.1 connections the UI shares. Window-label keys go (H-3, H-24). Lost: H-24's cancel by
  client disconnect.
- **D-71** Updater: `AppState.desktop.updater` holds the pending `Update`; routes check, download
  (op), pending delete, restart-after and restart; `getUpdateSize`/`enqueueUpdateForInstallation`
  keep their signatures and ignore the rid. Install-on-exit unchanged (H-26). The `updater` cargo
  feature and `MODRINTH_EXTERNAL_UPDATE_PROVIDER` stay as they are (release config, reported in Z3).
- **D-72** Sign-in: `POST /app/accounts/minecraft/login` opens today's native sign-in window on the
  desktop; Modrinth login runs today's listener and deep-link handoff; a cancel route replaces
  `cancel_modrinth_login` (H-20).
- **D-73** Remaining Tauri capabilities: window permissions, `core:event`, `core:webview`,
  `window-state`, and the `http` allow-list for Modrinth hosts plus `http://127.0.0.1:*/*` (the
  native window's `TauriModrinthClient` reaches the Core proxy through `tauriFetch`). `dev.json`
  goes together with its name in `tauri-local.conf.json`; `updater.json` goes; plugin-fs goes; JS
  plugin-dialog, -fs, -opener, -os, -updater go (H-25).
- **D-74** When a helper file switches, each wire type it declares becomes a re-export of the
  generated type under the same name; non-wire types stay hand-written (H-15).
- **D-75** A plain `apps/app-frontend/src/platform/` seam wraps every non-invoke Tauri call (C1),
  and an oxlint `no-restricted-imports` rule in root `vite.config.ts` forbids `@tauri-apps/*` plugin
  and window imports outside it; C4 extends it to `@tauri-apps/api/core`.
  `packages/browser-bridge/test/inventory.test.ts` is deleted in C1 (T-A6). Lost: H's per-slice
  `inventory.test.ts` edits (A would edit C's package).
- **D-76** No frontend test runner is added; behaviour is tested in Rust route tests and api-client
  vitest (H-17). Speed is measured as M1 (cold start to state ready), M2 (warm tab reload, target
  1000 ms), M3 (transport overhead p50 3 ms, via `Server-Timing` in debug) at C1 and C4 (H-42), and
  M1 is a gate in A4, A5a and A10b. With ops start-and-return (D-70) and immutable icons (D-66), six
  connections per host are accepted; if M3 shows saturation, local files move to the
  `localhost:<port>` alias host (H-44).

## Events

- **D-77** Each source owns its event enum, all with the same JSON shape
  `#[serde(tag = "type", content = "payload", rename_all = "snake_case")]` (the shape the codec
  already produces). `engine::Event` (the engine's `event/mod.rs` enum): Loading, Process, Instance,
  InstallJob, Warning, Log, FilesChanged (in-process, never sent). The App's `AppEvent`: Friend,
  Notification (`serde_json::Value`), OnboardingChecklist, InstanceGroupsChanged, Command,
  GameOptionLocalesUpdated, Progress, OpEnded, Resync. The Core's `CoreEvent` (D8): Console, Power,
  Backup, Ended, Resync. A socket sends `engine::Event` and its host's enum on one stream; the
  generated TS union is `EngineEvent | AppEvent` for `client.local` and `EngineEvent | CoreEvent`
  for `client.core(id)`. `Command` includes `ApproveCore` (H-18, H-22, C shapes). Lost: one `Event`
  enum in contracts, which cannot name engine payloads (D-16), and would put desktop variants in the
  engine.
- **D-78** Each host owns its socket loop at `/events`. The App's forwards every engine and App
  event except `FilesChanged`, with no subscribe protocol (one local user sees everything). The
  Core's lives in `apps/core`: `Topic { Instance, Installs, Console, Server, Backups }` and
  `ClientMessage { Subscribe, Unsubscribe }` in `contracts::core`; subscribe checks `CoreAccess`;
  the loop re-checks on access changes, on lag and every 600 s, closes the socket when the session
  fails and ends a subscription with `Ended` when its permission fails (C-33, C-34). Viewers may
  subscribe to Instance, Installs, Server and Backups; Console needs Edit (C-13). Four sockets per
  Core session. A shared fan-out helper is added only if both loops turn out to need it. Lost: the
  engine-owned generic loop and `Topics` trait, which made the App implement no-op session methods
  for a loop that forwards everything.
- **D-79** `initialize_state(events)` keeps its TS name. From A10b it resolves when `/events` opens,
  which happens once the engine is ready (D-60); there is no ready endpoint (H-2). The Rust
  `initialize_state` command is deleted in A10b.

## The Core proxy

- **D-80** The four `core_*` commands become `/app/cores` routes on the local server. The proxy
  keeps a listener of its own, one for all Cores, on a different port from the local server, so
  content from other people's Cores never shares an origin with `/app/file`. `connect` returns
  `http://127.0.0.1:<proxy port>/<secret>/core/<n>`. It is still a pipe with pinned TLS, adding the
  session and dropping the WebView's `Authorization`; it applies the D-63 guard and adds
  `Content-Security-Policy: sandbox` and `X-Content-Type-Options: nosniff` to every response, so a
  Core file opened as a document runs in an opaque origin and cannot call another Core through the
  proxy. Per-tunnel listeners and capabilities go (H-7). The fingerprint check exists once, in
  `sharing_client::net::tls` (C-26, T-H10). See Revisit 7. Lost: H-7's mount at `/core/{n}/*` on the
  local server.

## Access

- **D-81** `Access` lives in `engine::access` with two methods, as the doc sketches:
  `caller(&Parts) -> Result<Caller, ApiError>` and
  `require(&Caller, Option<&str>, Permission) -> Result<Permission, ApiError>`, which returns the
  granted level so a route can widen what it shows (a crash's `last_lines` needs Edit inside a View
  route) without a second call. `Permission { View, Edit, Owner }` matches Core's
  `Level::{Viewer, Editor, Owner}` and the UI's roles. The extractor is `InstanceAccess<P>`, not
  `Instance<P>`, which clashes with the engine's `Instance` model. Sessions, revalidation, access
  changes and stream caps are Core concepts and stay on `CoreAccess` in `apps/core`. Lost: H-35's
  trait in contracts (contracts holds no logic), C-10's `backend_auth` (D-38), and a five-method
  trait the App would fill with no-ops.
- **D-82** Each engine handler declares its level through `InstanceAccess<View|Edit|Owner>`; Core's
  levels carry over (Editor for every file route including reads; Owner for share history, restore,
  version delete and unshare; Viewer for content list, update checks, modpack card, edited-file
  list, icon). Engine-wide routes need `require(None, Owner)`; list routes filter by `View`. No role
  is 404, too low is 403, both inside `CoreAccess::require` (C-1, C-2, C-35).
- **D-83** Fresh tickets stay a Core-only `Dangerous` extractor; the engine never sees tickets; the
  Endpoint's `DANGEROUS` const tells `client.core(id)` to attach one (C-1, C-40).
- **D-84** The engine router exposes each handler. The Core mounts `engine::routes()` except the
  routes it hooks, which it mounts itself by wrapping the engine handler:
  `route::<E>(with_backup(engine::routes::update_pack))`, `with_log(..)`, `with_forget_players(..)`.
  The compiler ties each hook to its Endpoint type, so a renamed path or a new route cannot silently
  skip a backup (C-3, C-4). Core content mutations refuse while `Engine::mark_running(id)` is set,
  under the engine's instance lock that the Core also takes to start a server. Lost: a
  `(Method, E::PATH) -> Hook` string table.

## Backend and stores

- **D-85** accounts and social keep labrinth's shapes; upstream modules keep reaching them through
  `SelfHostedFeature`, narrowed to a matcher generated from the accounts and social path tables; our
  code uses `client.backend`; `labrinth-compat.ts` asserts assignability (P-07, P-26, P-29). See
  Revisit 11.
- **D-86** Core-key variants share one Endpoint per (method, path) with `AUTH = Account` and
  optional Core fields; the backend keeps its `coreSharing` prefix dispatch (P-08).
- **D-87** The Worker keeps hand validators as one `parse` per Endpoint typed by the generated body;
  the spliced manifest is typed through `JsonText<T>` and `spliceJson`; a typed
  `route(E, parse, handle)` table replaces the fall-through chain module by module; D1 row types
  stay (P-09, P-10, P-30, P-31).
- **D-88** The D1 schema is one fresh `0001_schema.sql` (B2); every `.data/backend` resets (P-40,
  P-41). Tables named after a contract concept take its name: `shared_members` to
  `shared_instance_users`, `shared_versions` to `shared_instance_versions`, `shared_files` to
  `shared_instance_files`, `shared_links` to `shared_instance_invites`, `shared_reports` to
  `shared_instance_reports`, `shared_servers` to `hosted_servers`, `server_players` to
  `hosted_server_players`. Tables with no contract concept keep their names: `core_holdings`,
  `core_storage`, `visible_servers` and the account and social tables. Columns: `root` (the hash of
  a version's sorted object hashes) becomes `version_hash`, matching the contract field; object and
  file hashes stay `sha256`. Contract field names follow the same words.
- **D-89** The friends socket is `backend::social::FriendsSocket`, an `Upgrade<S, R>` Endpoint whose
  message types are upstream ariadne's two enums, generated to TS through ariadne's `ts` feature
  (D-19). The App keeps ariadne as its Rust type source, the backend types its handler with the
  generated TS and its hand `Presence`/`Connection` types go. `SharedInstanceChange` becomes
  `contracts::backend::social` (P-32), in B7 together with `use-shared-instance-event.ts`. Lost: P6
  also editing that file, and the socket outside contracts.
- **D-90** Upstream's `shared-instances` module returns to upstream byte for byte in B10, as archon
  and kyros do in D9. It keeps reaching the backend through `SelfHostedFeature`'s `sharedinstances`
  routing, so the backend serves upstream's paths and shapes for every route upstream calls, the way
  accounts and social do (D-22, D-85); `labrinth-compat.ts` asserts the generated types fit
  upstream's `SharedInstances` types. Our screens call `client.backend`. The Amberite additions in
  that folder (`players`, `storage`, the `module.ts` base, `errors.ts`) leave it: the ones
  `packages/ui` calls (`client.sharedinstances.players_v1`, from upstream composables we already
  edited) move to `packages/api-client/src/sharing/` under the same client keys, bodies over
  `client.backend` with generated types, so `packages/ui` stays untouched; the rest are deleted once
  A40 moves our callers (P-27). Dead paths deleted in B10: the `already has access` text match, the
  App's 405 tolerance, `RemoteSharedInstanceUsersResponse::Legacy`,
  `ShareVersion.backtrack_version`, the always-404 server-icon route, the
  `/user/{id}/projects|organizations|collections` stubs (B7, social), `PATCH /v1/storage`,
  `GET /v1/instances/{id}/access` and `/history`, each after a grep finds no caller (P-38).
  `GET /v1/instances/{id}/servers` belongs to `backend::sharing` (P-36). `/health` is an Endpoint on
  every server (backend, App, Core); `/_dev/session` and `/_dev/outbox` are `backend::dev` Endpoints
  served only under `LOCAL_DEV` (B8). CORS is not a route. Lost: P-37's routes outside contracts and
  P-27's upstream bodies rewritten onto `client.backend`.
- **D-91** Backend black-box tests stay hand-written clients, updated by each slice (P-48). B2
  renames them `*.api.test.mjs`; "contract test" read as a test of `packages/contracts`.

## The Core on the engine

- **D-92** Core folders: `<root>/engine` (`ServersAccess::None`) holds engine.db, the engine store,
  meta and caches; `instances_dir = <root>/servers`; Core's own `meta/`, `java/`, `store/` unchanged
  (C-14, C-19, E-45).
- **D-93** Hosts may choose an instance's id and folder; the Core passes its server UUID for both,
  the App passes none (C-5, C-21, E-29). World id equals server id only inside the adapter.
- **D-94** Server content model: requirements already exist on content entries; the engine gains
  `sides_unlocked`, `server_override` (content entries) and `pending` (instance files), client-only
  pack files as `instance_files.missing`, `ModLoader::Paper`, `ProjectType::Plugin` (`plugins/`),
  and server datapacks under `<level-name>/datapacks`.
  `servers(id, port, memory_mb, java_path, java_version)` and `pending_packs` stay in core.db (C-8,
  C-20, C-31). The doc says the server filter uses "the types that already exist" and that
  server-only steps stay in Core; the columns and the two variants are new, and
  `apply_pending_on_stop` stays a Core step that calls the engine. See Revisit 6. Lost: E-28 keeping
  `pending` in core.db, because the engine's pack-update loop writes it and must not ask the host.
- **D-95** Core's folder sandbox, archive checks and pack-host guard move into the engine and apply
  only when `side == Server`; desktop install behaviour is unchanged. The archive checks write
  through the sandbox's `Folder` (cap-std, refuses links) and reserve Core's 256 MiB
  `DISK_MARGIN_BYTES`, so on desktop they would refuse archives with links, refuse extraction into
  the symlinked folders desktop users keep, and reserve disk space; they stay a server rule. The
  pack-host guard is more than an allowlist: `hosts::client()` resolves names itself, refuses
  private addresses, connects only to the addresses it checked and re-checks every redirect. A
  server-side engine holds that client and downloads every pack file, override and share file
  through it (D4), with the hosts.rs tests ported and a redirect-to-private-address test added.
  `jar/server.rs` becomes a child module of the engine's `embedded_content_metadata.rs` (C-25).
  Lost: C-7's archive checks on both sides (a desktop behaviour change nobody asked for).
- **D-96** The engine installs by side: on a server it downloads the server jar and runs the
  loader's server install, porting `apps/core/src/installs/**` (a server port of app-lib's launcher)
  into the engine, and `server_command(engine, id)` returns the Java command that Core's supervisor
  and shim run (doc "Client and server", E-44). Lost: C-9 keeping installs in Core, because the doc
  assigns the server install to the engine and `installs/**` is a second copy of the launcher. See
  Revisit 2.
- **D-97** Downloads use one mechanism: `POST /links { path }` gives a one-use five-minute token
  bound to path and caller, redeemed in `CoreAccess::caller` with `require` re-run (C-32). Core's
  upload sessions, zip, unarchive and stat move into the engine as file Endpoints behind the sandbox
  (C-44).
- **D-98** `copal.db` becomes `core.db` with a fresh `001_init.sql` in D5; one `.data/core` reset
  (C-38).
- **D-99** A Core starts only what `Engine::new` starts: DB, content-store recover, install
  recovery, watcher, process manager (C-39).
- **D-100** On a Core, unshare fails like on the App when the backend is down; Core's owed
  share-deletion retry goes; its other owed calls stay (C-42). See Revisit 8.
- **D-101** The Core writes `server-icon.png` from engine instance events (C-41). The copal CLI
  calls through one generic caller over its local socket (C-12). `setLinkedInstance` is dropped
  (C-17).
- **D-102** packages/ui is untouched for the Core panel: `packages/api-client/src/core-hosting/`
  wraps the archon and kyros modules with `withCoreHosting(client)`; calls for a Core server go to
  adapter methods of the same name over `client.core(id)`; deleting the folder and one call removes
  it. Upstream archon and kyros modules return to upstream byte for byte in the slice that moves
  their domain. Upstream calls with no Core equivalent (`restoreClientFiles`,
  `getUpdateGameVersionPreview`, `applyGameVersionUpdate`) answer `ApiError { error: unsupported }`
  from the adapter (D5). Our own Core screens are not the adapter's business: `archon/cores/v1.ts`
  (ours, a hand-written per-route client) and the `Cores` namespace in `archon/types.ts` are
  replaced by direct `client.backend` and `client.core(id)` calls with generated types. B4 turns the
  `Cores` types into re-exports of the generated ones (one definition); A40 moves our callers and
  deletes the module and the namespace, before D9 returns `archon/types.ts` to upstream (developer
  decision 5, C-36, C-37). `packages/ui` files we already edited that import `Copal.*` types keep
  compiling: api-client keeps exporting those names as re-exports of generated types.

## bedrock and shared crates

- **D-103** bedrock empties in order: limits to contracts (F5); `disk.rs` and `net/` to
  sharing-client (F6); sharing-client off `bedrock::state` and `bedrock::content` with `consume`
  moved to `apps/core/src/share_pack.rs` (B3); the pack-host guard, sandbox, archive checks and
  `jar/server.rs` to the engine, with Core's importers switched to the engine's copy (D3);
  `content/`, its last user of bedrock's jar clone (`content/domain/sides.rs` imports
  `upstream::jar::Environment`), the jar and store clones, glue and `consume` deleted together (D5),
  because content dies with or before jar and bedrock cannot import the engine; the crate,
  `ports.toml` and `shared-rust.md` deleted and the borrowed files restored in the engine (D10)
  (C-45). `ProjectType` and `ModLoader` stay in the engine; sharing-client uses
  `modrinth_content_management::ContentType` plus a folder table (P-42, C-29).
- **D-104** JSON events remove the engine's postcard and serde-binhum use and
  `packages/serde-binhum` in A5b, a mechanical slice after A5a switched the transport;
  `postcard-bindgen` stays a workspace dependency while bedrock declares it, until D10 (P-45, E-47).
- **D-105** toasty and the implicit async-minecraft-ping member stay; toasty is excluded from
  `cargo test --workspace` and reported (T-A17).

## Tooling, CI and upstream sync

- **D-106** CI has one `rust` job with a broad filter (every Rust input plus `apps/app/**`, since
  tauri-build checks capabilities and `tauri*.conf.json` at compile time, and `packages/*/.env*`)
  running `cargo test --workspace --exclude toasty --locked` and a desktop-crate guard;
  `check:changed` runs the tests of every touched crate with `SQLX_OFFLINE=true` (T-Q13, T-A4,
  T-A8).
- **D-107** `upstream.json` gets `renamed` (prefix map) and `ported` (upstream path to our port);
  `watch` folds into `ported`; the sync merges with `-c merge.directoryRenames=true`, never
  resurrects ported files and buckets their commits as `port`; rules live in
  `docs/internals/upstream.md` (T-Q6, T-Q14, T-H8). New upstream app-lib migrations are dropped from
  merges and ported by hand into the fresh schemas (T-H5).
- **D-108** `.coderabbit.yaml` lands in F1 with path filters (lockfiles, `.sqlx`,
  `packages/contracts/ts/**`, generated, deleted dirs), per-area instructions, five custom checks,
  and base branches for the integration branch and slices (T-A3).
- **D-109** One worktree and one `target/` per lane; `vp run up` keeps the shared build folder.
  Lanes start from primary's `.data` until a schema slice makes part of it unusable. From then on,
  every slice worktree based on that slice deletes the affected part and rebuilds what its check
  needs with scripts.md's "Regenerating the sharing dataset" steps: after B2, `.data/backend` (and
  the two-scenario sharing setup with it); after A38, `.data/scenarios`; after D4,
  `.data/core/engine` and `.data/scenarios`; after D5, `.data/core`. D1 starts from an empty
  `.data/scenarios` and `.data/core`. The dataset is rebuilt once in Z3 and left in a scratch
  folder; the developer replaces the primary checkout's `.data` after the final merge, as
  AGENTS.md's one-way data rule requires (T-A9, T-A10, T-A21).
- **D-110** main merges into the integration branch at three points: F7, C3 (before lane D), Z1
  (T-A14).
- **D-111** The engine version stamp in theseus-build.yml stays, path renamed (T-Q15). The four
  `.gitattributes` `!eol` lines go with the old migrations in A38 (T-Q7). The `setup-worktree.ts`
  fallback to `packages/app-lib/.env` (T-A12) goes in Z2.

## Docs

- **D-112** Docs change in the slice that makes them false; lane C owns AGENTS.md, `docs/**` and the
  skills except `docs/internals/shared-rust.md` (B, then D, which deletes it) and two named
  carve-outs: F5 rewrites AGENTS.md's "One contract" bullet (foundation, before lanes), and A38
  rewrites AGENTS.md's "Test data" section (C edits nothing in that section during the lanes). Z2
  writes `docs/internals/architecture.md` from the doc's lasting parts and deletes
  `.plans/restructure.md`, `.plans/restructure-decisions.md` and `.plans/restructure-slices.md`
  (T-H9, T6). The plan's claim that the "Copal never depends on app-lib" rule is in AGENTS.md is
  stale; it is only in `shared-rust.md:14` (C-16, T-Q10).
- **D-113** Names that would mislead are fixed where cheap and explained where not:
  `InstanceAccess<P>` (D-81), `DANGEROUS` (D-17), `*.api.test.mjs` (D-91), `Side` in contracts
  (D-24). `self_hosted::*` and `SelfHostedFeature` mean "our account backend", while self-hosting
  also means Cores; renaming them is churn across moved files, so Z2's glossary defines the term
  instead, next to pool, store, content store and holdings.

## Shapes other code depends on

```rust
// packages/contracts/src/lib.rs
pub const PROTOCOL_VERSION: u32 = 1;
pub enum Method { Get, Post, Put, Patch, Delete }
pub enum Auth { None, Session, CoreKey, Account, Ticket, Host }   // Ticket: the store's read ticket
pub enum Side { Client, Server }
pub trait Endpoint {
    const METHOD: Method;
    const PATH: &'static str;                 // "/v1/instances/{id}/versions"
    const AUTH: Auth;
    const DANGEROUS: bool = false;            // client.core(id) attaches a fresh ticket
    type Params: Serialize + DeserializeOwned; // () when none
    type Query: Serialize + DeserializeOwned;
    type Body: RequestBody;                   // Json<T> | Empty | Raw | Stream
    type Response: ResponseBody;              // Json<T> | Empty | Raw | Stream | Upgrade<S, R>
}
pub struct Raw { pub content_type: String, pub bytes: Bytes }

// error.rs
pub struct ApiError { pub error: ErrorCode, pub description: String }
#[serde(rename_all = "snake_case")] pub enum ErrorCode { /* full union, D-27 */ #[serde(other)] Unknown }
impl ErrorCode { pub fn status(self) -> u16 }

// client.rs (feature "client")
pub struct Remote<'a> { pub http: &'a reqwest::Client, pub base: &'a str, pub bearer: Option<&'a str>,
                        pub read_ticket: Option<&'a str>, pub timeout: Option<Duration> }
pub async fn call<E: Endpoint>(r: &Remote<'_>, p: &E::Params, q: &E::Query, b: E::Body)
    -> Result<<E::Response as ResponseBody>::Output, CallError>;
pub async fn call_stream<E: Endpoint<Response = Stream>>(r: &Remote<'_>, p: &E::Params, q: &E::Query, from: u64)
    -> Result<reqwest::Response, CallError>;
pub enum CallError { Api { status: u16, error: ApiError }, Http(reqwest::Error), Decode { status: u16, source: serde_json::Error } }

// server.rs (feature "server")
impl IntoResponse for ApiError;
pub struct Req<E: Endpoint> { pub params: E::Params, pub query: E::Query, pub body: /* inner */ }
pub fn route<E: Endpoint, H, T, S>(handler: H) -> (&'static str, MethodRouter<S>);

// core.rs (Core-native routes, the Core socket's protocol)
pub struct Health { pub protocol: u32 /* serde default 1 */, pub version: String }
pub enum Topic { Instance { id: String }, Installs, Console { id: String }, Server { id: String }, Backups { id: String } }
pub enum ClientMessage { Subscribe { topic: Topic }, Unsubscribe { topic: Topic } }

// backend/social.rs
pub struct FriendsSocket;   // Upgrade<ariadne::ServerToClientMessage, ariadne::ClientToServerMessage>
```

```rust
// packages/engine
pub struct EngineConfig {
    pub data_dir: PathBuf, pub instances_dir: PathBuf, pub logs_dir: Option<PathBuf>, pub side: Side,
    pub max_concurrent_downloads: usize, pub max_concurrent_writes: usize,
    pub accounts: Option<AccountEndpoints>, pub events: EventSink,
    pub pack_hooks: Option<Arc<dyn PackUpdateHooks>>,
}
impl Engine {
    pub async fn new(config: EngineConfig) -> crate::Result<Arc<Engine>>;
    pub fn set_modrinth_session(&self, s: Option<ModrinthSession>);
    pub fn set_backend_session(&self, s: Option<BackendSession>);
    pub fn mark_running(&self, instance: &str, running: bool);
    pub async fn lock_instance(&self, instance: &str) -> InstanceGuard;
}
pub struct BackendSession { pub base_url: Url, pub bearer: String, pub kind: BackendSessionKind /* User | CoreKey */ }
impl EventSink { pub fn new() -> Self; pub fn subscribe(&self) -> broadcast::Receiver<Event>; }
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum Event { Loading(..), Process(..), Instance(..), InstallJob(..), Warning(..), Log(..),
                 #[serde(skip)] FilesChanged { .. } }
#[async_trait] pub trait PackUpdateHooks: Send + Sync {
    async fn before_pack_update(&self, engine: &Engine, instance_id: &str) -> crate::Result<()>;
    async fn after_pack_update(&self, engine: &Engine, instance_id: &str, overrides: PackOverrides<'_>) -> crate::Result<()>;
}
pub struct LaunchCredentials { pub username: String, pub uuid: Uuid, pub access_token: String, pub user_type: String, pub profile_properties: serde_json::Value }
pub struct LaunchDefaults { pub extra_launch_args: Vec<String>, pub custom_env_vars: Vec<(String, String)>, pub memory: MemorySettings, pub game_resolution: WindowSize, pub force_fullscreen: bool, pub hooks: Hooks }
pub struct LaunchRequest { pub credentials: LaunchCredentials, pub defaults: LaunchDefaults, pub mc_set_options: Vec<(String, String)>, pub quick_play: QuickPlayType }
pub async fn ensure_installed(engine: &Arc<Engine>, id: &str) -> crate::Result<()>;
pub async fn launch(engine: &Arc<Engine>, id: &str, req: LaunchRequest) -> crate::Result<ProcessMetadata>;
pub async fn server_command(engine: &Engine, id: &str) -> crate::Result<ServerCommand>; // { java, args, cwd }
pub struct NewInstance { pub id: Option<String>, pub path: Option<String>, /* name, game_version, loader, .. */ }

// packages/engine/src/endpoints/*.rs: engine Endpoints over engine types
pub struct ListInstances;   // impl contracts::Endpoint, Response = Json<Vec<Instance>>
// packages/engine/src/routes/mod.rs: one pub fn per handler, plus
pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> where Arc<Engine>: FromRef<S>, Arc<dyn Access>: FromRef<S>;

// packages/engine/src/access.rs
pub enum Caller { Local, Session { id: String, user_id: String, owner: bool } }
pub enum Permission { View, Edit, Owner }   // Ord
pub trait Access: Send + Sync + 'static {   // object safe, BoxFuture returns
    fn caller<'a>(&'a self, req: &'a Parts) -> BoxFuture<'a, Result<Caller, ApiError>>;
    fn require<'a>(&'a self, c: &'a Caller, instance: Option<&'a str>, p: Permission) -> BoxFuture<'a, Result<Permission, ApiError>>;
}
pub struct InstanceAccess<P> { pub id: String, pub caller: Caller, pub granted: Permission, _level: PhantomData<P> } // extractor
```

```rust
// apps/app/src/server/mod.rs
#[derive(Clone)] pub struct AppState { pub engine: Arc<Engine>, pub access: Arc<dyn Access>,
    pub desktop: Arc<desktop::Desktop>, pub cores: Arc<cores::Cores>, /* one field per domain */ }
pub struct LocalServer { pub listener: std::net::TcpListener, pub secret: String }
impl LocalServer { pub fn bind(port: Option<u16>, secret: Option<String>) -> io::Result<Self>;
                   pub fn base_url(&self) -> String; pub async fn serve(self, router: Router); }
// apps/app/src/server/events.rs
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AppEvent { Friend(..), Notification(serde_json::Value), OnboardingChecklist(..), InstanceGroupsChanged(..),
  Command(Command), GameOptionLocalesUpdated, Progress { op: String, stage: Option<String>, done: u64, total: Option<u64> },
  OpEnded { op: String, error: Option<ApiError> }, Resync }
pub enum Command { RunMrpack { path }, LaunchInstance { id, server, singleplayer_world }, ApproveCore { code } }
// apps/app/src/launch.rs
pub async fn launch(engine: &Arc<Engine>, instance_id: &str, quick_play: QuickPlayType) -> Result<ProcessMetadata>;

// apps/core/src/api/events.rs
pub enum CoreEvent { Console { .. }, Power { .. }, Backup { .. }, Ended { topic: Topic, error: ApiError }, Resync }
```

```ts
// packages/contracts/ts/{contracts,engine,app}.ts (generated); one const object per module
export interface Endpoint<P, Q, B, R> {
	method
	path
	auth
	dangerous: boolean
	body
	response
	readonly _types?: { params: P; query: Q; body: B; response: R }
}
// packages/contracts/ts/index.ts (hand-written, three re-export lines)
// packages/api-client/src/contracts/client.ts
export class ContractClient {
	constructor(t: {
		base(): Promise<string>
		headers(): Promise<Record<string, string>>
		fetch: typeof fetch
	})
	call<E extends Endpoint>(e: E, input: InputOf<E>): Promise<ResponseOf<E>> // throws ApiCallError { status, code, description }
	url<E extends Endpoint>(e: E, params: ParamsOf<E>): string // sync once base is known
	stream<E extends Endpoint>(e: E, input: InputOf<E>, from?: number): Promise<Response>
}
// AbstractModrinthClient: get backend(); get local(); core(id: string)
// apps/app-frontend/src/platform/local.ts
export const localBase: string
export function localFileUrl(path: string): string
export const isNativeWindow: boolean
// packages/api-client/src/core-hosting/index.ts
export function withCoreHosting(client: AbstractModrinthClient): void
// apps/backend/src/common/router.ts
export function route<E extends Endpoint>(
	e: E,
	parse: (raw: unknown) => BodyOf<E>,
	handle: (ctx) => Promise<ResponseOf<E> | Spliced<ResponseOf<E>>>,
): Route
```

```ts
// upstream.json
type UpstreamConfig = {
	ignore: string[]
	renamed: Record<string, string>
	ported: Record<string, string>
}
```

## Revisit with the developer

Choices that depart from `.plans/restructure.md`, are irreversible, or are product-visible, made
without direction, with the default taken. Each default is what the slices build.

1. **Engine and App Endpoints live beside their types, not in contracts (D-16, D-77).** The doc says
   every route is an Endpoint in contracts and one `Event` enum lives there. Contracts sits below
   the engine and the engine's wire types are upstream's, so it cannot. Default: the engine and App
   declare their Endpoints and event enums next to their types; contracts keeps every type we own
   outright; one TS package still exports all of it. Alternative: mirror types in contracts with a
   conversion layer in the engine, one copy of every upstream shape.
2. **Server processes are started by the Core, not by an engine `launch` (D-96).** Default: the
   engine installs by side and returns `server_command`; Core's supervisor and shim run it. The
   doc's `launch(instance, side, credentials)` becomes install plus `server_command`. Alternative:
   one launch path with the shim inside the engine.
3. **Side is a property of the engine (D-34).** Default: `EngineConfig.side`; an engine holds only
   one kind of instance. Alternative: the doc's per-call side, if one engine should ever hold both.
4. **Some process-wide statics stay (D-35).** Default: the Modrinth rate-limit fence, Modrinth
   reqwest clients, user agent and semaphores stay process-wide, read as process resources rather
   than engine state; nothing keyed by instance or job does. Alternative: move them onto `Engine`,
   which only changes behaviour when two engines share a process (tests).
5. **Share history is created at runtime, not by a migration (D-42).** Default: sharing-client's
   `SCHEMA` runs on engine.db at open, so the table has one definition. Alternative: the table goes
   in the fresh engine migration and sharing-client's tests keep their own copy.
6. **New engine columns and enum variants for servers (D-94, D4).** Default: `sides_unlocked`,
   `server_override`, `pending`, `ModLoader::Paper`, `ProjectType::Plugin`, against the doc's "types
   that already exist". `apply_pending_on_stop` stays a Core step calling the engine. Alternative:
   keep these in core.db and Core code, with the engine asking the host during pack updates.
7. **The App's local server security model (D-63, D-66, D-80).** Default: one secret per App
   replaces the 32-byte capability per Core tunnel; `/app/file` and `/app/fs/read` serve any path to
   the secret holder, so the secret is the boundary for the machine. Same-origin requests are
   refused, and the Core proxy has its own listener (one for all Cores) with `CSP: sandbox` on every
   response, so a Core's content never shares an origin with the file routes or runs scripts. Cores
   share the proxy origin with each other; the sandbox is what keeps one Core's document from
   calling another. Alternative: per-route scopes, or a listener per Core as today, if a same-user
   process or extension could read the secret or the sandbox proves leaky.
8. **Unshare on a Core is no longer retried while the backend is down (D-100).** Default: it fails
   and the owner retries, as on the App. Alternative: the owed retry on both hosts.
9. **Legacy importer deleted (D-37).** Default: users of the 2023 JSON-era Modrinth App lose the
   one-time import. Alternative: a new importer for today's Modrinth `app.db`.
10. **Desktop actions from a tab act on the App's desktop (D-67).** Default: dialogs, links and
    folder opens started in a tab open on the machine running the App. Alternative: links open in
    the tab's own browser if tabs become a remote-control product.
11. **Account and social pages stay labrinth-shaped (D-85, D-90).** Default: upstream pages reach
    the backend through `SelfHostedFeature` with the backend frozen to labrinth's shapes, and
    upstream's shared-instances module the same way. Alternative: an Amberite account and sharing
    API.
12. **`PackUpdateHooks` (D-45).** Default: a narrow host callback around pack updates, so the App
    captures the user's options base before the pack overwrites files. It bends "the engine tells,
    never asks". Alternative: the App owns pack-update orchestration, or accepts losing local option
    changes on updates.
13. **Events lost to lag become a resync (D-36).** Default: a lagging socket sends `Resync` and the
    UI refetches; log bursts can still delay other events (the doc's Watch item). Alternative: log
    lines on their own channel, read from the live log buffer.
14. **Blog removal takes Modrinth's changelog tooling (D-11).** Default: `collect-changelog.ts`,
    `build-theseus-release-notes.ts` and the review-changelog skill go and are ignored on sync;
    `theseus-release.yml` then calls a missing script (reported). Alternative: keep Modrinth-style
    release notes by dropping three ignore entries and restoring the files.

Decided rather than revisited: the error wire format folds the backend's `code` into `error` with
labrinth's spellings (D-26; nothing has shipped, so it is cheap either way), and the rebuilt dataset
stays in a scratch folder for the developer to copy in (D-109; AGENTS.md's one-way data rule).
