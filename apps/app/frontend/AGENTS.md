# Amberite Frontend

Vue 3 UI forked from Modrinth App. Communicates with Tauri backend via `invoke()`.

## Source Structure

```
src/
├── assets/           # Icons, SVGs, external assets
├── components/       # Vue components
│   └── ui/           # UI components (modal, settings, friends, install_flow, etc.)
├── composables/      # Vue composables (useMemorySlider, useInstanceConsole, macCssFix)
├── directives/       # Vue directives (overlayScrollbars)
├── helpers/          # Utility modules for Tauri calls
├── locales/          # i18n translations
├── pages/            # Route pages (Index, library, instance, project, hosting, worlds, skins)
├── plugins/          # Vue plugins (i18n)
├── providers/        # Vue provide/inject (content-install, server-install, download-progress)
├── store/            # Pinia stores (state, loading, error, install, breadcrumbs, theme)
├── App.vue           # Root component
├── main.js           # Entry point
├── routes.js         # Vue Router routes
└── config.ts         # Config (API URLs)
```

## Vite Alias Configuration

`vite.config.ts` maps Modrinth packages to Amberite equivalents:

```ts
{ find: '@modrinth/ui', replacement: '@amberite/ui' }
{ find: '@modrinth/api-client', replacement: '@amberite/api-client' }
{ find: '@modrinth/assets', replacement: '@amberite/assets' }
{ find: '@modrinth/utils', replacement: '@amberite/utils' }
```

Import using `@amberite/*` names. These packages come from the Modrinth monorepo (git dependency).

## Import Conventions

- Use `@` alias for `src/`: `import { thing } from '@/helpers/utils'`
- Use `@amberite/*` for Modrinth packages: `import { Button } from '@amberite/ui'`
- Tauri APIs: wrap in helpers with fallbacks for dev mode

## Routing

`routes.js` defines all routes. Pages in `pages/` are auto-exported via `index.js`.

Key routes:
- `/` - Home
- `/library/*` - Instance library
- `/instance/:id/*` - Instance detail (mods, files, logs, worlds)
- `/project/:id/*` - Project pages
- `/browse/:projectType` - Content discovery
- `/hosting/manage/*` - Server management
- `/worlds`, `/skins` - Additional pages

## Calling Backend

Create helper functions in `helpers/` that wrap Tauri `invoke()`:

```js
// helpers/amberite.ts
import { invoke } from '@tauri-apps/api/core'

export async function amberiteHello() {
  return await invoke('plugin:amberite|hello')
}
```

Then use in components:

```js
import { amberiteHello } from '@/helpers/amberite'
const result = await amberiteHello()
```

## State Management

Pinia stores in `store/`:

- `state.js` - Theme, navigation, sidebar state
- `loading.js` - Loading indicator control
- `error.js` - Error modal state
- `install.js` - Installation flow state
- `breadcrumbs.js` - Navigation breadcrumbs
- `theme.ts` - Theme settings, feature flags

Use `useTheming()`, `useLoading()`, `useError()` composables.

## Providers

Vue provide/inject for cross-component state:

- `content-install` - Mod/project installation
- `server-install` - Server installation
- `download-progress` - Download tracking
- `app-notifications` - Toast notifications
- `app-popup-notifications` - Popup notifications

Setup in `providers/setup.ts`. Use `provideContentInstall()`, `provideServerInstall()`.

## Dev Commands

From `apps/app/frontend/`:
```bash
pnpm dev           # Vite dev server (port 1420)
pnpm build         # Type-check + build
pnpm tsc:check     # TypeScript check
pnpm lint          # ESLint + Prettier check
pnpm fix           # Auto-fix lint issues
```

## Mocking Tauri APIs

Dev mode runs without Tauri. Helpers must handle missing `@tauri-apps/*`:

```js
// Safe import pattern
let invoke
try {
  invoke = require('@tauri-apps/api/core').invoke
} catch {
  invoke = async () => console.log('Mock: tauri not available')
}
```

Or check `window.__TAURI__` existence before calling.

## File Limits

- Max 200 lines per file (strict)
- One component per file
- Never overwrite files - edit existing