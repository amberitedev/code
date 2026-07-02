# Architecture

The shared UI package used by both `apps/frontend` (Nuxt 3) and `apps/app-frontend` (Vue 3 + Tauri). Components here must be platform-agnostic. Use dependency injection for platform-specific behavior.

## Folder Structure

```text
src/
|-- components/       # Vue components organized by feature domain
|-- composables/      # Vue 3 composition API hooks
|-- layouts/          # Self-contained page layouts
|-- providers/        # Dependency injection contexts
|-- utils/            # Utility functions and constants
|-- pages/            # Cross-platform page components
|-- locales/          # Locale files
|-- styles/           # Tailwind CSS utilities
`-- stories/          # Storybook story files
```

Each subdirectory under `components/` has an `index.ts` barrel file. All public API is re-exported from the root `index.ts`.

### `src/layouts/`

Self-contained page layouts shared across frontends. Split into two categories:

- **`shared`**: Reusable layout modules with their own components, composables, providers, and types. Each module is a self-contained unit, such as `shared/content-tab/`.
- **`wrapped`**: Page-level Vue components that mirror route structures, such as `wrapped/hosting/manage/`.

Files inside `layouts/` use the `#ui/*` import alias, resolved through the package `imports` field, to reference other `src/` modules.

# Code Guidelines

## Motion Rules

- HARD RULE: Do not modify the `NavTabs` selected-slider animation unless the user explicitly asks for that animation to change. It is the upstream Modrinth staggered `left`/`right`/`top`/`bottom` slide transition, not a FLIP, transform, scale, or Web Animations animation. If adjacent code must change, preserve the exact timing, easing, delay, and edge-delay behavior from `src/composables/ui-motion.ts`.

### Tailwind Configuration

All frontend packages share a Tailwind preset at `packages/tooling-config/tailwind/tailwind-preset.ts`. This package's `tailwind.config.ts` extends it:

```ts
import preset from '@modrinth/tooling-config/tailwind/tailwind-preset.ts'
```

CSS custom properties are defined in `packages/assets/styles/variables.scss` with light, dark, and OLED theme variants.

### Color Usage Rules

The current product palette is charcoal/black surfaces plus one strong orange brand color. The UI should feel high-contrast and neutral by default, with orange used deliberately for primary actions, selected states, focus, and small accents. Avoid brown-looking intermediate colors.

**Use `surface-*` variables for backgrounds. Do not use aliased `bg-*` color variables for general surfaces.**

| Token            | Usage                                 |
| ---------------- | ------------------------------------- |
| `bg-surface-1`   | Deepest app background                |
| `bg-surface-1.5` | Odd row background                    |
| `bg-surface-2`   | Even row background, secondary panels |
| `bg-surface-3`   | Headers, floating bars, inputs        |
| `bg-surface-4`   | Cards and elevated surfaces           |
| `bg-surface-5`   | Borders and dividers                  |

**For text colors:**

| Class            | Usage                               |
| ---------------- | ----------------------------------- |
| `text-contrast`  | Primary headings                    |
| `text-primary`   | Default body text                   |
| `text-secondary` | Reduced emphasis and secondary info |

**Brand orange rules:**

- Use `--color-brand`, `--color-orange`, `color="brand"`, or `color="primary"` for the main orange button treatment and true brand accents.
- Do not invent extra orange hex values, ad-hoc Tailwind colors, or arbitrary transparent orange overlays.
- If orange needs to appear over dark UI without turning brown, use precomputed tokens from `variables.scss`, such as `--color-orange-accent-button-bg-hover`, `--color-left-nav-button-bg-hover`, and `--color-sidebar-button-bg-hover`.
- Clear and secondary buttons should stay neutral by default. Add orange hover/accent behavior only where the component or page intentionally opts into it.
- Warning states use `--color-warning` and `--color-warning-contrast`, not brand orange.
- Danger states use the danger/red tokens. Do not make destructive UI look like a brand action.

**Other color tokens are semantic or content-specific.** They may exist for status, links, charts, metadata, loaders, or platform badges, but they are not the product palette. Do not choose them for generic buttons, dashboard panels, or page accents unless the UI meaning requires that semantic color.

## Storybook

When modifying a component in `src/components/`, update its corresponding Storybook story in `src/stories/` if the change materially affects component states, variants, or behavior. Do not make or modify Storybook for tiny styling tweaks unless the story would clearly prevent a regression.

## Dependency Injection

This package defines the DI layer using `createContext` from `src/providers/index.ts`.

Key providers exported from this package:

- `provideModrinthClient` / `injectModrinthClient`: API client
- `provideNotificationManager` / `injectNotificationManager`: Notifications

## Error Display Guidance

Prefer notification errors for regular page action/load failures; use `injectNotificationManager()` and `addNotification({ type: 'error', title, text })`.
The notification implementation is in `packages/ui/src/providers/web-notifications.ts`; reserve inline errors for modals, setup/full-page states, and forms where the error belongs next to the field.

## Vue Template Rules

### Multi-statement event handlers

Never use newline-separated statements in Vue template event handlers like `@click`. Vue's template compiler cannot parse multi-line expressions separated only by newlines. Always use semicolons on a single line:

```vue
<!-- BAD: will cause "Unexpected token" parse error -->
@click=" foo = true $emit('bar') "

<!-- GOOD -->
@click="foo = true; $emit('bar')"
```

## Component List

Use this inventory as the first pass before building new UI. Prefer the public entrypoints below over reaching into internal helpers.

If you add a new shared component or materially change the role or description of an existing shared component, update this list in the same change.

| Component                        | Description                                                                                                                                                                             | Filepath                                                        |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| `affiliate`                      | Affiliate link management UI, including listing cards and creation modal flows.                                                                                                         | `packages/ui/src/components/affiliate/index.ts`                 |
| `base`                           | Core shared primitives such as buttons, cards, inputs, selects, tables, tabs, modals, empty states, progress, readiness, lazy mounting, ghost loading primitives, and page scaffolding. | `packages/ui/src/components/base/index.ts`                      |
| `GhostBlock`                     | Shared inert ghost block with shaped presets and scan-line shimmer for loading placeholders.                                                                                            | `packages/ui/src/components/base/GhostBlock.vue`                |
| `GhostText`                      | Shared ghost text lines for title, body, and metadata placeholders.                                                                                                                     | `packages/ui/src/components/base/GhostText.vue`                 |
| `GhostMedia`                     | Shared ghost media placeholder for square, rounded, circle, and banner shapes.                                                                                                          | `packages/ui/src/components/base/GhostMedia.vue`                |
| `GhostControl`                   | Shared ghost control placeholder for input, select, button, icon button, chip, and pagination shapes.                                                                                   | `packages/ui/src/components/base/GhostControl.vue`              |
| `GhostTabGroup`                  | Inert ghost tab group that composes real `NavTabs` instead of duplicating tab visuals.                                                                                                  | `packages/ui/src/components/base/GhostTabGroup.vue`             |
| `NavTabContentTransition`        | Opt-in page slide transition for tabbed routed content, with 120ms default timing, frozen outgoing frames, slow-motion debugging, and transition safety cleanup.                        | `packages/ui/src/components/base/NavTabContentTransition.vue`   |
| `ReadyTransition`                | Shared readiness boundary with delayed ghost, timeout, error, content-key, keep-previous, silent, and global loading-token support.                                                     | `packages/ui/src/components/base/ReadyTransition.vue`           |
| `UiLazyMount`                    | SSR-safe lazy mount wrapper with visible, idle, delay, and immediate modes plus fallback slots.                                                                                         | `packages/ui/src/components/base/UiLazyMount.vue`               |
| `UiMotionTransition`             | Shared configurable motion wrapper for keyed content, route, and tab transitions with slide, fade, scale-fade, height, none, and frozen-leave support.                                  | `packages/ui/src/components/base/UiMotionTransition.vue`        |
| `TextMorph`                      | Shared per-character text morph animation with Motion shared layout IDs, pop-layout exit handling, and variants/transition overrides.                                                   | `packages/ui/src/components/base/TextMorph.vue`                 |
| `SwipeDismissSurface`            | Shared horizontal swipe-to-dismiss surface with mirrored danger reveal, one-third wall clamp, axis lock, and no intermediate open state.                                               | `packages/ui/src/components/base/SwipeDismissSurface.vue`       |
| `StageContentTransition`         | Shared directional stage/page transition with horizontal or vertical push animation and smooth height release for modal and tab content.                                                | `packages/ui/src/components/base/StageContentTransition.vue`    |
| `billing`                        | Purchase, payment method, subscription, and hosted server checkout UI.                                                                                                                  | `packages/ui/src/components/billing/index.ts`                   |
| `brand`                          | Shared Modrinth branding components such as animated and text logos.                                                                                                                    | `packages/ui/src/components/brand/index.ts`                     |
| `changelog`                      | Changelog entry presentation for release and update content.                                                                                                                            | `packages/ui/src/components/changelog/index.ts`                 |
| `chart`                          | Shared chart visualizations, including compact chart variants.                                                                                                                          | `packages/ui/src/components/chart/index.ts`                     |
| `content`                        | Shared content list and article card UI for browse-style surfaces.                                                                                                                      | `packages/ui/src/components/content/index.ts`                   |
| `external_files`                 | UI for looking up and describing external project file licensing and permissions.                                                                                                       | `packages/ui/src/components/external_files/index.ts`            |
| `modal`                          | Shared modal shells and common confirmation, share, and install modal variants.                                                                                                         | `packages/ui/src/components/modal/index.ts`                     |
| `nav`                            | Shared navigation UI such as breadcrumbs, banners, and notification panels.                                                                                                             | `packages/ui/src/components/nav/index.ts`                       |
| `notifications`                  | Toast and notification stack rendering.                                                                                                                                                 | `packages/ui/src/components/notifications/index.ts`             |
| `page`                           | Reusable page-level wrappers such as normal pages and sidebar cards.                                                                                                                    | `packages/ui/src/components/page/index.ts`                      |
| `project`                        | Project cards, headers, sidebars, version displays, server info, and project settings UI.                                                                                               | `packages/ui/src/components/project/index.ts`                   |
| `search`                         | Shared search filters, categories, and sidebar filtering controls.                                                                                                                      | `packages/ui/src/components/search/index.ts`                    |
| `servers`                        | Hosted server UI, including listings, access management, backups, headers, icons, setup flows, labels, and promos.                                                                      | `packages/ui/src/components/servers/index.ts`                   |
| `settings`                       | Shared settings selectors such as language and theme pickers.                                                                                                                           | `packages/ui/src/components/settings/index.ts`                  |
| `skin`                           | Skin and cape buttons plus skin preview rendering UI.                                                                                                                                   | `packages/ui/src/components/skin/index.ts`                      |
| `user`                           | Shared user badge and user status presentation.                                                                                                                                         | `packages/ui/src/components/user/index.ts`                      |
| `version`                        | Version summary and filtering UI.                                                                                                                                                       | `packages/ui/src/components/version/index.ts`                   |
| `BrowsePageLayout`               | Shared browse tab layout with sidebar, header, selection bar, and browse install helpers.                                                                                               | `packages/ui/src/layouts/shared/browse-tab/index.ts`            |
| `ConsolePageLayout`              | Shared console page layout and console manager/provider surface.                                                                                                                        | `packages/ui/src/layouts/shared/console/index.ts`               |
| `ContentPageLayout`              | Shared content tab layout with cards, tables, selection, and content install and update modal flows.                                                                                    | `packages/ui/src/layouts/shared/content-tab/index.ts`           |
| `FilePageLayout`                 | Shared file manager layout with editor, upload, rename, move, delete, and conflict UI.                                                                                                  | `packages/ui/src/layouts/shared/files-tab/index.ts`             |
| `InstallationSettingsLayout`     | Shared installation settings layout and content diff and incompatibility flows.                                                                                                         | `packages/ui/src/layouts/shared/installation-settings/index.ts` |
| `ServerSettingsGeneralPage`      | Shared general server settings page export.                                                                                                                                             | `packages/ui/src/layouts/shared/server-settings/pages/index.ts` |
| `ServerSettingsInstallationPage` | Shared installation settings page for server configuration.                                                                                                                             | `packages/ui/src/layouts/shared/server-settings/pages/index.ts` |
| `ServerSettingsNetworkPage`      | Shared network settings page for hosted servers.                                                                                                                                        | `packages/ui/src/layouts/shared/server-settings/pages/index.ts` |
| `ServerSettingsPropertiesPage`   | Shared properties editor and settings page for servers.                                                                                                                                 | `packages/ui/src/layouts/shared/server-settings/pages/index.ts` |
| `ServerSettingsAdvancedPage`     | Shared advanced settings page for server management.                                                                                                                                    | `packages/ui/src/layouts/shared/server-settings/pages/index.ts` |
| `ServersManageRootLayout`        | Wrapped hosted server management root layout for full page route composition.                                                                                                           | `packages/ui/src/layouts/wrapped/index.ts`                      |
| `ServersManageOverviewPage`      | Wrapped hosted server overview page.                                                                                                                                                    | `packages/ui/src/layouts/wrapped/index.ts`                      |
| `ServersManageContentPage`       | Wrapped hosted server content management page.                                                                                                                                          | `packages/ui/src/layouts/wrapped/index.ts`                      |
| `ServersManageFilesPage`         | Wrapped hosted server file manager page.                                                                                                                                                | `packages/ui/src/layouts/wrapped/index.ts`                      |
| `ServersManageBackupsPage`       | Wrapped hosted server backups page.                                                                                                                                                     | `packages/ui/src/layouts/wrapped/index.ts`                      |
| `ServersManageAccessPage`        | Wrapped hosted server access management page.                                                                                                                                           | `packages/ui/src/layouts/wrapped/index.ts`                      |
| `ServerOnboardingPanelPage`      | Wrapped hosted server onboarding page.                                                                                                                                                  | `packages/ui/src/layouts/wrapped/index.ts`                      |
