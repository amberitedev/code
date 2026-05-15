# @modrinth/ui

Platform-agnostic shared Vue 3 component library — used by both `apps/frontend` (Nuxt website) and `apps/app-frontend` (Tauri desktop app).

---

## Structure

```
src/
  components/   Vue components by domain (17 subdirs, each with index.ts barrel)
  composables/  Vue 3 composables — many server-management-specific
  layouts/      Cross-platform page layouts (see below — most important for app-frontend)
  providers/    DI contexts via createContext pattern
  utils/        Utility functions
  locales/      34-language i18n files (FormatJS)
  stories/      Storybook stories (one per component)
  styles/       Tailwind utilities
```

Everything is re-exported from the root `index.ts`. Import from `@modrinth/ui`, not from deep paths.

---

## `src/layouts/` — Cross-Platform Page Layouts

The most active area for app-frontend work. Two categories:

**`shared/`** — Self-contained layout modules. Each subfolder is its own unit with `layout.vue`, `components/`, `composables/`, `providers/`, and `types.ts`:

| Module                   | What it is                  |
| ------------------------ | --------------------------- |
| `browse-tab/`            | Browse/discover content tab |
| `console/`               | Terminal console layout     |
| `content-tab/`           | Content/mods tab            |
| `files-tab/`             | File manager tab            |
| `installation-settings/` | Instance install settings   |
| `server-settings/`       | Server settings panel       |

**`wrapped/`** — Full page implementations consumed by both frontends. `hosting/manage/` contains `ServersManageRootLayout` and all server management sub-pages (`overview.vue`, `content.vue`, `files.vue`, `backups.vue`, `root.vue`). This is what `apps/app-frontend/src/pages/server/Index.vue` renders.

Files inside `layouts/` use the **`#ui/*` import alias** (e.g. `#ui/components/base/ButtonStyled.vue`) — defined in `package.json` `"imports"` field. Do not use relative paths across module boundaries here.

---

## `src/providers/` — Dependency Injection

All contexts use `createContext` from `providers/create-context.ts` — returns `[injectFoo, provideFoo]` pair. Throws at runtime if `inject` is called outside the providing tree (no silent null unless you pass `null` as fallback explicitly).

Key providers consumed by `apps/app-frontend`:

| Provider                                                   | File                   | What it provides         |
| ---------------------------------------------------------- | ---------------------- | ------------------------ |
| `provideCoreClient` / `injectCoreClient`                   | `core-client.ts`       | `CoreApiClient` instance |
| `provideAuth` / `injectAuth`                               | `auth.ts`              | Logged-in user state     |
| `provideModrinthClient` / `injectModrinthClient`           | `api-client.ts`        | Modrinth API client      |
| `provideNotificationManager` / `injectNotificationManager` | `web-notifications.ts` | Toast notifications      |

---

## Tailwind & Colors

Tailwind preset: `packages/tooling-config/tailwind/tailwind-preset.ts`. CSS custom properties (light/dark/OLED): `packages/assets/styles/variables.scss`.

**Backgrounds — always `surface-*`, never aliased `bg-*` color variables:**

| Token          | Usage                          |
| -------------- | ------------------------------ |
| `bg-surface-1` | Deepest background             |
| `bg-surface-2` | Secondary panels, even rows    |
| `bg-surface-3` | Headers, floating bars, inputs |
| `bg-surface-4` | Cards, elevated surfaces       |
| `bg-surface-5` | Borders, dividers              |

**Text:** `text-contrast` (headings) · `text-primary` (body) · `text-secondary` (reduced emphasis)

**Semantic:** `bg-{color}` / `bg-{color}-highlight` (25% opacity). Color palette with shades 50–950: red, orange, green, blue, purple, gray, plus platform colors (fabric, forge, quilt, neoforge, etc.).

---

## Code Rules

- **Adding a component:** must add/update a matching story in `src/stories/`.
- **`@click` multi-statement:** semicolons on one line only — newline-separated statements cause a Vue template parse error:
  ```vue
  <!-- BAD -->
  @click="foo = true $emit('bar')"
  <!-- GOOD -->
  @click="foo = true; $emit('bar')"
  ```
- **Platform-agnostic:** no Tauri imports, no `apps/app-lib` imports anywhere in this package. Platform-specific behavior goes through DI.
- **`flows/`** in `components/` is distinct from `layouts/shared/` — flows are modal/step-through sequences, not page layouts.
