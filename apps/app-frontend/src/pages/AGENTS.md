# pages/

Route-mapped views. Each subfolder maps to a route group in `routes.js`. Each folder has an `index.js` that exports named views consumed by `routes.js`.

---

## Route Map

| Route                  | Folder            | Notes                                                 |
| ---------------------- | ----------------- | ----------------------------------------------------- |
| `/`                    | `Index.vue`       | Home                                                  |
| `/worlds`              | `Worlds.vue`      |                                                       |
| `/core`                | `Core.vue`        | **Amberite** — Core/friend-group settings dashboard   |
| `/core/setup`          | `CoreSetup.vue`   | **Amberite** — local/remote Core pairing setup        |
| `/skins`               | `Skins.vue`       |                                                       |
| `/browse/:projectType` | `Browse.vue`      |                                                       |
| `/library`             | `library/`        | Tabs: Overview, Downloaded, Modpacks, Servers, Custom |
| `/project/:id`         | `project/`        | Description, Versions, Version, Gallery, Changelog    |
| `/instance/:id`        | `instance/`       | Flat client/server/synced instance shell              |
| `/server/:id`          | `server/`         | **Amberite** — legacy/dev Core server shell           |
| `/synced/:id`          | `synced/`         | **Amberite** — legacy/dev synced shell                |
| `/hosting/manage/:id`  | `hosting/manage/` | **Amberite** — hosted (cloud) server management       |
| `/hosting/manage/`     | `Servers.vue`     | Server list                                           |

---

## Amberite-Specific Pages

**`instance/`** — product instance route for local clients, dedicated Core servers, and synced instances. `Index.vue` owns the flat shell and child pages branch on `GameInstance.kind`. Sub-pages: Overview, Content, Files, Worlds, Logs, Backups, Settings.

**`Core.vue` / `CoreSetup.vue`** — Core and friend-group management outside the active instance shell. These pages intentionally do not touch `/instance/:id`; they expose setup, member permission, invite, and sync scaffolding that the instance page can connect to later.

**`server/`** — legacy/dev dedicated server shell managed by Amberite Core. Product routes should use `/instance/:id` with `kind === 'server'`.

**`synced/`** — legacy/dev synced instance shell. Product routes should use `/instance/:id` with `kind === 'synced'`.

**`hosting/manage/`** — cloud-hosted server. Sub-pages: Overview, Content, Files, Backups.

---

## Route Meta

- `renderMode: 'fixed'` — used on Console and Logs routes; the parent layout switches to fixed-height flex so the terminal fills the viewport without page scroll.
- `useContext` — breadcrumb reads a dynamic name from context (e.g. project title).
- `useRootContext` — same, but from root context (instance name).
- `breadcrumb` with `?Name` prefix — name is resolved dynamically at runtime.

---

## Gotchas

- Flat `/instance/:id` is the product shell for local client, Core server, and synced instances. Branch inside the instance pages with `GameInstance.kind`; do not add `/instance/:id/server` or `/instance/:id/synced` route namespaces.
- `/instance/:id` renders Overview. Content lives at `/instance/:id/content`.
- URL redirect at `/:projectType(mod|plugin|...)/:id` normalizes old Modrinth URLs to `/project/:id`.
- Scroll behavior targets `.app-viewport` element, not `window` — Linux workaround (`routes.js:349`).
