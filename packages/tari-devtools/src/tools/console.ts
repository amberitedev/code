import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { cdp } from "../cdp-client.js";
import { fmtTime, levelTag, dedupeConsole } from "../format.js";
import type { ConsoleEntry } from "../store.js";

const NOT_CONNECTED = `CDP not connected. Start the app with: pnpm app:dev`;

export function register(server: McpServer): void {
	server.registerTool("tari_console_summary", {
		description: "Show recent console output from the Tari app. Deduped and compact.",
		inputSchema: {
			level: z.enum(["all", "errors", "warnings", "errors+warnings"]).optional().describe("Filter by level (default: errors+warnings)"),
			limit: z.number().int().min(1).max(200).optional().describe("Max entries to show (default: 50)"),
		},
	}, async ({ level = "errors+warnings", limit = 50 }) => {
		if (!(await cdp.ensureConnected())) return { content: [{ type: "text", text: NOT_CONNECTED }] };

		let raw: ConsoleEntry[] = [];
		try {
			const json = (await cdp.evaluate("JSON.stringify(window.__mcp_logs||[])")) as string;
			raw = JSON.parse(json) as ConsoleEntry[];
		} catch {
			return { content: [{ type: "text", text: "Interceptor not ready — try tari_status" }] };
		}

		const levelFilter = (l: string): boolean => {
			if (level === "all") return true;
			if (level === "errors") return l === "error";
			if (level === "warnings") return l === "warn";
			if (level === "errors+warnings") return l === "error" || l === "warn";
			return true;
		};

		const filtered = raw.filter((e) => levelFilter(e.level));
		const deduped = dedupeConsole(filtered).slice(-limit);

		if (deduped.length === 0) return { content: [{ type: "text", text: `No ${level} entries.` }] };

		const lines = deduped.map((e) => {
			const count = e.count && e.count > 1 ? ` ×${e.count}` : "";
			return `${fmtTime(e.time)}  ${levelTag(e.level)}  ${e.message.slice(0, 200)}${count}`;
		});

		return { content: [{ type: "text", text: lines.join("\n") }] };
	});
}
