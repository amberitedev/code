import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { cdp } from "../cdp-client.js";
import { store } from "../store.js";

const NOT_CONNECTED = `CDP not connected. Start the app with: pnpm app:dev\nThen retry — the MCP server auto-connects on first tool call.`;

export function register(server: McpServer): void {
	server.registerTool("tari_status", {
		description: "Show Tari DevTools connection status and buffer stats",
		inputSchema: {},
	}, async () => {
		if (!(await cdp.ensureConnected())) return { content: [{ type: "text", text: NOT_CONNECTED }] };
		const counts = store.counts();
		const lines = [
			`Connected: ${cdp.targetTitle || cdp.targetUrl}`,
			`Interceptor: ${cdp.interceptorInjected ? "active" : "inactive"}`,
			`Buffer: ${counts.console} console | ${counts.exceptions} exceptions | ${counts.network} network`,
		];
		return { content: [{ type: "text", text: lines.join("\n") }] };
	});

	server.registerTool("tari_clear", {
		description: "Clear all captured logs, exceptions, and network entries from Tari DevTools buffers",
		inputSchema: {},
	}, async () => {
		store.clear();
		if (cdp.connected) {
			try {
				await cdp.evaluate("window.__mcp_logs = []; window.__mcp_errs = [];");
			} catch {
				// best-effort
			}
		}
		return { content: [{ type: "text", text: "Cleared." }] };
	});
}
