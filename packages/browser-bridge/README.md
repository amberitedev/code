# Amberite browser bridge

This internal development package lets Amberite's unmodified desktop frontend run in Chrome while
all Tauri calls still execute in Amberite's real native webview and backend.

The bridge is deliberately loopback-only. It is enabled by the App's `browser-bridge` Cargo
feature, which Amberite's development launcher passes automatically and production builds omit.
The browser URL is printed when the native App starts.

Run `vp run dev 1 2` for two isolated Apps and open each printed browser bridge URL. The raw Vite URL
does not provide Tauri calls. Each bridge needs its native App process to remain running; this package
avoids desktop-window automation, but does not remove the native build or backend requirement.

Compatibility lives entirely in this package: direct `@tauri-apps/*` imports, events, channels,
invoke options, errors, binary values, the asset protocol, and Vite's development frontend are
bridged without App-side shims.

The bridge serves its configuration as part of its external script to preserve the App's script
policy. It targets the main webview directly, including when the upstream ad view shares its window,
and forwards Vite's negotiated WebSocket protocol for hot reload.
