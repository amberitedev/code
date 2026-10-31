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

## Protocol

A tab sends `{type: "invoke", id, cmd, args, options}` over its WebSocket. The App records the
invoke as pending and evaluates the dispatcher (`native-bridge.js`) in the native webview. The
dispatcher runs the real Tauri invoke and emits `{connectionId, id, status, payload}` on the single
event `browser-bridge://response`. One permanent listener forwards it to the tab as
`{type: "response", id, status, payload}`.

Every invoke gets exactly one response. When the native page cannot answer, the tab gets an error
instead of waiting:

- The native page has no Tauri (an error page after a failed reload) or does not run the script
  within 5 seconds. The App also reloads a page without Tauri, at most once every 10 seconds.
- The native page starts loading again before it answered.
- The request could not be sent to the native webview at all.

A failed request never ends the tab's session. An invoke unanswered for 30 seconds is logged once
with its command, followed by the native page's URL, ready state, and whether Tauri and the
dispatcher exist there.

The bridge serves its configuration as part of its external script to preserve the App's script
policy. It targets the main webview directly, including when the upstream ad view shares its window,
and forwards Vite's negotiated WebSocket protocol for hot reload.
