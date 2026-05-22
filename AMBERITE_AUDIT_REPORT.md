# Amberite Repository Audit Report

Generated: 2026-05-16. Method: broad orientation agents, focused subsystem agents, and second-pass validation agents. This is a static audit; only the Tauri shell agent reported running `cargo check --all-targets` and `cargo check --all-targets --features updater` successfully in `apps/app`.

## Executive Summary

Amberite is currently a Modrinth desktop-app fork with a new custom Core server manager layered into the app. The highest-risk problems are not cosmetic. Production Core auth/pairing is not wired end-to-end, Core lifecycle and restore paths can corrupt process/data state, Core tests are skipped by root CI, and release/update/deep-link infrastructure still points at Modrinth. The desktop window-control bug is most likely a titlebar drag-region/event handling issue, not a missing Tauri permission.

Severity counts in this report: 12 critical, 19 high, 18 medium, and 6 low/cleanup items.

## Critical Findings

### C1. Production Core auth and pairing flow is not wired in the app

Files: `packages/amberite-api/src/client.ts`, `packages/amberite-api/src/auth.ts`, `apps/app-frontend/src/adapters/desktop.ts`, `apps/app-frontend/src/App.vue`, `apps/app-frontend/src/providers/setup/auth.ts`, `apps/core/src/presentation/extractors.rs`, `apps/core/src/presentation/handlers/setup.rs`.

`CoreApiClient` can send `Authorization: Bearer <jwt>` and the desktop adapter can store that JWT in the OS keychain, but app code does not call `startMicrosoftLogin`, `completeMicrosoftLogin`, `completeSetup`, `completeLocalSetup`, or `setCurrentJwt`. Current sign-in is Modrinth auth only. In production, Core private routes require a valid owner JWT, so `listInstances`, `createInstance`, `issueWsTicket`, and server pages fail unless an external process manually paired Core and stored a matching token. Fix by adding a real Amberite/Core auth bootstrap: obtain the proper JWT, store it via `adapter.setCurrentJwt`, complete local/code pairing, and gate Core pages until ready. Test with `AMBERITE_DEV=false` and a local JWKS fixture.

### C2. Core dev auth bypass can expose the full admin API

Files: `apps/core/src/config.rs`, `apps/core/src/presentation/extractors.rs`, `apps/core/src/presentation/router.rs`, `apps/core/.env.example`.

Debug builds default `AMBERITE_DEV=true`; `AuthUser` accepts all protected routes without a token and CORS is permissive. If a debug Core binds to `0.0.0.0`, network clients can create/delete instances, read/write files, start processes, and use admin endpoints. Default dev mode should be false or startup should refuse dev mode on non-loopback unless a second explicit unsafe override is set. Add tests for default config, non-loopback rejection, and unauthenticated protected routes.

### C3. Concurrent Core start can spawn duplicate JVMs and lose actor handles

Files: `apps/core/src/application/instance_status_service.rs`, `apps/core/src/infrastructure/process/instance_actor.rs`.

`start_instance` checks `state.instances.contains_key(id)`, then awaits DB/Java/spawn work before inserting the handle. Two concurrent `/start` requests can both spawn a server in the same directory/port; the second handle can overwrite the first, leaving an untracked JVM. Add per-instance lifecycle locking or an atomic DB/state transition `offline|crashed -> starting`, use non-overwriting map insertion, and roll back on spawn failure. Add concurrent start tests with a counting mock spawner.

### C4. Core actor insertion race can miss running state or leave stale handles

Files: `apps/core/src/application/instance_status_service.rs`, `apps/core/src/infrastructure/process/instance_actor.rs`.

`spawn_actor` starts the actor before `state.instances.insert`. A fast process can emit `Done (` before insertion, leaving status stuck `starting`; if it exits immediately, actor removal removes nothing and the service then inserts a dead handle. Insert/register the handle before the actor can process output, remove the map-presence gate for readiness, and ensure actor initialization failures clean state. Test with fake processes that emit ready/exit immediately.

### C5. Backup restore can destroy current instance data on partial failure

Files: `apps/core/src/application/backup_service.rs`.

`restore_backup` ignores failure of the pre-restore backup, deletes the entire instance directory, then extracts the selected zip. Corrupt zip, disk/full extraction failure, or guarded extraction rejection can leave the original instance destroyed. Validate the archive first, require pre-restore backup success, restore into a temp directory, then atomically swap. Add tests proving corrupt/traversal backups leave original files intact.

### C6. Core tests are skipped by root CI/Turbo

Files: `apps/core/package.json`, `package.json`, `turbo.jsonc`, `.github/workflows/turbo-ci.yml`.

`apps/core/package.json` has no `test` or `lint`; root CI runs Turbo `lint test`, and `pnpm turbo run test --filter=@modrinth/core` executes no tasks. Core is an isolated Cargo workspace, so root Rust checks do not cover it either. Add Core scripts such as `test: cargo test --tests` and CI steps for `apps/core`; add lint/fmt/clippy as appropriate.

### C7. Convex relay is not deliverable to Core

Files: `packages/amberite-api/src/transport.ts`, `packages/amberite-api/src/convex-relay.ts`, `packages/amberite-api/src/heartbeat.ts`, `packages/convex/messaging.ts`, `packages/convex/presence.ts`, `apps/core/src/**`.

Convex relay functions require Convex auth, but Core has no Convex client, auth token, background poller, or heartbeat task. `CoreApiClient.request()` still performs direct HTTP and ignores relay payloads, so `online-relay` is not a working transport. Add a Core-side service that authenticates as the paired Core, heartbeats, polls pending messages, executes operations, and completes messages, or remove/rename relay states until implemented.

### C8. Remote pairing creates Convex paired state without configuring Core

Files: `packages/convex/presence.ts`, `apps/core/src/application/pairing_service.rs`, `apps/core/src/presentation/handlers/setup.rs`, `packages/amberite-api/src/client.ts`.

`presence:claimPairingCore` can mark Convex rows/cores as claimed, but the actual Core SQLite `core_config` remains unpaired until `POST /setup` succeeds. If `connectionUrl` is absent/unreachable, Convex can claim a Core that still rejects protected routes. Make claim transactional with Core setup, or add explicit setup-pending state and reconciliation.

### C9. Updater/release pipeline still targets Modrinth

Files: `apps/app/tauri-release.conf.json`, `apps/app-frontend/src/App.vue`, `.github/workflows/theseus-build.yml`, `.github/workflows/theseus-release.yml`.

Release updater endpoint and Linux manual update checks point to `https://launcher-files.modrinth.com/updates.json`; workflows and generated manifests use Modrinth App names, URLs, and likely signing keys. Amberite builds can fetch/publish/sign against the wrong channel. Create Amberite update infrastructure, signing keys, artifact names, and workflow names, or disable updater/release workflows until ready.

### C10. Custom URL scheme still registers `modrinth://`

Files: `apps/app/tauri.conf.json`, `apps/app/Info.plist`, `packages/ui/src/components/modal/OpenInAppModal.vue`, `apps/frontend/src/pages/[type]/[id].vue`.

Installing Amberite may steal/conflict with Modrinth App deep links, and Modrinth web links can open Amberite. Use `amberite://`, or intentionally support both schemes with documented compatibility/collision behavior.

### C11. Local Core server list opens hosted-server route

Files: `apps/app-frontend/src/pages/Servers.vue`, `packages/ui/src/components/servers/ServerListing.vue`, `packages/ui/src/layouts/wrapped/hosting/manage/root.vue`.

Core instances are cast into hosted `Archon.Servers.v0.Server` entries, but `ServerListing` hardcodes navigation to `/hosting/manage/${server_id}`. Core management lives at `/server/:profilePath`; clicking a local Core server can enter hosted Archon/Kyros pages with a Core ID. Add a route-builder/nav callback or a Core-specific listing that routes to `/server/${encodeURIComponent(profile.path)}`.

### C12. App window controls likely fail because they sit inside a drag-region wrapper

Files: `apps/app-frontend/src/App.vue`, `apps/app-frontend/src/components/ui/WindowControls.vue`, `apps/app/capabilities/core.json`, `apps/app/tauri.conf.json`, `apps/app/src/main.rs`.

Permissions look correct for window `main`: minimize, toggle-maximize, close, set-decorations, start-dragging are allowed. The likely bug is DOM/event handling: the right titlebar wrapper has `data-tauri-drag-region`, while `WindowControls` relies on unsupported `data-tauri-drag-region-exclude` plus CSS. Tauri v2 only recognizes `data-tauri-drag-region`; the custom exclude attribute is repo-local. Minimal patch: remove `data-tauri-drag-region` from the interactive right-side wrapper, keep drag attributes only on noninteractive regions, and replace inline click promises with named async handlers that catch/log errors. Verify direct DevTools calls to `getCurrentWindow().minimize()`, `toggleMaximize()`, and `close()`.

## High Findings

### H1. First-run setup trusts caller-supplied auth root

Files: `apps/core/src/presentation/handlers/setup.rs`, `apps/core/src/application/state.rs`, `packages/amberite-api/src/client.ts`, `packages/convex/presence.ts`.

After pairing-code/local-secret verification, Core persists caller-supplied `auth_jwks_url`, `auth_audience`, and `owner_user_id`. Anyone with the setup credential can set their own JWT authority. Setup should validate a signed setup assertion or pin/allowlist JWKS/issuer and derive owner from authenticated claims.

### H2. Local setup secret is remote-usable and weakly protected on Windows

Files: `apps/core/src/application/state.rs`, `apps/core/src/presentation/handlers/setup.rs`, `apps/app-frontend/src/adapters/desktop.ts`.

`.setup_secret` is accepted by unauthenticated `POST /setup` from any client and gets no Windows ACL hardening. Restrict local-secret setup to loopback, store via OS-protected storage or owner-only ACLs, add expiry/single-use behavior, and test non-loopback rejection.

### H3. Core symlink/path escapes remain in filesystem/log APIs

Files: `apps/core/src/application/fs_service.rs`, `apps/core/src/application/log_service.rs`, `apps/core/src/presentation/handlers/logs.rs`.

Directory listing can traverse symlink directories; log/crash reads follow symlinks; create/write paths can create directories outside via symlink parents before later rejection. Canonicalize final targets, reject symlink components/leafs for writes, and test symlink/junction escapes.

### H4. Modrinth-provided mod filenames and downloads are not fully validated

Files: `apps/core/src/application/mod_service.rs`, `apps/core/src/infrastructure/minecraft/mrpack.rs`.

User-upload filenames are sanitized, but filenames from Modrinth API are joined directly under `mods/`; downloaded bytes are not verified against advertised hashes. `.mrpack` downloads may skip integrity and can panic on empty downloads. Sanitize all external filenames, require/verify hashes, and return structured errors for malformed manifests.

### H5. SQLite foreign-key cascades are likely inactive

Files: `apps/core/src/infrastructure/db/mod.rs`, `apps/core/migrations/002_full_rewrite.sql`, `004_mods.sql`, `007_backups.sql`, `apps/core/src/infrastructure/db/instance_repo.rs`.

Migrations declare `ON DELETE CASCADE`, but SQLite requires `PRAGMA foreign_keys=ON` per connection. Deleting instances can orphan mods, manifests, backups, and schedules. Enable foreign keys in SQLx connection options and add cleanup migration/tests.

### H6. Stop/kill can report success when actor command delivery failed

Files: `apps/core/src/application/instance_status_service.rs`.

`stop_instance` and `kill_instance` ignore `cmd_tx.send` failures while command send handles them. Closed/stale actor channels can produce `200 { ok: true }` without stopping anything. Map failures to `ActorDead`, remove stale handles, and reconcile status.

### H7. Start/delete/restart/restore races can orphan processes or overwrite live files

Files: `apps/core/src/presentation/handlers/instances.rs`, `apps/core/src/application/instance_status_service.rs`, `apps/core/src/application/backup_service.rs`.

Delete checks `contains_key` once while start can be between DB read and handle insert. Restart has a gap between stop/removal and restart. Restore can check not-running then proceed without lifecycle lock. Share a per-instance lifecycle lock across start/stop/kill/restart/delete/restore/modpack/destructive FS operations.

### H8. Startup restore races with HTTP lifecycle requests

Files: `apps/core/src/main.rs`, `apps/core/src/application/instance_service.rs`.

`restore_instances` runs in the background while HTTP starts accepting requests. User starts can race restore starts, and broad transient-status reset can mark a live starting instance offline. Run restore before binding or gate lifecycle routes behind `restore_complete`; also use lifecycle locks.

### H9. Instance creation/install failures are silent and start does not validate readiness

Files: `apps/core/src/application/instance_service.rs`, `apps/core/src/application/instance_status_service.rs`, `apps/core/src/presentation/handlers/console.rs`, `packages/amberite-api/src/client.ts`, `apps/app-frontend/src/providers/setup/creation-modal.ts`.

`POST /instances` creates a DB row, then downloads the server jar in the background. Failures are logged only; API lib/app do not consume progress SSE; start can spawn before files exist. Add install state/errors, replayable progress, start blocking until ready, and UI progress/failure handling.

### H10. Unexpected process exits are recorded as offline, not crashed

Files: `apps/core/src/infrastructure/process/instance_actor.rs`.

Final status uses `handle.is_running()` after the loop, so normal unexpected exits become `offline`. Track stop intent and process exit status; mark unexpected exits `crashed`.

### H11. Core content install flow calls hosted Archon APIs

Files: `apps/app-frontend/src/pages/server/Index.vue`, `apps/app-frontend/src/providers/setup/server-install-content.ts`, `apps/app-frontend/src/pages/Browse.vue`.

Core browse flow sets `source=core`, but queued installs always flush through `client.archon.content_v1.addAddons(...)` and require hosted `worldId`. Branch Core installs to `CoreApiClient.addMod(serverId, versionId)` and update installed state via `listMods`.

### H12. Synced server start/stop controls are no-op

Files: `apps/app-frontend/src/pages/synced/Index.vue`, `apps/app-frontend/src/pages/synced/ServerSetup.vue`.

`serverRunning` is hardcoded false and start/stop actions are empty, while `ServerSetup` has real Core runtime state. Wire actions to `CoreApiClient.start/stop`, consume the slot/exposed state, and disable while pending.

### H13. Core upload methods bypass the platform adapter

Files: `packages/amberite-api/src/api.ts`, `packages/amberite-api/src/client.ts`, `apps/app-frontend/src/adapters/desktop.ts`.

Normal Core calls use desktop `tauriFetch`; uploads use raw `XMLHttpRequest`, which can fail under production CORS. Add an adapter upload transport or Tauri upload bridge. Current Core UI does not expose upload yet, but the API boundary is wrong.

### H14. Tauri HTTP/CSP allowlists do not match configurable Core/Convex URLs

Files: `apps/app/capabilities/plugins.json`, `apps/app/tauri.conf.json`, `apps/app/src/api/amberite/mod.rs`, `packages/amberite-lib/src/settings.rs`, `apps/app-frontend/src/adapters/desktop.ts`, `packages/amberite-api/src/convex-relay.ts`.

`core_get_url` can return user-configured URLs, but Tauri HTTP scopes allow only fixed localhost/tailnet/Modrinth patterns. Convex URLs are also required by desktop adapter yet not obviously allowlisted. Either constrain settings to allowlisted origins, expand scopes for intended origins, or proxy through validating Rust commands.

### H15. Release/build workflows omit Amberite dependencies and still publish Modrinth artifacts

Files: `.github/workflows/theseus-build.yml`, `.github/workflows/theseus-release.yml`, `apps/app/tauri.conf.json`, `apps/app/tauri-release.conf.json`.

App build path filters omit `packages/amberite-api/**` and `packages/amberite-lib/**`. Workflows and artifact names still use Modrinth App. Add missing paths and rehome/disable Modrinth release infrastructure.

### H16. Inherited Modrinth apps remain active in workspace and CI/release surfaces

Files: `Cargo.toml`, `pnpm-workspace.yaml`, `package.json`, `apps/frontend/**`, `apps/labrinth/**`, `apps/docs/**`, `apps/daedalus_client/**`, `apps/app-playground/**`, related workflows.

Root Cargo includes Labrinth, Daedalus, app-playground, and app; Core is isolated. pnpm includes all `apps/*`. Frontend/Labrinth/Daedalus workflows target Modrinth infra. Decide whether these are upstream-only, disable/exclude from Amberite CI, or rehome them.

### H17. Telemetry, crash, support, and runtime content endpoints point at Modrinth

Files: `apps/app-frontend/src/helpers/analytics.ts`, `apps/app-frontend/src/main.js`, `apps/app-frontend/src/components/ui/ErrorModal.vue`, `apps/app-frontend/src/config.ts`, `packages/app-lib/.env.prod`, `packages/app-lib/src/state/mr_auth.rs`, `packages/app-lib/src/lib.rs`, `packages/api-client/src/core/abstract-client.ts`.

PostHog, Sentry, support, Intercom, auth, content APIs, and user-agent/support metadata are Modrinth-owned. Some content/auth coupling may be intentional, but privacy/support endpoints should be rebranded or disabled until Amberite-owned.

### H18. Core UI is coupled to Modrinth/Archon server types and DI

Files: `packages/ui/src/providers/server-context.ts`, `packages/ui/src/layouts/core/server-manage/runtime.ts`, `packages/ui/src/layouts/core/server-manage/root.vue`, `packages/ui/src/layouts/shared/console/layout.vue`.

Core layouts cast Core instances into `Archon.Servers.v0.Server`, provide `ModrinthServerContext`, and the shared console unconditionally injects Modrinth client for mclogs sharing. Introduce Core-native context/capability interfaces and make log sharing an injected optional handler.

### H19. Desktop app bridge and amberite-lib have little/no test coverage

Files: `apps/app/package.json`, `apps/app/src/api/amberite/mod.rs`, `packages/amberite-lib/src/core_launcher.rs`, `packages/amberite-lib/src/session.rs`.

Tauri command names, keychain JWT behavior, Core URL resolution, local setup secret lookup, and Core launcher archive extraction are not meaningfully tested. Add unit tests where possible and Tauri-mock/integration tests for invoke names.

## Medium Findings

1. JWT validation lacks issuer binding and JWKS cache is not URL-keyed: `apps/core/src/infrastructure/auth/jwks.rs`, `apps/core/src/presentation/extractors.rs`. Persist/validate issuer and key cache by URL.
2. Pairing lockout is unauthenticated permanent DoS until restart: `apps/core/src/presentation/handlers/setup.rs`. Use rate limits/cooldowns and allow correct credential after cooldown.
3. Core relay identity/status contract drifts from api-lib: `apps/core/src/presentation/handlers/relay.rs`, `packages/amberite-api/src/core-relay.ts`, `packages/amberite-api/src/transport.ts`. Preserve message IDs and make wait/status mode-aware.
4. Core relay auth does not authorize sender/recipient claims: `apps/core/src/presentation/handlers/relay.rs`, `packages/convex/messaging.ts`. Derive or authorize sender/recipient from authenticated principal.
5. Direct-only/self-hosted mode is blocked by required Convex URL assumptions: `packages/amberite-api/src/adapter.ts`, `packages/amberite-api/src/types.ts`, `apps/app-frontend/src/adapters/desktop.ts`, `apps/core/src/presentation/handlers/setup.rs`. Make Convex optional for direct-only mode.
6. Core URL is cached permanently by `CoreApiClient`: `packages/amberite-api/src/client.ts`. Add invalidation/TTL or react to settings changes.
7. Creation modal hardcodes Core port `25565`: `apps/app-frontend/src/providers/setup/creation-modal.ts`. Allocate/validate available ports or let Core choose.
8. Core creation UI exposes unsupported client-oriented loader choices for server/synced instances: `apps/app-frontend/src/providers/setup/creation-modal.ts`, `packages/ui/src/components/flows/creation-flow-modal/**`. Restrict loader options by instance kind.
9. Browser online/offline listeners leak across routes: `apps/app-frontend/src/pages/synced/Index.vue`, `pages/instance/Index.vue`, `pages/library/Index.vue`, `pages/Index.vue`, `pages/Browse.vue`. Use named handlers and cleanup/composable.
10. App root global listeners/timers lack cleanup: `apps/app-frontend/src/App.vue`. Mostly lifetime/HMR risk; store unlisten/interval IDs.
11. Add-server modal can crash after `list()` failure: `apps/app-frontend/src/components/ui/install_flow/AddServerToInstanceModal.vue`. Default caught result to `[]`.
12. Core console send/load errors are swallowed in UI: `apps/app-frontend/src/pages/server/Console.vue`; Core WS command failures are also ignored in `apps/core/src/presentation/handlers/console.rs`. Surface errors.
13. Stats player-count probe aborts on unrelated events and ignores send failure: `apps/core/src/application/stats_service.rs`. Continue until timeout and handle closed actor channels.
14. `java_version` override is accepted but ignored; Java detection can misclassify `java`: `apps/core/src/presentation/handlers/instances.rs`, `apps/core/src/application/instance_status_service.rs`, `apps/core/src/infrastructure/minecraft/java.rs`.
15. Backup creation snapshots running servers without quiescing; schedule validation allows bad retention/cron: `apps/core/src/application/backup_service.rs`, `backup_scheduler.rs`. Require offline or safe save-off/save-all/snapshot flow; validate retention/cron on write.
16. Core files/backups UI is incomplete and destructive backup actions lack confirmation: `packages/ui/src/layouts/core/server-manage/files.vue`, `backups.vue`. Reuse shared file/backup layouts or add confirmations/error state.
17. Shared console state is global and can leak logs between servers: `packages/ui/src/composables/server-console.ts`. Key/provide state per server.
18. Package-manager and Rust toolchain pins drift: root `package.json`, `.nvmrc`, `apps/app-frontend/package.json`, root/Core `rust-toolchain.toml`, root/Core `Cargo.toml`, `.github/workflows/check-rust.yml`. Align or document intentional MSRV/toolchain split.

## Low / Cleanup Findings

1. `openSurvey()` opens Tally popup twice: `apps/app-frontend/src/App.vue`.
2. Stale `scroll_ads_window` command remains in `apps/app/build.rs` while removed from `apps/app/src/api/ads.rs`.
3. Deep-link handling swallows command errors and has a panic edge for empty macOS JSON payloads: `apps/app/src/main.rs`.
4. Ads init script references undefined `originalCreateGain`: `apps/app/src/api/ads-init.js`.
5. Core backup API reports scheduled backups as non-automated due to `automated` vs `automatic` mismatch: `apps/core/src/application/backup_scheduler.rs`, `apps/core/src/presentation/handlers/backups.rs`.
6. Core layouts have hardcoded English and accessibility gaps: `packages/ui/src/layouts/core/server-manage/*.vue`.

## Recommended Fix Order

1. Fix CI first: add and run Core tests in root CI, then add missing high-risk tests for auth, lifecycle, filesystem, backups, console/SSE, and modpack.
2. Fix the app window controls with the minimal drag-region patch and async error logging; it is a contained user-visible bug.
3. Decide production auth/pairing architecture and implement app bootstrap, Core setup, JWT storage, and route gating.
4. Lock down Core security/data-loss issues: dev-mode exposure, setup trust root, symlink escapes, restore atomicity, SQLite FKs, external filename/hash validation.
5. Fix lifecycle concurrency with per-instance locks/CAS transitions before adding more Core features.
6. Remove or quarantine Modrinth release/update/deep-link/telemetry surfaces for Amberite builds.
7. Untangle Core UI/API boundaries from hosted Archon/Modrinth assumptions.
