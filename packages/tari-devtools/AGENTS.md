# tari-devtools — MCP Server Package

Provides AI-friendly Chrome DevTools Protocol (CDP) tools for debugging the Tauri app's WebView.

## Package Info
- Name: `@amberite/tari-devtools`
- Entry: `src/index.ts` (MCP stdio server)
- Deps: `@modelcontextprotocol/sdk`, `zod`, `ws`

## Architecture
- `src/cdp-client.ts` — `CdpClient` class + `cdp` singleton. Connects to `localhost:9222`, injects JS interceptor, streams CDP events into store.
- `src/store.ts` — `DevToolsStore` singleton (`store`). Ring buffers for exceptions (100) and network (200). Console captured via injected interceptor in page (`window.__mcp_logs`).
- `src/format.ts` — Output formatters: `fmtTime`, `fmtDuration`, `fmtAge`, `fmtBytes`, `truncUrl`, `cleanStack`, `levelTag`, `isVueWarning`, `dedupeConsole`.
- `src/tools/` — One file per tool group, each exports `register(server)`.

## Available MCP Tools
| Tool | Description |
|---|---|
| `tari_status` | Connection state, buffer counts |
| `tari_clear` | Clears all buffers |
| `tari_console_summary` | Deduped console logs, filterable by level |
| `tari_js_errors` | JS exceptions with cleaned stack traces |
| `tari_network_issues` | Failed/slow requests |
| `tari_performance` | JS heap, DOM nodes, layout metrics |
| `tari_page_info` | URL, title, readyState, buffer counts |
| `tari_vue_warnings` | `[Vue warn]` messages, grouped+deduped |

## Prerequisites
CDP is opt-in for debug builds. Start the app with `AMBERITE_ENABLE_CDP=true pnpm app:dev`.
When enabled, CDP listens on `http://localhost:9222`.

## Gotchas
- Console history requires the JS interceptor (`window.__mcp_logs`). CDP Runtime.consoleAPICalled only fires for new messages after connection.
- Re-injection happens on `Page.frameNavigated` automatically.
- `ws` must be listed as a dependency (not built into Bun's Node compat layer for `WebSocket`).
