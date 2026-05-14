import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { cdp } from "../cdp-client.js";
import { store } from "../store.js";
import { fmtAge, dedupeConsole, isVueWarning } from "../format.js";
import type { ConsoleEntry } from "../store.js";

const NOT_CONNECTED = `CDP not connected. Start the app with: pnpm app:dev`;

export function register(server: McpServer): void {
	server.registerTool("tari_page_info", {
		description: "Show current page URL, title, and document state of the Tari app",
		inputSchema: {},
	}, async () => {
		if (!(await cdp.ensureConnected())) return { content: [{ type: "text", text: NOT_CONNECTED }] };

		let pageInfo: { url: string; title: string; readyState: string } | null = null;
		try {
			pageInfo = (await cdp.evaluate(`(function(){ return JSON.stringify({url:location.href,title:document.title,readyState:document.readyState}); })()`)) as { url: string; title: string; readyState: string };
			if (typeof pageInfo === "string") pageInfo = JSON.parse(pageInfo) as { url: string; title: string; readyState: string };
		} catch {
			// ignore
		}

		const counts = store.counts();
		const lines = [
			`URL:          ${cdp.targetUrl}`,
			`Title:        ${cdp.targetTitle}`,
			pageInfo ? `Ready State:  ${pageInfo.readyState}` : "",
			`Interceptor:  ${cdp.interceptorInjected ? "active" : "inactive"}`,
			`Buffered:     ${counts.console} console | ${counts.exceptions} exceptions | ${counts.network} network`,
		].filter(Boolean);

		return { content: [{ type: "text", text: lines.join("\n") }] };
	});

	server.registerTool("tari_vue_warnings", {
		description: "Show [Vue warn] messages captured from the Tari app console. Grouped and deduped.",
		inputSchema: {
			limit: z.number().int().min(1).max(100).optional().describe("Max warnings to show (default: 30)"),
		},
	}, async ({ limit = 30 }) => {
		if (!(await cdp.ensureConnected())) return { content: [{ type: "text", text: NOT_CONNECTED }] };

		let raw: ConsoleEntry[] = [];
		try {
			const json = (await cdp.evaluate("JSON.stringify(window.__mcp_logs||[])")) as string;
			raw = JSON.parse(json) as ConsoleEntry[];
		} catch {
			return { content: [{ type: "text", text: "Interceptor not ready — try tari_status" }] };
		}

		const vueWarns = raw.filter((e) => isVueWarning(e.message));
		const deduped = dedupeConsole(vueWarns).slice(-limit);

		if (deduped.length === 0) return { content: [{ type: "text", text: "No Vue warnings." }] };

		const lines = deduped.map((e) => {
			const count = e.count && e.count > 1 ? ` ×${e.count}` : "";
			return `${e.message.slice(0, 300)}${count}`;
		});

		return { content: [{ type: "text", text: lines.join("\n\n") }] };
	});
}
