import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { cdp } from "../cdp-client.js";
import { store } from "../store.js";
import { fmtDuration, fmtBytes, truncUrl } from "../format.js";

const NOT_CONNECTED = `CDP not connected. Start the app with: pnpm app:dev`;

export function register(server: McpServer): void {
	server.registerTool("tari_network_issues", {
		description: "Show failed or slow network requests from the Tari app",
		inputSchema: {
			slowThresholdMs: z.number().int().min(100).optional().describe("Slow request threshold in ms (default: 2000)"),
			limit: z.number().int().min(1).max(100).optional().describe("Max entries (default: 30)"),
		},
	}, async ({ slowThresholdMs = 2000, limit = 30 }) => {
		if (!(await cdp.ensureConnected())) return { content: [{ type: "text", text: NOT_CONNECTED }] };

		const all = store.getNetwork();
		const issues = all.filter((r) => {
			if (r.failed) return true;
			if (r.status >= 400) return true;
			if (r.endTime && r.endTime - r.startTime > slowThresholdMs) return true;
			return false;
		}).slice(-limit);

		if (issues.length === 0) {
			return { content: [{ type: "text", text: `No failed or slow (>${slowThresholdMs}ms) requests.` }] };
		}

		const lines = issues.map((r) => {
			const duration = r.endTime ? fmtDuration(r.endTime - r.startTime) : "pending";
			const status = r.failed ? `FAILED(${r.errorText ?? "?"})` : String(r.status || "?");
			const url = truncUrl(r.url, 60);
			return `${r.method.padEnd(6)}  ${url.padEnd(60)}  ${status.padEnd(12)}  ${duration}`;
		});

		return { content: [{ type: "text", text: lines.join("\n") }] };
	});
}
