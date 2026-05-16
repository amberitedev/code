# apps/app — Tauri App Shell (Rust)

Tauri shell wrapping `apps/app-frontend/` as a WebView. Registers command plugins, manages the window lifecycle, and bridges the frontend to `theseus` (Minecraft engine) and `amberite-lib` (Core integration).

## Runtime Processes

Three separate processes form the full runtime:

| Process | What |
|---------|------|
| `apps/app` | Tauri Rust shell + WebView (this package) |
| `apps/app-frontend` | Vue 3 SPA running inside the WebView (same OS process as above) |
| `apps/core` | Optional Axum HTTP server on `localhost:16662` — separate process |

Frontend talks to Core via `tauriFetch` (HTTP), not Tauri IPC. Core communication path: `CoreApiClient` → `PlatformAdapter.fetchFn` = `tauriFetch` → `apps/core` REST/WS.

## Initialization Sequence (order is critical)

1. `start_logger()` — must be first
2. `EventState::init(app_handle)` — **before** `State::init`; events emitted during migrations are silently dropped if this is missing
3. `State::init("amberite")` — opens `app.db`, runs SQLite migrations
4. Vue mounts → calls `invoke('initialize_state')` → triggers steps 2–3
5. Vue `onMounted()` calls `show_window` — window starts `visible: false` to avoid white flash

## Key Rust Dependencies

| Crate | Source | Role |
|-------|--------|------|
| `theseus` | `packages/app-lib` | Full Minecraft launcher engine — profiles, auth, process mgmt |
| `amberite-lib` | `packages/amberite-lib` | Core process control, `AppSettings`, OS keychain session storage, health/setup commands |
| `daedalus` | `packages/daedalus` | Minecraft metadata types (`VersionManifest`, loaders) |

**`theseus::State`** — singleton holding `DirectoryInfo`, `SqlitePool`, `ProcessManager: DashMap<Uuid, Process>`, semaphores, `DiscordGuard`, `FriendsSocket`. `State::get()` busy-waits if called before `State::init()` — logs "this should never happen!" but doesn't crash or return an error.

**`amberite-lib::AppSettings`** — stores non-secret local preferences only: `core_url`, `display_name`, and `auto_launch_core`. Amberite session JWTs live in the OS keychain via `amberite-lib::session`; app-launched Core pairing uses the one-time `.setup_secret`, not `.local_token`. `is_core_running()` creates a new `reqwest::Client` per call — do not call in a polling loop.

**`daedalus`** — use `native_arch()` not `native()` for ARM architecture detection.

## Key JS/TS Dependencies (consumed by `apps/app-frontend`)

| Package | Role |
|---------|------|
| `@amberite/api-lib` | `CoreApiClient` (typed HTTP to Core) + `PlatformAdapter` (tauriFetch abstraction) |
| `@modrinth/api-client` | Modrinth REST client — projects, versions, users, Archon servers |
| `@modrinth/ui` | All Vue components, layouts, DI providers |

`@amberite/api-lib` vs `@modrinth/api-client` are entirely separate — Core API calls use the former, Modrinth API calls use the latter. `CoreConnectionMonitor` pings `/health` every 10s; `CoreWsConnection` handles console/stats WebSocket.

Pinia stores: `themeStore` (theme + 9 feature flags), `breadcrumbsStore`, `errorsStore`.

DI providers: `provideCoreClient`, `provideModrinthClient`, `provideAuth` — injected at the app root.

## File Map

| Path | What |
|------|------|
| `src/main.rs` | Tauri builder, plugin registration, window lifecycle |
| `src/api/` | All Tauri command plugins — see `src/api/AGENTS.md` |
| `src/error.rs` | Tracing span-aware error display utility |
| `src/macos/` | macOS deep link handling — mutex-latched payload for pre-init events |
| `src/updater_impl.rs` | Update download/install (feature-gated) |
| `src/updater_impl_noop.rs` | Stub when `updater` Cargo feature is off |
| `capabilities/` | Tauri security capability groups: `ads`, `core`, `plugins`, `updater` |
| `tauri.conf.json` | Product name `Amberite`, identifier `amberite`, `visible: false` |
| `tauri.{macos,linux,no-hmr}.conf.json` | Platform overrides merged on top |

## Gotchas

- **Window visibility:** `plugin-window-state` saves `POSITION | SIZE | MAXIMIZED` only. Adding `VISIBLE` to the flags breaks the white-flash fix.
- **`updater` feature flag:** compile-time Cargo feature, not a runtime toggle. Linux excludes it entirely. `MODRINTH_EXTERNAL_UPDATE_PROVIDER` env var also disables the built-in updater at runtime.
- **Profile path = profile ID.** `path` is both the DB identifier and the filesystem directory name. `profile_edit` only patches the DB — it does not move the directory on disk.
- **`EventState` before `State`:** inverting init order causes silent event loss during DB migrations with no error surfaced.

## Build

| Task | Command | From |
|------|---------|------|
| Dev | `pnpm app:dev` | repo root |
| Build | `pnpm build` | `apps/app/` |
