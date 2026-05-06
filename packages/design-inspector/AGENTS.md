# Design Inspector Package

A dev-only design feedback tool. Hold **Alt** in the app to activate the inspector, hover to highlight a component, then click to open a comment bubble. Comments are POSTed to `http://localhost:4096/design-comments` (the opencode server).

## Architecture

- **`vue-plugin.ts`** — Vue plugin that mounts a separate `createApp(DesignInspectorRoot)` at `#design-inspector-ui` (body-level sibling to `#app`). This isolation means the inspector has no access to the main app's Pinia store or notification system.
- **`vite-plugin.ts`** — Vite plugin that hooks `vite-plugin-vue-inspector` and redirects the virtual overlay module to `InspectorOverlay.vue`.
- **`InspectorOverlay.vue`** — Injected into the main app by the Vite plugin. Listens for Alt key → crosshair mode → click → dispatches `design-inspector:pick` CustomEvent on `window`.
- **`DesignInspectorRoot.vue`** — Separate Vue app. Listens for `design-inspector:pick` → mounts `CommentBubble`. Also shows `FetchErrorToast` on submit failure.
- **`CommentBubble.vue`** — Fixed-position bubble (z-index 9999) with component chip, contenteditable input, send button. Uses CSS custom properties via inline fallback styles (belt-and-suspenders: Tailwind classes + explicit `style=` attributes ensure visibility regardless of CSS scan status).
- **`FetchErrorToast.vue`** — Self-contained bottom-right toast (z-index 10000). Auto-dismisses after 4s. Uses only inline styles — no dependency on Tailwind or main app CSS.
- **`useCommentSubmit.ts`** — POSTs to the discovered opencurser URL at `/design-comments`. URL is injected at Vite build/dev time via `__OPENCURSER_URL__`. Returns `{ submit, loading, error }`.
- **`useClipboardMode.ts`** — Singleton composable. Manages the auto-clipboard toggle (`Alt+Shift+C`). State is persisted to `localStorage` (key `di:autoClipboard`). Exports `useClipboardMode()` (registers the keybind — call once from `DesignInspectorRoot`) and `copyPayloadToClipboard(payload)` (standalone function, safe to call from any component; no-ops when mode is off).
- **`usePosition.ts`** — Calculates fixed-position coords for the bubble (right → left → above → center fallback).
- **`InspectorClone.vue`** — Persistent coloured overlay for Win+Alt sticky picks. Custom colour, 16-colour picker on name hover, **no close button**. Dismissed via Escape or token deletion.

## Overlay Taxonomy

Every overlay the design inspector can render, with its canonical name:

| Name | Component | Trigger | Animated | Colour | Picker | Close |
|------|-----------|---------|----------|--------|--------|-------|
| **HoverHighlight** | `ElementHighlight` in `InspectorOverlay` | Alt held + hover | ✓ | green (fixed) | ✗ | ✗ |
| **PickModeHighlight** | `ElementHighlight` in `InspectorOverlay` | Win+Alt pick-for-bubble mode | ✓ | green (fixed) | ✗ | ✗ |
| **CommentHighlight** | `ElementHighlight` in `DesignInspectorRoot` (non-sticky) | Plain Alt click | ✗ | green (fixed) | ✗ | via `CommentBubble` |
| **InspectorClone** | `InspectorClone` in `DesignInspectorRoot` (sticky) | Win+Alt inline pick | ✗ | custom palette | ✓ | Escape / token delete |

Edge cases covered:
- **No name** (anonymous element) → name chip not rendered; colour picker not shown
- **Element scrolled/off-screen** → width/height 0; overlay collapses to zero-size, still tracked
- **Element removed from DOM** → `liveEl` set to `null`; overlay removed from `activeClones`
- **PickModeHighlight without inspector data** → name shows empty string; overlay still highlights geometry

## Debug Logging

All `diLog`/`console.log` calls are gated behind `import.meta.env.DEV`. They are silent in production builds.

## Server Discovery

The design inspector never talks to OpenCurser directly from the browser. Instead, all requests go through a reverse-proxy at `/__design-relay` on the Vite dev server (same origin as the app — always `localhost:1420` in dev).

`vite-plugin.ts` adds a `configureServer` middleware that:
1. Re-reads `%TEMP%/opencurser-server.json` on **every request** (never stale)
2. Proxies the request to whatever OpenCurser URL is in the lock file
3. Falls back to `http://localhost:4096` if the lock file is absent (CLI mode)

**Result**: OpenCurser can restart with a new random port and the very next comment POST will find it automatically — no Vite restart, no `__OPENCURSER_URL__` globals, no `ReferenceError` risk, no CSP issues.

The lock file `{ url, auth }` is written by the OpenCurser Electron app at startup (`packages/desktop-electron/src/main/index.ts`).

## TODO

- The `FetchErrorToast` gives user feedback on submit failure, but the "opencurser not running" case deserves a better UX — perhaps a persistent badge or tooltip explaining the server is not available.

## Rules

- Max 200 lines per file (hard rule inherited from root AGENTS.md)
- Inspector-internal toast/notifications MUST be self-contained — do NOT use the main app's Pinia store or notification system
- All new components go in `components/`, composables in `composables/`
- `vite-plugin.ts` is Node.js only — never import it from browser entry points
