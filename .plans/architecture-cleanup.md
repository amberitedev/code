# Amberite Code Architecture Cleanup

Target architecture, locked-in decisions, and Definition of Done for removing duplication across Amberite (App), Copal (Core), `packages/engine`, `apps/backend`, `packages/api-client`, and the UI.

This is an outcome spec, not a step-by-step recipe. Only the architecture and code changes below are locked in. For small implementation choices not stated here, inspect the code and take the simplest path that fits the principles. If a change alters user-visible behavior beyond what is listed or weakens a safety check, stop and ask ilai.

---

## Non-Negotiable Principles

1. **Three buckets, reuse upstream by default:**
   - **Engine (`packages/engine`):** follows Modrinth App's existing code and shapes by default.
   - **Copal-specific (`apps/core` + `contracts::core`):** follows Modrinth Hosting's (`Archon`/`Kyros`) schemas and functionality directly for everything Copal supports, so the UI talks directly to Copal with zero translators.
   - **Desktop-specific (`apps/app`):** lives in `apps/app/src/<domain>`, never inside `packages/engine`.
   - Reuse what Modrinth already built unless we have a real reason to diverge. Do not churn working upstream files for style (keep `state` names, upstream engine code such as `InstanceLink::ModrinthHosting`, `packages/toasty`'s Cargo SQLite patch, and thin upstream `helpers/*` wrappers). Every deleted or replaced upstream path goes into `upstream.json` (`ignore` or `ported`).
2. **Code enforces boundaries:**
   - Crate graph, binary-crate isolation, typed Rust/TS contracts, and tests beat prose rules.
   - Core-to-App communication stays impossible at compile time (`apps/core` cannot depend on `apps/app`) and runtime (`apps/app` listens on `127.0.0.1` behind a secret path and rejects non-loopback `Host`).
   - `packages/sharing-storage` stays its own crate (depending only on `contracts` and `sharing-client`) and runs as the `copal store` subprocess, so Cargo prevents store code from touching Core keys, `CoreState`, `engine`, or `core.db`.
3. **Minimal Rust crate graph (keep existing boundaries):**
   ```text
   packages/contracts <-- packages/sharing-client <-- packages/engine <-- apps/app, apps/core
                                                  \-- packages/sharing-storage <-- apps/core
   ```
   `packages/contracts` holds wire types, `Endpoint` declarations, shared limits/rules, codegen, and transport-agnostic `client`/`server` glue (no business logic, crypto, TLS, or disk code, except tiny conversion helpers on shared enums).
4. **Every owned route is an `Endpoint`:**
   - No raw owned HTTP/WS routes (reverse proxy pass-throughs, Copal's `/store/{*rest}` and App's Core proxy, are explicitly excluded). No hand-written TS wire copies, and no `Partial<Module>` translators that silently fall through at runtime.
5. **Zero Modrinth Hosting exposure:**
   - Remove all Modrinth Hosting fallbacks (`archon.modrinth.com`, `shared-instances.modrinth.com`, Stripe, Intercom, Survey, billing, Modrinth updater feed). Missing Amberite backend config fails loudly.
6. **Naming:**
   - **Copal** stays the name of the Core (`apps/core`, `copal` binary, OS accounts, release workflow). Delete only the `core-hosting/copal.ts` alias.
   - **Amberite** (`amberite-app`, `AMBERITE_*`, `amberite/<version>`) replaces `theseus` in owned code and config (`apps/app/`, `apps/core/`, `apps/backend/`, `packages/contracts/`, `scripts/`, `.github/workflows/`, and `apps/app-frontend/src/platform/modrinth-client.ts`). Leave upstream `packages/engine` (`theseus.jar`, Java agent, upstream doc comments) and `apps/app-frontend/src/helpers/*` untouched (pass the user agent from the host via `EngineConfig` so `packages/engine` does not hardcode `modrinth/theseus/`).
7. **Two SQLite files per host:**
   - Keep `engine.db` + `app.db` on App and `engine.db` + `core.db` on Copal so `engine` never sees host tables and hosts never join engine tables.
8. **Scope & docs:**
   - No UX redesign here; keep current screens working as-is. Small in-scope behavior improvements are listed in Section 5 so the App pass verifies them.
   - New CI policy, skills, and `.coderabbit.yaml` deletion belong to the `AGENTS.md Non-Negotiable Principles` thread (if `.coderabbit.yaml` still exists during Checkpoint 1, leave it to that thread). Update `ci.yml`, `dev-runner.ts`, `AGENTS.md`, `docs/internals/architecture.md`, and `docs/internals/upstream.md` wherever this work changes or reverses them.
9. **Keep backend cost near $0.**

---

## Explicit Reversals of Recorded Docs

Update the matching sections of `docs/internals/architecture.md` and `docs/internals/upstream.md` when landing these intentional reversals:

- **Upstream hosting adapters (`upstream.md`):** Reverses keeping upstream `archon` modules and answering them via `packages/api-client/src/core-hosting/*` adapters. Instead we delete `core-hosting/*`, implement upstream's shared tab providers once over `{ client, instanceId }`, call `client.core(coreId)` directly on Copal-only screens, and record replaced/deleted upstream hosting files in `upstream.json` (`ported`/`ignore`).
- **Roles-cut (#157) handler levels (`architecture.md`):** Removes `View`/`Edit`/`Owner` generics on engine handlers under YAGNI on an owner-only Core, and removes dead per-topic access rechecks / `CoreEvent::Ended` from Copal `/events` (keeping `Dangerous` fresh-ticket checks and session-ended socket close).
- **Shared cross-host enums in `contracts` (`architecture.md`):** Moves `ModLoader` and `ReleaseChannel` (with their small conversion helpers) out of `engine` into `contracts` because Copal's hosting types need them, leaving `pub use` re-exports in `packages/engine` (`PowerState` and `ErrorCode` are already in `contracts`).
- **Per-file TS namespaces (`architecture.md`):** Switches `@amberite/contracts` from one flat namespace to per-file namespaces (`protocol`, `backend`, `core`, `engine`, `app`) once `core-hosting/*` is deleted in Checkpoint 5.
- **Unreleased protocol fallbacks & error code cleanup (`architecture.md`):** Drops the `Health.protocol ?? 1` fallback and collapses duplicate `ErrorCode` synonyms.

---

## Architecture & Code Changes

### 1. Dead Weight, Modrinth Hosting Mode Deletion & Owned Renames

- **Delete dead packages and leftovers:**
  - Delete `packages/blog`, `scripts/build-theseus-release-notes.ts`, and `@modrinth/blog` / `changelog:combine-for-app` script entries in root `package.json` (inline `VersionEntry` in `packages/ui/src/components/changelog/ChangelogEntry.vue` or remove unused changelog code, remove `@modrinth/blog` from `packages/ui/package.json`, and record deleted upstream paths in `upstream.json` `ignore`). Keep `packages/toasty` (required by Cargo's `[patch.crates-io]` to avoid SQLite `links` conflict with SQLx).
  - Delete `apps/core/docs/DO-NOT-READ-stale-*.md` and fold `apps/app/src/api/` leftovers into `apps/app/src/desktop/` (recording `ported` entries in `upstream.json`).
  - Delete `.github/workflows/theseus-release.yml`, rename `.github/workflows/theseus-build.yml` -> `amberite-app-build.yml` (stripping `prod-with-staging-archon`, `VITE_STRIPE_PUBLISHABLE_KEY`, and `"Modrinth App"` product/artifact names), remove Modrinth's updater URL/pubkey AND disable the `"updater"` feature in `apps/app/tauri-release.conf.json` (so release builds do not fail at startup before Amberite has its own feed), strip `MODRINTH_ARCHON_BASE_URL` / `SHARED_INSTANCES_API_BASE_URL` from `packages/engine/.env.*` (deleting `.env.prod-with-staging-archon`), and trim Stripe, Intercom, Tally, and Modrinth Hosting hosts from `apps/app/tauri.conf.json` CSP and capabilities.
- **Delete Modrinth Hosting mode (frontend & `api-client`):**
  - Remove `selfHostedHosting` and all Modrinth Hosting fallbacks (`archon.modrinth.com`, `shared-instances.modrinth.com`, Stripe, Intercom, Survey, `HostingUpdateRequired`, dead billing/marketing/onboarding/suspension UI, `isSiteAdmin`, Modrinth-only business `api-client` modules like billing/payouts/analytics/tech-review, `platform/nuxt.ts`, `PanelVersionFeature`). Keep `SelfHostedFeature` and upstream account/social/shared-instances Labrinth modules (per `architecture.md`).
  - Replace `packages/ui/src/composables/server-permissions.ts` body with an owner-only stub (keeping its upstream filename/exports).
  - Keep `nodeAuthState` / `NodeAuthFeature` alive until Checkpoint 5 deletes `core-hosting/*`.
- **Rename `theseus` -> `amberite` (owned code/config only):**
  - Rename `apps/app/Cargo.toml` (`amberite-app`), `THESEUS_CONFIG_DIR` -> `AMBERITE_CONFIG_DIR`, and user agent -> `amberite/<version>` (passed via `EngineConfig` from hosts plus `apps/app-frontend/src/platform/modrinth-client.ts`), updating owned scripts and configs together (`ci.yml`, `scripts/dev-runner.ts`, `scripts/dev-runner.test.ts`, `scripts/check-changed.ts`, `scripts/dev-bench.ts`, `apps/app/turbo.jsonc`, `packages/engine/.cargo/config.toml`). Rename `THESEUS_DB_BACKUP_DIR` -> `AMBERITE_DB_BACKUP_DIR` when `db_backup` policy moves to `apps/app` in Checkpoint 3.
  - Fix colliding names in owned files: `apps/app/src/server/` -> `local_api/`, swap `packages/contracts/src/core/hosting.rs` <-> `servers.rs`, and `local-servers.vue` -> `core-servers.vue` (keep upstream `access-page.vue` and keep `apps/core/src/api/` so it does not collide with `engine::routes`). Land directory renames as an early standalone commit/PR to minimize worktree conflicts.

### 2. Roles-Cut Finish & Contracts / Codegen Machinery (Wire-Compatible)

Keep Copal's wire payloads, paths, and flat `@amberite/contracts` re-exports compatible with `core-hosting/*` in this checkpoint so `main` never breaks and `core-hosting/*` needs zero throwaway edits.

- **Roles-cut (#157) finish:**
  - Remove `View`/`Edit`/`Owner` generics (`Permission`, `Level`, `granted`, `Hook.level`) on engine routes, and remove dead per-topic access rechecks from Copal's `/events` handler while **keeping session-ended revalidation and socket close**. (Keep `CoreEvent::Ended` in `contracts::core::CoreEvent` as unused until Checkpoint 5 deletes `core-hosting/sockets.ts`.)
  - **Keep `Dangerous` fresh-ticket protection as-is** across all dangerous Core actions (deleting a server, ending sessions, updating/rolling back Core, removing Core, restoring/deleting backups).
- **Keep endpoints where isolation is strongest:**
  - Keep App desktop endpoints in `apps/app` (binary crate prevents `apps/core` from importing them) and upstream engine structs (`Instance`, etc.) in `packages/engine`.
  - Move `ModLoader` and `ReleaseChannel` (and their conversion helpers) into `packages/contracts` with `pub use` re-exports in `packages/engine`.
- **Unify Endpoint machinery and codegen:**
  - One exported `contracts::endpoints!` macro everywhere (replacing duplicate macro dialects and hand-written non-test `impl Endpoint`).
  - One `ts-rs` feature gate named `export-ts` (matching upstream engine) generating per-file TS namespaces (`protocol` for shared primitives, plus `backend`, `core`, `engine`, `app`; drop unused `store::ts()` from TS output). Keep flat re-exports in `packages/contracts/ts/index.ts` additively until Checkpoint 5 drops them. Delete hand-maintained TS route templates.
  - One axum route-mount helper in `contracts::server`, and separate `contracts::client` request-building/response-parsing from reqwest transport so Copal CLI (hyper local socket) and App's pinned hyper `SignIn`/`SignOut` tunnel reuse typed `Endpoint` calls.
  - Keep upstream `ErrorKind::code()` intact and enforce via a Rust test that every `ErrorKind` maps to a real `contracts::ErrorCode` (zero silent `Unknown` fallback, zero upstream signature churn); collapse duplicate `ErrorCode` synonyms.
  - Declare Copal `/events` (`Upgrade`) as an `Endpoint`, and drop `Event::AdsConsentRequired`.

### 3. Pure Engine & Copal Deduplication

Write characterization tests for server pack install (excluding intentionally removed edited-file conflict behavior) and file routes (external desktop symlinks, `ManagedPaths`, `sandbox::Folder`, server-owned protection) _before_ refactoring.

- **Keep `Engine.side`, collapse the duplicate server pipeline:**
  - Keep `Engine.side` (one engine holds either client or server instances).
  - Collapse `packages/engine/src/api/pack/server.rs`, `api/pack/server_install.rs`, `api/pack/server/index.rs`, `api/instance/server_content.rs`, `state/instances/commands/embedded_content_metadata/server.rs`, and `state/instances/adapters/sqlite/server_rows.rs` into the main engine pack/content pipeline. Keep a thin `CompleteSideReview` route stub until Checkpoint 5 deletes `core-hosting/content.ts`.
  - Delete `EditedFiles`, `DecideEdited`, engine's `PendingChange` (`packages/engine/src/state/instances/model/file.rs`, where pack updates now overwrite managed server files directly; leave `synced_options` `PendingChange` untouched), and the discarded Archon jar manifest builder.
- **Unify file routes & lazy watcher:**
  - Unify file-route orchestration behind `packages/engine/src/routes/files.rs` while preserving both sides' real invariants: Copal (`Side::Server`) uses handle-walked `sandbox::Folder` (`cap-std`, no symlink escapes) + server-owned file protection; App (`Side::Client`) uses a local open policy that supports external desktop symlinks (while rejecting direct root/leaf symlinks as today) + preserves `ManagedPaths` read-only protection so the Files tab cannot corrupt content-store hardlinks.
  - Make `packages/engine/src/state/instances/watcher.rs` lazy: keep `NonRecursive` root watch (never watches `world/`), stop pre-creating empty folders, watch only allowlisted folders present on disk, and attach lazily on creation.
- **Move desktop leaks out of `packages/engine` into `apps/app`:**
  - Move desktop `Settings` + UI prefs (`Theme`, `DefaultPage`, `FeatureFlag`, `OnboardingChecklist`), `CommandPayload`, `FriendPayload`, metered-network detection (`util/network.rs`, dropping `cidre`/`zbus`/WinRT network deps and `ErrorKind::ZbusError` from `packages/engine`), and App's backup policy/dir (`AMBERITE_DB_BACKUP_DIR`) into `apps/app`. **Keep** launch value types (`MemorySettings`, `Hooks`, `WindowSize`) and `create_sqlite_snapshot` (used by `state/db.rs` and `debug_info.rs`) in `engine`.
  - Delete `Event::Desktop` from `engine` (`apps/app` emits `AppEvent` directly).
  - Move desktop tables (`instance_groups`, `instance_group_memberships`, `join_log`) out of `engine.db` into `app.db`, **keeping `app_metadata` in `engine.db`** (engine's content store uses it on Copal). Detach `group_ids` from engine `Instance` (owned by `apps/app` and composed onto `GameInstance` for the library UI), have `process.rs` emit a join event instead of writing `join_log` (merged into worlds in `apps/app`), put `app.db` + desktop state directly on `AppState` (removing the `InstanceHooks` `Any` downcast), and regenerate `.sqlx` offline metadata.
- **Eliminate Core-vs-engine duplication & enforce mount parity:**
  - Move ticket key verification (`Keys::verify`) out of `packages/sharing-storage` into `packages/sharing-client` so `apps/core` auth and the store share it without Core auth depending on store internals.
  - Copal stores server name, `created_at`, memory, and JVM flags on the engine instance (keeping `edit-instance` unmounted on Copal so remote callers cannot set host paths or launch hooks, and keeping `java_version` on `core.db` `servers` alongside `port`), and reuses engine's RAM check (it already reuses `sandbox::Folder` and `property_key`).
  - Route `packages/engine/src/api/logs.rs` file reads/deletes through `sandbox::Folder` on Copal before mounting engine log routes on Copal (preventing symlink/FIFO escapes), keep `GetLiveLog`/`ClearLiveLog` classified as `ClientOnly` (Copal serves live console backlog from its shim), and replace Copal power/job polling with real events.
  - Classify every engine `Endpoint` as `Shared` vs `ClientOnly` in `contracts::endpoints!`, and add a Rust test asserting `apps/core/src/api/engine_mount.rs` mounts 100% of `Shared` routes, 0% of `ClientOnly` routes, and no `Shared` endpoint input/output type carries a host path (`PathBuf` or host-path fields).

### 4. Backend $0 Cost & Cleanliness (`apps/backend` + Cross-Crate `ReadTicket` Removal)

- Add missing D1 indexes (`shared_instances(home/replica/owner_id)`, `shared_instance_invites(instance_id)` and `(core_id, server_id)`, `sessions(refresh_expires)`), batch invite writes, avoid full-table Core scans on placement, add expiry cleanup, and reduce `FriendsHub` DO fan-out wakeups.
- Move **user profile avatars** (`apps/backend/src/social/index.ts`) from base64 in D1 to R2 (`env.FILES`) to match share icons and trim D1 bloat, with long-lived immutable caching on the revisioned URL, and delete the R2 object on account deletion. Keep generated share icons as D1 JSON `icon_config`.
- **Keep abuse protection (`blacklist`/quarantine) working**, eliminating the extra per-request D1 lookup.
- Drop backend-signed `ReadTicket` JWTs across Rust and TS (`TicketKind::Read`, `READ_TICKET_HEADER`, `IssueReadTicket`, and the `Auth::Account` ticket-header path in `contracts::client`), replacing it by checking `core.owner_id` at read time while keeping `Auth::Ticket` for store tickets (`StoreTicket`/`FetchTicket`/`PullTicket`) intact. (Keep `InstanceFollow.ticket` accepted-and-ignored on Copal until Checkpoint 5 removes it with `core-hosting/content.ts`.)
- Share limits/rules from `contracts`, give Core-key sharing routes (reads and writes, removing `amc_` prefix branching) distinct `Endpoint`s, and split `sharing/index.ts` by domain.

### 5. Zero Translators: Copal Schema Alignment, Clean `api-client`, Socket & Unified UI

Ship Copal's wire changes together with deleting `core-hosting/*` in one atomic checkpoint/PR.

- **Align `contracts::core` directly to Modrinth Hosting's schemas (real subset only, zero fake SFTP/region/billing/suspension/world-id fields):**
  - Backups (**stay in `apps/core` for now**, moved to `engine` later when client backups exist), server power/status (including real "not set up" state, `upstream` modpack, `icon` hash + `icon_config`), `server.properties`, startup/Java, activity log (with server-side filtering), file stats, and server WebSocket events.
  - **Timestamps:** standardize Amberite-owned wire timestamps to RFC 3339 (`DateTime<Utc>`), with an explicit carve-out for `contracts::core` to match upstream Modrinth Hosting's exact field encoding (including numeric timestamps where Kyros uses numbers).
  - Convert Copal backup/file/export downloads to `Stream` `Endpoint`s (dropping `/v1/worlds/{wid}`), emit one shared structured log-line event shape from App and Copal, unify long Core ops with App's non-blocking 202-op pattern, drop `CoreEvent::Ended` (subscribing to a missing/removed server returns `not_found`), drop `InstanceFollow.ticket`, drop `Health.protocol ?? 1`, drop `CompleteSideReview`, and delete `CreateDownloadLink` (`apps/core/src/access/links.rs`).
- **Clean `packages/api-client` & multiplexed socket:**
  - Delete ad hoc `new ContractClient(localTransport)` calls in favor of `client.local`; delete silent first-Core fallback and `hostingUrl` regex scraping (server-to-Core lookup throws `not_found` on unknown server); move `client.cores` off `archon.cores_v1`; keep `SelfHostedFeature` on `client.labrinth` for upstream account/social/shared-instances pages.
  - Add upload progress + abort, timeouts, and `url()` query support to `ContractClient`; unify error classes onto `ApiCallError` typed on `ErrorCode`.
  - Build one TS socket helper (distinctly named from App's `EventSocket` Endpoint, e.g. `EventStream`) with **one multiplexed `/events` socket per Copal**, replacing per-server sockets, server-list/offline/follow polling, and `CoreSyncClient`; fix Core resync and stop the App boot refetch storm.
- **Delete `core-hosting/*` & unify shared UI:**
  - Delete `packages/api-client/src/core-hosting/*`, `nodeAuthState`/`NodeAuthFeature`, `declare module` augmentations, `copal.ts`, `utils/node-url.ts`, hand-added Core fields in `modules/archon/types.ts`, flat re-exports in `packages/contracts/ts/index.ts`, and `apps/app-frontend/src/platform/app-lib/` (recorded in `upstream.json`).
  - Keep `apps/app-frontend/src/helpers/types.d.ts` as a thin wrapper re-exporting generated `@amberite/contracts` types + App's `GameInstance` (`Instance` + `synced_options` + App-owned `group_ids`), deleting hand-copied wire structs without breaking its upstream importers.
  - **Shared UI provider pattern:** keep upstream's shared tab layouts and provider interfaces (`packages/ui/src/layouts/shared/content-tab`, `files-tab`, etc.) and implement **each provider once** over `{ client, instanceId }` (Content, Files, Console/Logs with optional Copal command input, Install Progress, Share History, Icons, Export `.mrpack`) for both `client.local` and `client.core(coreId)`.
  - Copal-only screens (Overview/Power, Backups, Properties, Activity) call `client.core(coreId).call(...)` directly on generated `contracts::core` types (no `worldId` plumbing), recording replaced/deleted upstream hosting pages in `upstream.json` (`ported`/`ignore`).
- **In-scope behavior improvements (verify in App pass):** upload progress + abort, real "not set up" server state, server-side activity filtering, and real-time events replacing polling.

---

## Checkpoints (Land in Order, Never Break `main`)

1. **Dead weight + Modrinth Hosting mode deletion + owned `theseus` renames** (Section 1; land directory renames as their own quick commit/PR).
2. **Roles-cut finish + contracts & codegen machinery** (Section 2; Copal wire and flat TS re-exports stay compatible).
3. **Engine & Copal deduplication** (Section 3; characterization tests written first; regenerate `.sqlx`).
4. **Backend $0 cost & cleanliness + `ReadTicket` removal** (Section 4).
5. **Copal schema alignment + clean `api-client` + multiplexed socket + delete `core-hosting/*` + unified App/Copal UI** (Section 5; Rust + TS ship together).

---

## Definition of Done

1. **Zero-count & structural checks:**
   - `packages/api-client/src/core-hosting/`, `packages/blog/`, `apps/app-frontend/src/platform/app-lib/`: **0 files**
   - `selfHostedHosting`, `archon.modrinth.com`, `shared-instances.modrinth.com`, `nodeAuthState`, `CoreSyncClient`, Stripe/Intercom/Tally hosts: **0 hits**
   - Modrinth updater feed URL and `"updater"` feature in `apps/app/tauri-release.conf.json` / workflows: **0 hits**
   - `Partial<` adapters over upstream API modules, `declare module` augmentations on `archon`/`kyros`: **0 hits**
   - `packages/engine/src/api/pack/server*`, `packages/engine/src/state/instances/adapters/sqlite/server_rows.rs`, `packages/engine/src/api/instance/server_content.rs`, `EditedFiles`, `DecideEdited`, and engine `state/instances/model/file.rs` `PendingChange`: **0 files / 0 hits**
   - `theseus` in owned paths (`apps/app/`, `apps/core/`, `apps/backend/`, `packages/contracts/`, `scripts/`, `.github/workflows/`, `apps/app-frontend/src/platform/modrinth-client.ts`): **0 hits**
   - Rust invariants: **0** non-test `impl Endpoint for` outside `contracts::endpoints!`; **0** non-test raw `.route(` on owned routers outside the mount helper and documented reverse proxies (`/store/{*rest}`, Core proxy); **0** `InstanceAccess<` or word-boundary `Permission` in `engine`; **0** `Event::Desktop`; **0** desktop tables (`instance_groups`, `instance_group_memberships`, `join_log`) in `packages/engine` migrations; **0** `TicketKind::Read`.
2. **Automated tests & boundary checks:**
   - `vp run check:changed` passes locally and CI is green (`cargo shear`, `SQLX_OFFLINE`) at every checkpoint.
   - Characterization tests (written _before_ refactoring) pass for server pack install and file routes (external desktop symlinks, `ManagedPaths`, `sandbox::Folder`, server-owned protection), plus `api/logs.rs` sandbox safety on Copal.
   - Rust tests prove: `apps/core` mounts 100% of `Shared` engine endpoints, 0% of `ClientOnly` endpoints, and no `Shared` endpoint input/output type carries a host path; every engine `ErrorKind` maps to a non-`Unknown` `ErrorCode`; `dangerous_actions_need_a_fresh_ticket_of_the_owner` (`apps/core/src/api/tests.rs`) covers the full `Dangerous` route set.
   - Dependency boundary check in `ci.yml` proves `packages/sharing-storage` depends on neither `packages/engine` nor `apps/core` (alongside existing `copal`/`engine` checks).
   - Backend tests pass for R2 avatars (including immutable caching and deletion on account delete), batched invites, `core.owner_id` read auth, and blacklist check; D1 schema includes the new indexes.
3. **Real App + Copal integrated pass:**
   - Exercise local App instances and Copal servers through fixtures: Content (list, toggle, embedded icons, install), Files (list, upload with progress/abort, desktop symlink, read-only `mods/`), Console/Logs (live stream, old logs, server command), Backups, Properties, Activity filtering, "not set up" state, and Share publish + install round trip.
4. **Fresh Council review:**
   - Up to 2 rounds of fresh reviewers against **this spec + the repo** until zero findings violate this spec's principles (any remaining edge calls go to ilai).
