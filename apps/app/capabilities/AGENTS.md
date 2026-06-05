# apps/app/capabilities

Tauri capability definitions for the desktop app.

## What Capabilities Are

- Capabilities are permission bundles that define what the app webview is allowed to access.
- They gate use of Tauri core APIs, Tauri plugins, custom command namespaces, network destinations, and filesystem scope.
- If the frontend can call a Rust command but Tauri still blocks it, the missing piece is often here.

## What Lives Here

```text
capabilities/
  core.json                 Base window/app/webview/core permissions
  plugins.json              Plugin permissions, HTTP allowlist, FS scope, and command bundles
  ads.json                  Ads-specific permissions
  updater.json              Updater-specific permissions
```

## Important Details

- `core.json` covers general app/window/webview permissions such as show/hide, minimize/maximize, decorations, dragging, and zoom.
- `plugins.json` is the main capability bundle. It includes:
  - Tauri plugin permissions like dialog, opener, OS info, deep-link, and window-state
  - network allowlists for the HTTP plugin
  - filesystem scope limits for profile directories
  - permission bundles for custom command namespaces like `auth`, `process`, `settings`, `files`, `worlds`, and others
- `ads.json` and `updater.json` isolate narrower permission groups for those features.
