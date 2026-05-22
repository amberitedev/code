# Amberite Upstream Diff Audit - HUMAN ONLY

Snapshot date: 2026-05-15. Base: `upstream/main`. Current diff size: 350 files, about 47k insertions and 2.1k deletions.

Do not treat this as machine-consumable project truth. It is a human review snapshot of a messy fork state and may intentionally mention broken or stale code.

## What changed at a high level

- New Amberite Core service under `apps/core`, with Axum APIs for instances, console, files, backups, mods, properties, stats, auth, migrations, Docker, tests, and a large vendored `swc_common` patch tree.
- Desktop app now tries to bridge to Core through Rust (`apps/app/src/api/amberite`, profile creation changes, manual Tauri window builder), frontend adapters, and a new `@amberite/amberite-api` package.
- App frontend adds `/server/:id` and `/synced/:id`, Core-backed server management pages, synced instance reuse, creation-flow instance type, Core install/browse flows, and several dev/mock changes.
- Shared `packages/ui` server-management code was rewritten in-place from Modrinth Archon/Kyros assumptions toward Core assumptions.
- New design inspector and Tari DevTools packages were added, plus devtools/Vite/Storybook changes.
- Root/tooling/docs added human/planning docs, changed agent docs, changed scripts, changed lockfiles, added Supabase functions/schema, and committed Minecraft runtime jars.

## Most likely causes of current app bugs

- `apps/app-frontend/src/pages/server/Index.vue`: `profilePath`, `profile`, and `coreInstanceId` are captured once from `route.params.id`. Vue can reuse the parent when navigating `/server/a` to `/server/b`, so URL changes while the layout keeps the old Core instance.
- `apps/app-frontend/src/pages/synced/Index.vue`: `coreInstanceId` is captured once and never updates after `fetchInstance()`. Synced Core-backed children can show stale or empty content after route param changes.
- `apps/app-frontend/src/pages/synced/Index.vue`: `provideCoreClient(new CoreApiClient(...))` happens after an `await`. Vue `provide()` should happen before top-level awaits; Core injection can fail or warn in async setup.
- `apps/app-frontend/src/pages/synced/ServerSetup.vue`: `connectSocket(props.coreInstanceId)` runs only on mount. If the prop changes, the socket does not reconnect.
- `apps/app-frontend/src/pages/synced/Index.vue`: child `RouterView`/`Suspense` are keyed by `instance.path`, not `route.path`/`route.fullPath`, so tab/content transitions can reuse stale async state.
- `apps/app-frontend/src/routes.js` and `pages/synced/Index.vue`: route meta uses `?Instance`, but synced page sets breadcrumb `SyncedInstance`; breadcrumbs can be unresolved/stale.
- `apps/app-frontend/src/pages/Servers.vue`: Core server list is cached with `staleTime: Infinity`, and Core IDs/profile paths may be mixed, so clicking servers can route to IDs that `/server/:id` cannot resolve with `getProfile()`.
- `apps/app/src/api/profile_create.rs` and `apps/app-frontend/src/providers/setup/creation-modal.ts`: Core instance provisioning appears duplicated. Rust profile creation provisions Core, then frontend creates another Core instance and calls `editProfile(... core_instance_id ...)`.
- `apps/app/src/api/profile.rs`: `EditProfile` does not include `kind` or `core_instance_id`, so the frontend patch for `core_instance_id` is likely ignored.
- `apps/app/src/api/profile_create.rs`: posts to `/api/v1/instances`, but Core routes are under `/instances`; server/synced creation can fail or produce profiles with missing Core links.
- `apps/core/src/infrastructure/auth/jwks.rs`: Supabase JWT validation likely lacks `aud = authenticated`, so protected Core endpoints can all return 401.
- `packages/amberite-lib/src/core_launcher.rs`: hardcodes Core URL `http://localhost:7000`, while current Core/app use `16662`; also spawns Core with unsupported `--data-dir`.
- `apps/app/tauri.macos.conf.json` and `apps/app/src/main.rs`: macOS window config was removed from Tauri config and replaced with undecorated manual builder behavior, likely losing native traffic lights/traffic-light positioning.
- `apps/app-frontend/src/components/ui/WindowControls.vue`: the restored expanded hit area now has `z-index: -1`; clicks around icons may hit the drag region instead of controls.
- `packages/design-inspector/components/InspectorOverlay.vue`: global capture-phase pointer/click handlers block app events whenever inspector/pick mode is active or stuck.
- `packages/design-inspector/components/DesignInspectorRoot.vue`: monkey-patches `history.pushState`/`replaceState`, observes the whole body, and warms network caches. Keep strictly opt-in.
- `packages/ui/src/layouts/shared/content-tab/layout.vue`: wraps content in `TabGroup`; if `tabs`/slot behavior is wrong in some state, content can disappear. Side filter defaults can also hide server-only items.
- `packages/ui/src/composables/server-manage-core-runtime.ts`: stubs filesystem auth/operations and reports socket connect success before real heartbeat/state. File UI and console can appear connected but remain stale/disconnected.
- `packages/ui/src/layouts/wrapped/hosting/manage/files.vue`: `extractFile` ignores dry-run/override and always unzips in normal mode; root rename can drop the leading slash.
- `packages/ui/src/components/servers/backups/BackupItem.vue`: still has Kyros download behavior, while Core wrapper no longer passes Kyros URL/JWT, so backup downloads are disabled/broken.
- `packages/amberite-api/src/api.ts` and `src/client.ts`: direct fetch has no timeout; several fetch paths bypass error wrapping; relay fallback can wait 60s and cannot handle binary file payloads.

## Highest-risk security/data issues

- `apps/core/src/infrastructure/auth/jwks.rs`: valid Supabase users are not checked against `owner_user_id`; any valid token from the project may administer Core.
- `apps/core/src/presentation/router.rs`: uses permissive CORS and ignores configured `allowed_origin`.
- `apps/core/src/application/fs_service.rs`: ZIP extraction uses raw entry names and is vulnerable to ZIP slip; several file operations do not canonicalize symlink escapes.
- `apps/core/src/application/backup_service.rs`: backup restore also extracts ZIP paths unsafely.
- `supabase/functions/provision-core-machine-account/index.ts`: trusts `owner_user_id` from request body while using service role; if JWT verification is off, it is account provisioning by anyone.
- `supabase/migrations/20250612000000_api_lib_schema.sql`: policies reference `core_group_members` before creation, likely migration failure; relay UUID columns do not match api-lib hostname/JWT string IDs.
- `packages/amberite-lib/src/auth.rs` and `settings.rs`: Supabase JWT is stored plaintext in settings while `keyring` is unused.
- `packages/amberite-lib/src/core_launcher.rs`: `install_core()` downloads and extracts executable ZIPs without checksum/signature validation.

## Changes that should be reverted or separated first

- Revert or isolate `apps/app-frontend/src/components/ui/friends/FriendsList.vue` mock friends. It changes real app behavior unrelated to Core.
- Restore `download` support in `packages/ui/src/components/base/Button.vue`, `OverflowMenu.vue`, website version pages, and `VersionSummary.vue` unless every caller was intentionally migrated. Current diff changes download links into navigation/open behavior.
- Move Core-specific UI behavior out of shared Modrinth components where practical: backup modals, `SaveBanner`, server settings pages, `server-manage-core-runtime`, and wrapped hosting pages now require `injectCoreClient()` and can break non-Core consumers.
- Keep upstream shared settings layouts intact or create Amberite/Core variants instead of deleting Modrinth sections in shared files.
- Move `HOSTING-REWRITE.md`, `.plan/active/*.md`, and similar planning docs out of source-facing app paths or keep them explicitly human-only.
- Remove committed runtime/cache binaries: `libraries/**` and `versions/1.20.4/server-1.20.4.jar`, unless there is a deliberate vendoring policy.
- Remove or explicitly wire/document `apps/core/patches/swc_common-0.37.5/**`; it is huge vendored third-party source and may be dead weight if not patched in Cargo config.
- Make Tari DevTools and CDP port `9222` opt-in. Current debug builder exposes CDP in every debug app run and can collide with other tools.
- Keep design inspector and Vue DevTools behind explicit env flags, with internal no-op guards in `DesignInspectorPlugin.install()`.
- Move or untrack `PROJECT-HUMAN-ONLY.md`/`opencode.json` if they are private/local. Do not let agents rely on human-only docs as project truth.

## Core/API contract mismatches to fix before debugging UI

- Standardize Core URL/port: Core default, app Rust bridge, `amberite-lib`, api-lib, Tauri HTTP capabilities, and CSP currently disagree between `7000`, `16662`, and wildcard localhost.
- Standardize auth: Core validates Supabase JWTs; `amberite-lib` reads `.local_token`; desktop adapter exposes `getLocalCoreToken`; Supabase provision function returns machine passwords. Pick one model.
- Standardize instance creation owner: only one layer should create the Core instance. Prefer an Amberite-specific frontend/api flow that creates Core, validates returned `id`, then persists `core_instance_id` once.
- Add rollback/atomicity for server/synced creation. Today failures can leave local profiles without Core instances or Core instances without profiles.
- Align console WS flow. Core uses `POST /ws-token` returning `{ ticket }` and `/instances/:id/console?ticket=...`; `amberite-lib` uses stale `/instances/:id/ws-token` and `?token=`.
- Align properties keys. Core currently expects exact Minecraft keys like `server-port`; UI/shared settings may send normalized snake_case/camel keys.
- Align Supabase relay IDs. Database expects UUIDs, api-lib sends hostnames/JWT strings; relay cannot work as written.
- Add request timeouts and cancellation to `@amberite/amberite-api` before routing pages depend on it.

## Keepable changes

- App frontend `saveWindowState(POSITION | SIZE | MAXIMIZED)` is correct; do not save `VISIBLE`.
- Moving `WindowControls` back into the titlebar region is correct; review only the hit-area/z-index details.
- Gating design inspector/Vue DevTools by explicit env is correct; add internal guards and split devtools flags.
- Lifecycle cleanup refactors in `Instance.vue`, `QuickInstanceSwitcher.vue`, `instance/Files.vue`, `instance/Index.vue`, and `synced/Index.vue` are directionally good, but add unmounted guards for async listener registration.
- `packages/app-lib` schema additions for `kind` and `core_instance_id` are reasonable if kept minimal and only used as opaque persisted metadata.
- `apps/app-frontend/src/adapters/desktop.ts` is the right kind of separation point for platform/Core communication, but it should not throw for missing Supabase env in local-only Core flows.
- `TabbedModal` Suspense fallback is likely a safe UI improvement.
- `TabGroup` is small and useful, but needs accessibility polish before broad reuse.

## Suggested cleanup order

1. Fix contract mismatches: Core port, auth, instance creation path, `core_instance_id` persistence, route URLs.
2. Fix route reactivity: key `/server/:id` and `/synced/:id` parents or make their Core IDs reactive; move `provideCoreClient()` before awaits.
3. Disable or remove dev tooling from normal app runs: design inspector, Vue DevTools, CDP/Tari unless explicitly enabled.
4. Revert unrelated behavior changes: mock friends, download prop removals, Storybook script renames without aliases, planning/vendor artifacts.
5. Split Core-specific UI from shared Modrinth UI to reduce accidental regressions and make upstream merges easier.
6. Harden Core: JWT ownership checks, CORS, file path canonicalization, ZIP extraction, backup restore, port validation, process readiness.
7. Simplify `@amberite/amberite-api`: direct Core client first; defer Supabase relay/auth/monitoring until schema and identity model are correct.
