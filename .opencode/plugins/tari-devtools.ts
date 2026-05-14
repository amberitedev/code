import { plugin } from "@opencode-ai/plugin";

export default plugin({
	name: "tari-devtools",
	onCompact() {
		return [
			"Tari DevTools MCP tools are available for debugging the Tauri app WebView:",
			"  tari_status         — connection state and buffer counts",
			"  tari_console_summary — recent console output (deduped, filterable by level)",
			"  tari_js_errors      — JS exceptions with stack traces",
			"  tari_network_issues — failed/slow network requests",
			"  tari_performance    — JS heap, DOM nodes, layout metrics",
			"  tari_page_info      — URL, title, readyState",
			"  tari_vue_warnings   — [Vue warn] messages grouped+deduped",
			"  tari_clear          — clear all captured data",
			"CDP auto-enabled in dev builds (pnpm app:dev) — no manual env var needed. CDP on localhost:9222",
		].join("\n");
	},
});
