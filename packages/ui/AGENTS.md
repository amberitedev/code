# Architecture

The shared UI package used by both `apps/frontend` (Nuxt 3) and `apps/app-frontend` (Vue 3 + Tauri). Components here must be platform-agnostic — use dependency injection for platform-specific behavior.

## Folder Structure

```
src/
├── components/       # Vue components organized by feature domain
├── composables/      # Vue 3 composition API hooks
├── layouts/          # Self-contained page layouts (see below)
├── providers/        # Dependency injection contexts (createContext pattern)
├── utils/            # Utility functions and constants
├── pages/            # Cross platform page components (used in both app-frontend and frontend)
├── locales/          # 34 language locale files (FormatJS)
├── styles/           # Tailwind CSS utilities
└── stories/          # Storybook story files
```

Each subdirectory under `components/` has an `index.ts` barrel file. All public API is re-exported from the root `index.ts`.

### `src/layouts/`

Self-contained page layouts shared across frontends. Split into two categories:

- **`shared/`** — Reusable layout modules with their own components, composables, providers, and types. Each module is a self-contained unit (e.g. `shared/content-tab/` contains the content/mods tab layout with its own `layout.vue`, `components/`, `composables/`, `providers/`, and `types.ts`).
- **`wrapped/`** — Page-level Vue components that mirror route structures (e.g. `wrapped/hosting/manage/`). These are full page implementations consumed by both `apps/frontend` and `apps/app-frontend`.

Files inside `layouts/` use the `#ui/*` import alias (resolved via the `"imports"` field in `package.json`) to reference other `src/` modules like `#ui/components/base/ButtonStyled.vue` or `#ui/composables/i18n`.

# Code Guidelines

### Tailwind Configuration

All frontend packages share a Tailwind preset at `packages/tooling-config/tailwind/tailwind-preset.ts`. This package's `tailwind.config.ts` extends it:

```ts
import preset from '@modrinth/tooling-config/tailwind/tailwind-preset.ts'
```

CSS custom properties are defined in `packages/assets/styles/variables.scss` with light, dark, and OLED theme variants.

### Color Usage Rules

**Use `surface-*` variables for backgrounds — never aliased `bg-*` color variables:**

| Token            | Usage                                     |
| ---------------- | ----------------------------------------- |
| `bg-surface-1`   | Deepest background layer                  |
| `bg-surface-1.5` | Odd row background (tables)               |
| `bg-surface-2`   | Even row background, secondary panels     |
| `bg-surface-3`   | Headers, floating bar backgrounds, inputs |
| `bg-surface-4`   | Cards, elevated surfaces                  |
| `bg-surface-5`   | Borders, dividers                         |

**For text colors:**

| Class            | Usage                            |
| ---------------- | -------------------------------- |
| `text-contrast`  | Primary headings                 |
| `text-primary`   | Default body text                |
| `text-secondary` | Reduced emphasis, secondary info |

**Brand and semantic colors** not all exposed as Figma variables — refer to `packages/assets/styles/variables.scss` for the full set:

- `bg-{color}`, `text-{color}` etc. — Primary brand colors
- `bg-{color}-highlight` — 25% opacity semantic highlights

**Color palette** (each with shades 50–950): red, orange, green, blue, purple, gray. Platform-specific colors also exist (fabric, forge, quilt, neoforge, etc.).

## Storybook

When modifying a component in `src/components/`, you must also update its corresponding Storybook story in `src/stories/` to reflect the changes. If a story file doesn't exist yet, create one. Stories should cover the component's key states and variants - do not make or modify a storybook unless the user asks for it or skip if it's incredibly obvious one should not be needed (e.g minor changes or styling changes DO NOT need a storybook edit)

## Dependency Injection

This package defines the DI layer using `createContext` from `src/providers/index.ts`. See the `dependency-injection` skill (`.claude/skills/dependency-injection/SKILL.md`) for full documentation.

Key providers exported from this package:

- `provideModrinthClient` / `injectModrinthClient` — API client
- `provideNotificationManager` / `injectNotificationManager` — Notifications

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
- If you add a new shared component or materially change the role or description of an existing shared component, update this list in the same change.

| Component | Description | Filepath |
| --- | --- | --- |
| `affiliate` | Affiliate link management UI, including listing cards and creation modal flows. | `packages/ui/src/components/affiliate/index.ts` |
| `base` | Core shared primitives such as buttons, cards, inputs, selects, tables, tabs, modals, empty states, progress, and page scaffolding. | `packages/ui/src/components/base/index.ts` |
| `billing` | Purchase, payment method, subscription, and hosted server checkout UI. | `packages/ui/src/components/billing/index.ts` |
| `brand` | Shared Modrinth branding components such as animated and text logos. | `packages/ui/src/components/brand/index.ts` |
| `changelog` | Changelog entry presentation for release and update content. | `packages/ui/src/components/changelog/index.ts` |
| `chart` | Shared chart visualizations, including compact chart variants. | `packages/ui/src/components/chart/index.ts` |
| `content` | Shared content list and article card UI for browse-style surfaces. | `packages/ui/src/components/content/index.ts` |
| `external_files` | UI for looking up and describing external project file licensing and permissions. | `packages/ui/src/components/external_files/index.ts` |
| `modal` | Shared modal shells and common confirmation, share, and install modal variants. | `packages/ui/src/components/modal/index.ts` |
| `nav` | Shared navigation UI such as breadcrumbs, banners, and notification panels. | `packages/ui/src/components/nav/index.ts` |
| `notifications` | Toast and notification stack rendering. | `packages/ui/src/components/notifications/index.ts` |
| `page` | Reusable page-level wrappers such as normal pages and sidebar cards. | `packages/ui/src/components/page/index.ts` |
| `project` | Project cards, headers, sidebars, version displays, server info, and project settings UI. | `packages/ui/src/components/project/index.ts` |
| `search` | Shared search filters, categories, and sidebar filtering controls. | `packages/ui/src/components/search/index.ts` |
| `servers` | Hosted server UI, including listings, access management, backups, headers, icons, setup flows, labels, and promos. | `packages/ui/src/components/servers/index.ts` |
| `settings` | Shared settings selectors such as language and theme pickers. | `packages/ui/src/components/settings/index.ts` |
| `skin` | Skin and cape buttons plus skin preview rendering UI. | `packages/ui/src/components/skin/index.ts` |
| `user` | Shared user badge and user status presentation. | `packages/ui/src/components/user/index.ts` |
| `version` | Version summary and filtering UI. | `packages/ui/src/components/version/index.ts` |
| `BrowsePageLayout` | Shared browse tab layout with sidebar, header, selection bar, and browse install helpers. | `packages/ui/src/layouts/shared/browse-tab/index.ts` |
| `ConsolePageLayout` | Shared console page layout and console manager/provider surface. | `packages/ui/src/layouts/shared/console/index.ts` |
| `ContentPageLayout` | Shared content tab layout with cards, tables, selection, and content install and update modal flows. | `packages/ui/src/layouts/shared/content-tab/index.ts` |
| `FilePageLayout` | Shared file manager layout with editor, upload, rename, move, delete, and conflict UI. | `packages/ui/src/layouts/shared/files-tab/index.ts` |
| `InstallationSettingsLayout` | Shared installation settings layout and content diff and incompatibility flows. | `packages/ui/src/layouts/shared/installation-settings/index.ts` |
| `ServerSettingsGeneralPage` | Shared general server settings page export. | `packages/ui/src/layouts/shared/server-settings/pages/index.ts` |
| `ServerSettingsInstallationPage` | Shared installation settings page for server configuration. | `packages/ui/src/layouts/shared/server-settings/pages/index.ts` |
| `ServerSettingsNetworkPage` | Shared network settings page for hosted servers. | `packages/ui/src/layouts/shared/server-settings/pages/index.ts` |
| `ServerSettingsPropertiesPage` | Shared properties editor and settings page for servers. | `packages/ui/src/layouts/shared/server-settings/pages/index.ts` |
| `ServerSettingsAdvancedPage` | Shared advanced settings page for server management. | `packages/ui/src/layouts/shared/server-settings/pages/index.ts` |
| `ServersManageRootLayout` | Wrapped hosted server management root layout for full page route composition. | `packages/ui/src/layouts/wrapped/index.ts` |
| `ServersManageOverviewPage` | Wrapped hosted server overview page. | `packages/ui/src/layouts/wrapped/index.ts` |
| `ServersManageContentPage` | Wrapped hosted server content management page. | `packages/ui/src/layouts/wrapped/index.ts` |
| `ServersManageFilesPage` | Wrapped hosted server file manager page. | `packages/ui/src/layouts/wrapped/index.ts` |
| `ServersManageBackupsPage` | Wrapped hosted server backups page. | `packages/ui/src/layouts/wrapped/index.ts` |
| `ServersManageAccessPage` | Wrapped hosted server access management page. | `packages/ui/src/layouts/wrapped/index.ts` |
| `ServerOnboardingPanelPage` | Wrapped hosted server onboarding page. | `packages/ui/src/layouts/wrapped/index.ts` |
