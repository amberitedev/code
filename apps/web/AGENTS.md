# panel/ — Amberite Server Dashboard (UI-only — FOR NOW)

> **Status:** Early stage — extracted UI from desktop app. Many features are stubbed/mocked. Each iteration replaces mocks with real implementations.

## Context Loading
Read `../AGENTS.md` first. For deeper context:
- **Source code details:** `src/AGENTS.md` (load for component patterns, entry points)
- **Component details:** `src/components/<name>/AGENTS.md` (load on-demand)
- **Page details:** `src/pages/<name>/AGENTS.md` (load on-demand)
- **API layer:** `src/api/AGENTS.md` (load on-demand)

## What it is
Amberite Server Dashboard — UI-only frontend for managing Minecraft servers. Originally a Tauri desktop app; the UI has been extracted and all Tauri-specific helpers mocked to work in a browser environment. **Goal:** Fully migrate away from Tauri to a web-first architecture. Currently a skeleton — backend logic lives in Amberite Core (Rust/Axum) on port `16662`.

## Tech Stack
- **Vue 3** — Composition API with `<script setup>` syntax
- **TypeScript** — Strict type checking across all components
- **Tailwind CSS** — Utility-first styling via `tailwind.config.ts`
- **Vite** — Fast build tool and dev server
- **Vue Router** — Client-side routing with nested routes
- **Pinia** — State management (stores in `src/store/`)
- **@tanstack/vue-query** — Server state management and caching
- **vue-i18n** — Internationalization (35+ languages in `src/locales/`)
- **Tauri** — Desktop app bundling (APIs mocked in dev) — **BEING PHASED OUT**

## Commands
| Command | Description |
|---------|-------------|
| `pnpm dev` | Start Vite dev server (port 5173) |
| `pnpm build` | Type-check + production build |
| `pnpm preview` | Preview production build |

## Directory Structure
```
panel/
├── src/
│   ├── App.vue                 # Root component (1500+ lines)
│   ├── main.js                 # Entry point
│   ├── routes.js               # Vue Router configuration
│   ├── i18n.config.ts          # Vue I18n setup
│   ├── components/
│   │   ├── GridDisplay.vue     # Grid layout component
│   │   ├── RowDisplay.vue      # Row layout component
│   │   ├── LoadingIndicatorBar.vue
│   │   └── ui/                 # Shared UI components
│   │       ├── AccountsCard.vue
│   │       ├── Breadcrumbs.vue
│   │       ├── ErrorModal.vue
│   │       ├── NavButton.vue
│   │       ├── QuickInstanceSwitcher.vue
│   │       ├── RunningAppBar.vue
│   │       ├── SplashScreen.vue
│   │       ├── friends/        # Friends list components
│   │       ├── install_flow/   # Installation modals
│   │       ├── modal/          # Generic modals
│   │       └── settings/       # Settings components
│   ├── pages/
│   │   ├── Index.vue           # Home page
│   │   ├── Browse.vue          # Content browser
│   │   ├── Skins.vue           # Skin browser
│   │   ├── Worlds.vue          # Worlds page
│   │   ├── instance/           # Instance detail pages
│   │   │   ├── Index.vue       # Mods/content tab
│   │   │   ├── Files.vue
│   │   │   ├── Logs.vue
│   │   │   ├── Worlds.vue
│   │   │   └── Overview.vue
│   │   ├── library/            # Library pages
│   │   │   ├── Index.vue
│   │   │   ├── Downloaded.vue
│   │   │   ├── Modpacks.vue
│   │   │   ├── Servers.vue
│   │   │   ├── Custom.vue
│   │   │   └── Overview.vue
│   │   └── project/            # Project pages
│   │       ├── Index.vue
│   │       ├── Description.vue
│   │       ├── Versions.vue
│   │       ├── Version.vue
│   │       ├── Gallery.vue
│   │       └── Changelog.vue
│   ├── store/
│   │   ├── breadcrumbs.js
│   │   ├── error.js
│   │   ├── install.js
│   │   ├── loading.js
│   │   ├── state.js
│   │   └── theme.ts
│   ├── composables/            # Vue composables
│   ├── providers/              # Context providers
│   │   ├── content-install.ts
│   │   ├── server-install.ts
│   │   ├── app-notifications.ts
│   │   └── instance-settings.ts
│   ├── plugins/                # Vue plugins
│   ├── helpers/                # Utility functions
│   │   ├── ads.js              # Ad window (Tauri-specific)
│   │   ├── analytics.ts        # Event tracking
│   │   ├── auth.js             # Auth helpers
│   │   ├── mr_auth.ts          # Modrinth auth
│   │   ├── settings.ts         # Settings persistence
│   │   ├── logs.js             # Log parsing
│   │   ├── skins.ts            # Skin handling
│   │   └── utils.js            # General utilities
│   ├── mocks/                  # Dev mocks — TEMPORARY
│   │   └── tauri-apps.ts       # Mocks Tauri APIs for web dev
│   └── locales/                # i18n translations (35+ languages)
├── packages/                   # Internal monorepo packages
├── index.html
├── package.json
├── vite.config.ts
├── tailwind.config.ts
└── tsconfig*.json
```

## Routes

| Path | Component | Data Load | Purpose |
|------|-----------|-----------|---------|
| `/` | `Pages.Index` | — | Home/dashboard |
| `/worlds` | `Pages.Worlds` | — | World management |
| `/hosting/manage/` | `ServersManagePageIndex` | — | Server hosting (@modrinth/ui) |
| `/browse/:projectType` | `Pages.Browse` | `useContext` | Modrinth content browser |
| `/skins` | `Pages.Skins` | — | Skin browser |
| `/library` | `Library.Index` | — | Library index |
| `/library/downloaded` | `Library.Downloaded` | — | Downloaded content |
| `/library/modpacks` | `Library.Modpacks` | — | Modpacks list |
| `/library/servers` | `Library.Servers` | — | Server projects |
| `/library/custom` | `Library.Custom` | — | Custom instances |
| `/project/:id` | `Project.Index` → `Description` | `useContext` | Project description |
| `/project/:id/versions` | `Project.Index` → `Versions` | `useContext` | Version list |
| `/project/:id/version/:version` | `Project.Index` → `Version` | `useContext` | Single version |
| `/project/:id/gallery` | `Project.Index` → `Gallery` | `useContext` | Image gallery |
| `/instance/:id` | `Instance.Index` → `Mods` | `useRootContext` | Instance content/mods |
| `/instance/:id/worlds` | `Instance.Index` → `Worlds` | `useRootContext` | Instance worlds |
| `/instance/:id/files` | `Instance.Index` → `Files` | `useRootContext` | File manager |
| `/instance/:id/logs` | `Instance.Index` → `Logs` | `useRootContext` | Log viewer |

Routes use `meta.breadcrumb` for navigation. `useContext` loads project data; `useRootContext` loads instance data.

## API Communication
- **Base URL:** `http://localhost:16662` (Amberite Core)
- **Client:** `GenericModrinthClient` from `@modrinth/api-client`
- **Auth:** JWT tokens via `AuthFeature`
- **HTTP:** `ofetch` (`$fetch`) for direct calls

## Internal Packages
| Package | Purpose |
|---------|---------|
| `@modrinth/ui` | Shared UI components (buttons, modals, layouts) |
| `@modrinth/assets` | Icons, SVGs, static assets |
| `@modrinth/utils` | Utility functions (`formatBytes`, `renderString`) |
| `@modrinth/api-client` | API client with auth features |
