# Amberite browser bridge

This internal development package lets Amberite's unmodified desktop frontend run in Chrome while
all Tauri calls still execute in Amberite's real native webview and backend.

The bridge is deliberately loopback-only. It is enabled by the App's `browser-bridge` Cargo
feature, which Amberite's development launcher passes automatically and production builds omit.
The browser URL is printed when the native App starts.

Compatibility lives entirely in this package: direct `@tauri-apps/*` imports, events, channels,
invoke options, errors, binary values, the asset protocol, and Vite's development frontend are
bridged without App-side shims.
