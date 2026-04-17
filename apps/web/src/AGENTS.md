# src/ — Source code entry point

## Context Loading
**Parent `../AGENTS.md` is the main panel guide — read that first.** This file points to deeper source-specific context.

Load on-demand:
- **Components:** `components/<name>/AGENTS.md`
- **Pages:** `pages/<name>/AGENTS.md`
- **Store:** `store/<name>/AGENTS.md`
- **Providers:** `providers/<name>/AGENTS.md`
- **Helpers:** `helpers/<name>/AGENTS.md`

## Key Files
| File | Purpose |
|------|---------|
| `App.vue` | Root component — layout, nav, modals, global state setup |
| `main.js` | App bootstrap, plugin registration |
| `routes.js` | All route definitions with breadcrumb metadata |
| `i18n.config.ts` | Vue I18n setup — 35+ languages |

## Patterns
- **Components:** `<script setup>` + Composition API
- **State:** Pinia (global) + Vue Query (server) + Providers (DI)
- **Routing:** `meta.breadcrumb` for nav, `useContext`/`useRootContext` for data loading

## Tauri Migration
- **Mocked:** `mocks/tauri-apps.ts` for dev
- **Goal:** Remove all `@tauri-apps/*` imports, replace with web APIs or backend HTTP calls

## State Stores
| Store | Purpose |
|-------|---------|
| `breadcrumbs.js` | Navigation breadcrumb state |
| `error.js` | Error handling + modal display |
| `install.js` | Content installation state |
| `loading.js` | Loading indicator control |
| `state.js` | App-wide initialization state |
| `theme.ts` | Theme preferences (light/dark/advanced rendering) |

## Providers (DI)
| Provider | Purpose |
|----------|---------|
| `content-install.ts` | Modpack/mod installation context |
| `server-install.ts` | Server instance installation context |
| `app-notifications.ts` | Notification manager |
| `instance-settings.ts` | Instance configuration context |

## API Layer
See `api/AGENTS.md` for HTTP client details and endpoint patterns.
