import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { cdp } from "../cdp-client.js";
import { store } from "../store.js";
import { fmtAge, cleanStack } from "../format.js";

const NOT_CONNECTED = `CDP not connected. Start the app with: pnpm app:dev`;

export function register(server: McpServer): void {
	server.registerTool("tari_js_errors", {
		description: "Show JavaScript exceptions thrown in the Tari app with stack traces",
		inputSchema: {
			limit: z.number().int().min(1).max(50).optional().describe("Max errors to show (default: 20)"),
			newOnly: z.boolean().optional().describe("Only show errors from the last 60s (default: false)"),
		},
	}, async ({ limit = 20, newOnly = false }) => {
		if (!(await cdp.ensureConnected())) return { content: [{ type: "text", text: NOT_CONNECTED }] };

		let exceptions = store.getExceptions();
		if (newOnly) {
			const cutoff = Date.now() - 60_000;
			exceptions = exceptions.filter((e) => e.time >= cutoff);
		}
		exceptions = exceptions.slice(-limit);

		if (exceptions.length === 0) {
			return { content: [{ type: "text", text: newOnly ? "No exceptions in the last 60s." : "No exceptions recorded." }] };
		}

		const now = Date.now();
		const lines: string[] = [];
		for (const e of exceptions) {
			const age = fmtAge(now - e.time);
			const tag = now - e.time < 60_000 ? "[NEW]" : "[OLD]";
			lines.push(`${tag} ${age}  ${e.message}`);
			const frames = cleanStack(e.stack).slice(0, 4);
			for (const f of frames) lines.push(`  at ${f}`);
			lines.push("");
		}

		return { content: [{ type: "text", text: lines.join("\n").trimEnd() }] };
	});
}
