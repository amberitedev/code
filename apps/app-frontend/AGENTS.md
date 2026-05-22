# app-frontend

Vue 3 frontend for the Amberite desktop app — a fork of the Modrinth desktop app with added Amberite Core integration for server management and synced instances.

---

## Structure

```
src/
  adapters/     Tauri bridge (PlatformAdapter singleton)
  components/   App-specific components (ui/ has all of them)
  composables/  Small Vue composables
  directives/   Vue directives (overlay-scrollbars)
  helpers/      All business logic — Tauri invoke wrappers + API calls
  pages/        Route-mapped views
  plugins/      i18n, i18n-debug
  providers/    DI/setup — wires Vue provide/inject at boot
  store/        Pinia stores (breadcrumbs, theme, error, install)
  locales/      i18n translation files
  routes.js     Full route table
  main.js       App bootstrap
```

---

## Key Relationships

- **`adapters/desktop.ts`** — singleton `PlatformAdapter` from `@amberite/amberite-api`. Bridges OS keychain session storage, the app-launched Core setup secret, Convex URL, Tauri HTTP fetch (bypasses WebView CORS), and `invoke()` calls to Core. Every page that talks to Core creates a `CoreApiClient` with this adapter. Convex Auth login/refresh is owned by `helpers/amberite-auth.ts`, not the adapter itself.
- **`@amberite/amberite-api`** — provides `CoreApiClient` and `PlatformAdapter` types. Used by pages that need Core API access.
- **`@modrinth/ui`** — shared component library. Server management UI (`ServersManageRootLayout`) lives here, not in this package. See `packages/ui/AGENTS.md`.
- **`packages/app-lib/`** — provides Tauri commands. `.env` for this app is read from `packages/app-lib/.env` (loaded manually in `vite.config.ts`), not from this directory.
- **`helpers/`** — all Tauri `invoke()` wrappers and API logic. Pages import from here, not from `@modrinth/ui` data utilities.

---

## What Amberite Added (vs Modrinth fork)

| Area                       | Where                         |
| -------------------------- | ----------------------------- |
| Core-managed server pages  | `pages/server/`               |
| Core-synced instance pages | `pages/synced/`               |
| Hosted server management   | `pages/hosting/manage/`       |
| Tauri/Convex/Core bridge   | `adapters/desktop.ts`         |
| Server install providers   | `providers/server-install.ts` |

Everything else (`Browse`, `Library`, `Project`, `Instance`, `Skins`, `Worlds`) is inherited from Modrinth.

---

## Navigation

| Need                                        | Go to                          |
| ------------------------------------------- | ------------------------------ |
| Route table                                 | `src/routes.js`                |
| Page views                                  | `src/pages/` → `AGENTS.md`     |
| Business logic / Tauri calls                | `src/helpers/` → `AGENTS.md`   |
| Boot/DI wiring                              | `src/providers/` → `AGENTS.md` |
| App-level state                             | `src/store/`                   |
| App-specific components                     | `src/components/ui/`           |
| Shared UI library (components, layouts, DI) | `packages/ui/` → `AGENTS.md`   |

---

## Dev Tools

### Theme Editor (Ctrl+Shift+T)

A standalone Tauri dev window for live-editing CSS custom property theme values. Changes propagate to the main window in real time via Tauri events and persist across restarts via `localStorage`.

| File                                                  | Purpose                                                                                                                   |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `theme-editor.html`                                   | Standalone Vite entry point (separate from main `index.html`)                                                             |
| `src/theme-editor-main.ts`                            | Minimal Vue app mount (no Pinia/i18n)                                                                                     |
| `src/composables/useThemeEditorComms.ts`              | Module-level reactive singleton; loads saved overrides on import; emits `tw:var-change` / `tw:var-reset-all` Tauri events |
| `src/components/theme-editor/ThemeEditorApp.vue`      | Shell: tab bar, dark/light toggle, Copy CSS, Reset All                                                                    |
| `src/components/theme-editor/ThemeEditorSurfaces.vue` | Surface and text color pickers                                                                                            |
| `src/components/theme-editor/ThemeEditorPalette.vue`  | Full color scale palette (red/orange/green/blue/purple/gray × 11 shades)                                                  |
| `src/components/theme-editor/ThemeEditorTokens.vue`   | Gap/radius sliders, platform color pickers                                                                                |

Tauri capability: `apps/app/capabilities/theme-editor.json` (window label `theme-editor`).
