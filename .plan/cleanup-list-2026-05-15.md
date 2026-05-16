# Amberite Cleanup Checklist - 2026-05-15

This checklist turns the cleanup plan into file-level work. Inspect each diff
before applying a full revert. Prefer upstream parity unless Amberite has a clear
reason to differ.

## Delete Or Remove From Git

- `apps/core/patches/swc_common-0.37.5/`: delete entirely. Then find what restored
	it last time and remove that generator, copy step, or patch reference.
- `libraries/**`: remove committed Minecraft runtime/cache jars.
- `versions/1.20.4/server-1.20.4.jar`: remove committed server jar.
- Add ignore rules for downloaded Minecraft runtime artifacts after removal.

## Move Existing Plans

- `.plan/active/api-lib.md` -> `.plan/api-lib-2026-05-12.md`
- `.plan/active/core-launch.md` -> `.plan/core-launch-2026-05-10.md`
- `.plan/active/hosting-rewrite.md` -> `.plan/hosting-rewrite-2026-05-11.md`
- Remove `.plan/active/` after it is empty.

## Revert Whole Files First

- `apps/app-frontend/src/components/ui/friends/FriendsList.vue`: restore upstream
	real friends behavior; remove mock friends.
- `packages/ui/src/components/base/Button.vue`: restore `download` prop and anchor
	binding.
- `packages/ui/src/components/base/OverflowMenu.vue`: restore menu item download
	support.
- `apps/frontend/src/pages/[type]/[id]/changelog.vue`: restore download attribute.
- `apps/frontend/src/pages/[type]/[id]/settings/versions.vue`: restore download
	menu option.
- `apps/frontend/src/pages/[type]/[id]/version/[version].vue`: restore download
	attributes.
- `apps/frontend/src/pages/[type]/[id]/versions.vue`: restore download attributes
	and options.

## Revert Storybook Churn

- `packages/ui/.storybook/main.ts`: revert autodocs/docgen monkey-patch unless
	reintroduced in a dedicated Storybook cleanup.
- `packages/ui/.storybook/preview.ts`: revert global autodocs tag.
- `packages/ui/src/stories/Overview.stories.ts`: remove broad all-components
	catalog from this cleanup path.
- `packages/ui/src/stories/base/TabGroup.stories.ts`: keep only if `TabGroup` is
	kept as a standalone generic component.
- Root and package `package.json` Storybook script rename churn: revert first;
	reintroduce `story` naming later as a focused change if still desired.

## Restore Shared UI, Fork Core UI

- Restore shared server settings pages close to upstream:
	- `packages/ui/src/layouts/shared/server-settings/pages/advanced.vue`
	- `packages/ui/src/layouts/shared/server-settings/pages/general.vue`
	- `packages/ui/src/layouts/shared/server-settings/pages/network.vue`
	- `packages/ui/src/layouts/shared/server-settings/pages/properties.vue`
- Restore or replace `packages/ui/src/composables/server-manage-core-runtime.ts`
	with a separate Core runtime instead of overwriting upstream behavior.
- Move Core-specific server files/backups/settings/overview behavior into a local
	`core` folder inside `packages/ui`.
- Remove `injectCoreClient()` requirements from shared backup modals, `SaveBanner`,
	and shared settings pages. Use Core-specific wrappers or injected handlers.

## Fix Core Server Pages

- `apps/app-frontend/src/pages/server/Index.vue`: make `profilePath`, profile,
	and `coreInstanceId` reactive to route param changes, or key the parent route by
	ID.
- `apps/app-frontend/src/pages/synced/Index.vue`: make `coreInstanceId` reactive
	and move `provideCoreClient()` before any top-level `await`.
- `apps/app-frontend/src/pages/synced/ServerSetup.vue`: reconnect the socket when
	`coreInstanceId` changes.
- `apps/app-frontend/src/pages/synced/Index.vue`: key nested route content by
	`route.path` or `route.fullPath`, not only `instance.path`.
- `apps/app-frontend/src/routes.js`: align synced breadcrumb key with the page.

## Fix Creation And Profile Contracts

- `apps/app/src/api/profile_create.rs`: stop calling `/api/v1/instances`; Core uses
	`/instances`.
- Pick one layer to create Core instances. Prefer frontend/api-lib or a dedicated
	Amberite flow, not both Rust profile creation and frontend creation.
- `apps/app/src/api/profile.rs`: add `kind` and `core_instance_id` to `EditProfile`
	if frontend is expected to persist them.
- Add rollback for server/synced creation failures.
- Avoid creating local server/synced profiles without a valid Core ID.

## Fix App Shell And Dev Tooling

- `apps/app/tauri.macos.conf.json`: restore macOS native decoration/traffic-light
	behavior.
- `apps/app/src/main.rs`: make CDP port `9222` opt-in instead of always enabled in
	debug builds.
- `apps/app-frontend/src/components/ui/WindowControls.vue`: remove negative
	z-index from expanded hit area.
- Keep design inspector and Vue DevTools opt-in only; add internal no-op guards.

## Fix Core/API Security And Contracts

- `apps/core/src/infrastructure/auth/jwks.rs`: validate Supabase/temporary auth
	correctly until Convex auth is designed.
- Core auth: check token owner against stored `owner_user_id`.
- Core CORS: stop using permissive CORS for normal operation.
- Core filesystem and backups: fix ZIP slip and symlink escape risks.
- Core properties: align exact Minecraft keys with UI keys.
- API lib: add request timeouts and cancellation.
- API lib: redesign relay around Convex identity and message types instead of the
	current Supabase UUID/string mismatch.

## Convex/API Lib Architecture Work

- Add `.convex/` as the centralized backend boundary when migration begins.
- Define API lib communication modes explicitly: direct, Core relay, Convex relay.
- Require each API method to declare its communication mode.
- Keep auth decisions open until Convex auth design is settled.
