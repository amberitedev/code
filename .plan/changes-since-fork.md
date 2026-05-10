# Amberite Changes Since Modrinth Fork

All changes listed below are **uncommitted working-tree modifications** against HEAD (`c780ff05b`), which is the upstream Modrinth commit. Nothing here is in a git commit yet.

---

## apps/app-frontend/ Changes

### apps/app-frontend/package.json
**Status**: Modified  
**Summary**: Adds the `vite-plugin-vue-devtools` dev dependency to enable the Vue DevTools overlay during development.  
**Key changes**:
- Added `"vite-plugin-vue-devtools": "^8.1.2"` to `devDependencies`

---

### apps/app-frontend/vite.config.ts
**Status**: Modified  
**Summary**: Registers the `VueDevTools` Vite plugin so the in-browser Vue DevTools panel is available in dev mode.  
**Key changes**:
- Imports `VueDevTools` from `vite-plugin-vue-devtools`
- Adds `VueDevTools()` as the first entry in the `plugins` array

---

### apps/app-frontend/src/App.vue
**Status**: Modified  
**Summary**: Strips all Modrinth-specific sidebar content (news feed, Modrinth+ promotion, `PromotionWrapper`) and simplifies the friends list integration by exposing a `ref` for it.  
**Key changes**:
- Removed news feed fetch, `news` ref, `NewsArticleCard`, and "View all news" / "Upgrade to Modrinth+" sidebar sections
- Removed `PromotionWrapper` import and `showAd` / `hasPlus` conditional rendering from the sidebar
- `FriendsList` moved into a plain `p-4` div; now wrapped with `ref="friendsListRef"` (new ref added)
- Removed `has-plus` and `pb-12` conditional CSS classes from sidebar containers

---

### apps/app-frontend/src/components/ui/friends/FriendsList.vue
**Status**: Modified  
**Summary**: Replaces the real friends API calls with static mock data for design preview and refactors the "Add friend" button to a prominent bottom CTA.  
**Key changes**:
- `loadFriends` now immediately sets `loading = false` and populates `userFriends` with a `MOCK_FRIENDS` array of three hardcoded users (alice_plays, CraftingBob, RedstoneCharlie)
- Removed the circular `UserPlusIcon` button from the friends header row
- Added a new full-width `ButtonStyled color="brand" size="large"` "Add friend" button at the bottom
- Added `defineExpose({ showAddFriendModal })` so the parent (`App.vue`) can programmatically open the modal

---

### apps/app-frontend/src/helpers/profile.ts
**Status**: Modified  
**Summary**: Extends the `create` profile function to accept optional `kind` (ProfileKind) and `port` parameters so Amberite can distinguish server/synced profiles at creation time.  
**Key changes**:
- Imports `ProfileKind` from `./types`
- `create()` now accepts: `kind?: ProfileKind | null` and `port?: number | null`
- Both values are forwarded into the Tauri invoke payload

---

### apps/app-frontend/src/helpers/types.d.ts
**Status**: Modified  
**Summary**: Adds Amberite-specific fields to the `GameInstance` type and defines the new `ProfileKind` union type.  
**Key changes**:
- `GameInstance` gains `kind: ProfileKind` and `core_instance_id?: string` (marked `// AMBERITE PATCH`)
- New exported type `ProfileKind = 'client' | 'server' | 'synced'`

---

### apps/app-frontend/src/helpers/core.ts
**Status**: New file  
**Summary**: Typed TypeScript wrappers for all Amberite Core Tauri plugin commands under the `plugin:amberite|*` namespace.  
**Contents**: Exports `CoreInstanceDetail` interface and six async functions: `core_get_instance`, `core_start`, `core_stop`, `core_restart`, `core_send_command`, `core_issue_ws_token`, `core_get_url`

---

### apps/app-frontend/src/pages/Servers.vue
**Status**: Modified  
**Summary**: Removes the Modrinth billing/Stripe product fetching logic, rendering `ServersManagePageIndex` with no props.  
**Key changes**:
- Deleted `useQuery`, `injectModrinthClient`, billing query, `stripePublishableKey`, and `resolvedProducts`
- `<ServersManagePageIndex />` rendered without `:stripe-publishable-key` or `:products` props

---

### apps/app-frontend/src/pages/instance/Index.vue
**Status**: Modified  
**Summary**: Derives `isServerInstance` from `ProfileKind`, redirects server/synced profiles to `/server/:id`, and replaces the play button tree with a `JoinedButtons` component.  
**Key changes**:
- `isServerInstance` converted from `ref(false)` to `computed(() => instance.value?.kind === 'server' || ...)` (marked `// AMBERITE PATCH`)
- `fetchInstance` redirects to `/server/:id` if `kind` is `'server'` or `'synced'`
- Single `<JoinedButtons>` replaces the multi-branch play/stop/starting button tree
- `playActions` computed builds different action sets depending on `playing`, `stopping`, `serverRunning`
- Added "Push to server" overflow menu item (TODO stub)

---

### apps/app-frontend/src/providers/setup/creation-modal.ts
**Status**: Modified  
**Summary**: Passes the selected `instanceKind` from the creation modal config into the `create()` profile call.  
**Key changes**:
- Imports `ProfileKind` from `@/helpers/types`
- In the vanilla/loader creation branch, passes `config.instanceKind.value as ProfileKind` as the `kind` argument to `create()`

---

### apps/app-frontend/src/routes.js
**Status**: Modified  
**Summary**: Registers the new `/server/:id` route tree with Overview, Console, and Content child routes.  
**Key changes**:
- Imports `* as Server from '@/pages/server'`
- Adds `/server/:id` parent route with three children: `''` → Overview, `'console'` → Console (renderMode: 'fixed'), `'content'` → Content

---

### apps/app-frontend/src/pages/server/ (directory)
**Status**: New directory  
**Summary**: Dedicated management UI for Amberite server/synced profiles.  
**Files**:
- `index.js`: Barrel re-exporting `Index`, `Overview`, `Console`, `Content`
- `Index.vue`: Shell layout — fetches `GameInstance` + `CoreInstanceDetail`, polls Core status every 5s, shows Start/Stop/Restart buttons, renders `NavTabs` (Overview / Console / Content)
- `Overview.vue`: Two-card grid — "Server Details" (status, game version, loader) and "Instance Info" (profile name, Core ID); status colour-coded green/yellow/red
- `Console.vue`: Live WebSocket console — connects to `ws://<core_url>/instances/:id/console?ticket=<token>`, streams log lines, provides command input
- `Content.vue`: Placeholder stub — "Mod management for server instances is coming soon."

---

## apps/app/ Changes (Tauri Rust backend)

### apps/app/Cargo.toml
**Status**: Modified  
**Summary**: Adds `amberite-lib` and `reqwest` (JSON feature) as dependencies.  
**Key changes**:
- `amberite-lib = { workspace = true }`
- `reqwest = { workspace = true, features = ["json"] }`

---

### apps/app/build.rs
**Status**: Modified  
**Summary**: Registers the new `"amberite"` Tauri plugin with all 8 commands allowed by default.  
**Key changes**:
- `InlinedPlugin` block for `"amberite"` plugin declaring: `ping`, `core_get_instance`, `core_start`, `core_stop`, `core_restart`, `core_send_command`, `core_issue_ws_token`, `core_get_url`
- `DefaultPermissionRule::AllowAllCommands`

---

### apps/app/capabilities/plugins.json
**Status**: Modified  
**Summary**: Adds `"amberite:default"` to the app's capability list.  
**Key changes**:
- `"amberite:default"` appended to the permissions array

---

### apps/app/src/api/mod.rs
**Status**: Modified  
**Summary**: Declares the `amberite` module and extends the error enum to propagate `AmberiteError`.  
**Key changes**:
- `pub mod amberite;`
- `TheseusSerializableError::Amberite(#[from] amberite_lib::error::AmberiteError)` variant added

---

### apps/app/src/api/profile_create.rs
**Status**: Modified  
**Summary**: Extends `profile_create` to accept `kind` and `port`; auto-provisions a Core server instance for server/synced profiles.  
**Key changes**:
- Imports `amberite_lib::core_instances::{CreateInstanceRequest, create_instance}`
- `profile_create` gains `kind: Option<String>` and `port: Option<u16>`
- For server/synced: reads `core_url` from `AppSettings`, calls `create_instance` via reqwest, passes returned instance ID as `core_instance_id`

---

### apps/app/src/main.rs
**Status**: Modified  
**Summary**: Registers the `amberite` plugin with the Tauri app builder.  
**Key changes**:
- `.plugin(api::amberite::init())` added

---

### apps/app/src/api/amberite/ (directory)
**Status**: New directory  
**Summary**: Tauri plugin (`plugin:amberite|*`) bridging the frontend to Core over HTTP.  
**Files**:
- `mod.rs`: Defines `init()`, `resolve_core_url()` (reads from `AppSettings`, fallback `http://localhost:16662`), and all command handlers: `ping`, `core_get_instance`, `core_start`, `core_stop`, `core_restart`, `core_send_command`, `core_issue_ws_token`, `core_get_url`

---

## packages/app-lib/ Changes

### packages/app-lib/src/api/mod.rs
**Status**: Modified  
**Summary**: Re-exports `ProfileKind` as part of the public `data` API surface.  

---

### packages/app-lib/src/api/profile/create.rs
**Status**: Modified  
**Summary**: Extends `profile_create` with optional `kind: Option<ProfileKind>` and `core_instance_id: Option<String>`.  
**Key changes**:
- Defaults to `ProfileKind::Client` if `kind` is `None`
- `profile_create_from_duplicate` passes `None, None` (no behaviour change for duplicated profiles)

---

### packages/app-lib/src/state/legacy_converter.rs
**Status**: Modified  
**Summary**: Fills in `kind: ProfileKind::Client` and `core_instance_id: None` when converting legacy profiles.  

---

### packages/app-lib/src/state/profiles.rs
**Status**: Modified  
**Summary**: Defines `ProfileKind` enum and wires it end-to-end through `Profile`, `ProfileQueryResult`, and all SQL queries.  
**Key changes**:
- New `ProfileKind` enum (marked `// AMBERITE PATCH`): `Client`, `Server`, `Synced`
- `as_str()` and `from_str()` helpers; `from_str` falls back to `Client` for unknown values
- `Profile` struct gains `kind: ProfileKind` and `core_instance_id: Option<String>`
- SELECT macro and upsert query updated to include both columns (`$29`, `$30`)

---

### packages/app-lib/migrations/20260509000000_add-profile-kind.sql
**Status**: New file  
**Summary**: SQLite migration adding `kind` and `core_instance_id` columns to the `profiles` table.  
**Contents**:
- `ALTER TABLE profiles ADD COLUMN kind TEXT NOT NULL DEFAULT 'client';`
- `ALTER TABLE profiles ADD COLUMN core_instance_id TEXT;`

---

### packages/app-lib/.sqlx/ (SQLx query cache)
**Status**: New files (3 query JSON files)  
**Summary**: Auto-generated SQLx offline query cache reflecting the updated SELECT/INSERT/UPDATE queries that now include `kind` and `core_instance_id`.

---

## packages/ui/ Changes

### packages/ui/src/components/flows/creation-flow-modal/components/CustomSetupStage.vue
**Status**: Modified  
**Summary**: Adds a "Client / Server / Synced" chip selector to the custom setup stage, shown only when `flowType === 'instance'`.  
**Key changes**:
- Imports `InstanceKind` from `creation-flow-context`
- Binds `instanceKind` ref to a new `<Chips>` component with `instanceKindItems` array and `formatInstanceKindLabel` formatter
- Four new i18n messages: `instanceTypeLabel`, `clientKind`, `serverKind`, `syncedKind`

---

### packages/ui/src/components/flows/creation-flow-modal/creation-flow-context.ts
**Status**: Modified  
**Summary**: Introduces `InstanceKind` type and `instanceKind: Ref<InstanceKind>` to the creation flow context, defaulting to `'client'`.  
**Key changes**:
- Exports `InstanceKind = 'client' | 'server' | 'synced'`
- Initialises `instanceKind` as `ref<InstanceKind>('client')` inside `createCreationFlowContext`
- Resets to `'client'` in the flow reset block
- Exposes `instanceKind` in the returned context object

---

### packages/ui/src/layouts/shared/content-tab/layout.vue
**Status**: Modified  
**Summary**: Adds a client/server side-filter toggle to the content-tab toolbar (marked `// AMBERITE:`).  
**Key changes**:
- `sideView` ref (`'client' | 'server' | null`, default `null`) drives filtering in `filteredItems`
- `'client'` hides `server_only` items; `'server'` hides client-only items; `null` shows all
- Two pill toggle buttons rendered via `v-for` with `aria-pressed` and active/inactive styling

---

## apps/frontend/ Changes

### apps/frontend/src/middleware/dev-mock.global.ts
**Status**: New file  
**Summary**: Nuxt global route middleware that seeds a fake authenticated Modrinth user in dev mode so hosting UI pages are browsable without a live backend.  
**Purpose**: Runs before all named middleware (including real `auth` guard). No-ops in production (`import.meta.dev` guard). Writes mock token + `Labrinth.Users.v2.User` into `useState('auth')` on first navigation.

---

### apps/frontend/src/plugins/hosting-mock.client.ts
**Status**: New file  
**Summary**: Nuxt client plugin that monkey-patches `globalThis.fetch` to intercept all Archon API requests and return synthetic responses, making the hosting UI functional in dev without a backend.  
**Purpose**: Intentional dev tooling (marked `// AMBERITE PATCH`). Production guard ensures it's a no-op in production. Covers ~30 Archon v0 and v1 endpoints via regex mini-router. Non-Archon requests fall through to real `fetch`.

---

## Root-Level Changes

### AGENTS.md
**Status**: Modified  
**Summary**: Replaced the upstream Modrinth symlink with a full Amberite-specific 104-line agent instructions document.  
**Key changes**: Documents all apps, WSL vs Windows dev split, 200-line file limit, `// AMBERITE PATCH` markers, session memory instructions

---

### Cargo.toml
**Status**: Modified  
**Summary**: Registers `packages/amberite-lib` in the Rust workspace.  
**Key changes**:
- Added `"packages/amberite-lib"` to `[workspace] members`
- Added `amberite-lib = { path = "packages/amberite-lib" }` to `[workspace.dependencies]`

---

### Cargo.lock
**Status**: Modified  
**Summary**: Regenerated — reflects new Rust crate dependencies from `packages/amberite-lib`

---

### opencode.json
**Status**: Modified  
**Summary**: Adds the Supabase remote MCP server for Amberite's Supabase project (`zbngxzouadkdbsxtumyf`).  

---

### package.json
**Status**: Modified  
**Summary**: Adds `"core:dev": "turbo run dev --filter=@modrinth/core"` convenience script.

---

### pnpm-lock.yaml
**Status**: Modified  
**Summary**: Regenerated — reflects new npm dependencies

---

### turbo.jsonc
**Status**: Modified  
**Summary**: Removes Rust `target/` from Turbo cache; adds Core/Supabase env vars to `dev` task allowlist.  
**Key changes**:
- `build.outputs`: removed `$TURBO_ROOT$/target/**`
- `dev.env`: added `PORT`, `AMBERITE_*`, `SUPABASE_*`, `ALLOWED_ORIGIN`
- `dev.passThroughEnv`: added `CARGO_INCREMENTAL`

---

## New Top-Level Additions

### PROJECT.md
**Status**: New file  
**Summary**: 356-line single source of truth for the whole Amberite project — architecture, decisions, known issues, build priority. Intended to be fed to AI agents at session start.

---

### .plan/active/core-test-plan.md
**Status**: New file  
**Summary**: 875-line comprehensive test plan for `apps/core/` — known bugs, test infrastructure, per-file integration test spec, CLI plan, Docker plan.

---

### .plan/active/hosting-ui.md
**Status**: New file  
**Summary**: 156-line AI reference for the Modrinth hosting UI — route map, layout hierarchy, TanStack Query keys, component API, mock data setup.

---

### apps/core/
**Status**: New directory (Amberite Core — fully custom, zero Modrinth code)  
**Summary**: Rust/Axum HTTP server on port 16662 that manages Minecraft server instances.  
**Architecture**: Clean architecture — `domain → ports → application → infrastructure → presentation`. SQLite storage, PTY console, RS256 JWT auth via JWKS, Deno macro engine, 112 integration tests.  
**Key tech**: Axum 0.7, SQLx 0.9, Tokio, jsonwebtoken, deno_core/deno_runtime, portable-pty, reqwest, sysinfo, async_zip, rcon.

---

### packages/amberite-lib/
**Status**: New directory (planned Tauri backend library)  
**Summary**: Scaffold Rust crate that will become the Amberite-specific backend for the Tauri app. Currently a stub; real logic still in `apps/app/backend/`. 16 modules covering Core HTTP client, WebSocket streaming, OS keychain auth, Core binary launcher, mod sync, friends/groups, Supabase client, Playit.gg tunnel management.
