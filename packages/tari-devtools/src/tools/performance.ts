import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { cdp } from "../cdp-client.js";
import { fmtBytes } from "../format.js";

const NOT_CONNECTED = `CDP not connected. Start the app with: pnpm app:dev`;

interface Metric { name: string; value: number }

const METRIC_LABELS: Record<string, string> = {
	JSHeapUsedSize: "JS Heap Used",
	JSHeapTotalSize: "JS Heap Total",
	Nodes: "DOM Nodes",
	JSEventListeners: "Event Listeners",
	RecalcStyleCount: "Style Recalcs",
	LayoutCount: "Layouts",
	ScriptDuration: "Script Duration",
	LayoutDuration: "Layout Duration",
	RecalcStyleDuration: "Style Duration",
};

export function register(server: McpServer): void {
	server.registerTool("tari_performance", {
		description: "Show runtime performance metrics for the Tari app (heap, DOM, layout, scripts)",
		inputSchema: {},
	}, async () => {
		if (!(await cdp.ensureConnected())) return { content: [{ type: "text", text: NOT_CONNECTED }] };

		let metrics: Metric[];
		try {
			const result = (await cdp.request("Performance.getMetrics", {})) as { metrics: Metric[] };
			metrics = result.metrics;
		} catch (e) {
			return { content: [{ type: "text", text: `Failed to get metrics: ${e}` }] };
		}

		const byName = Object.fromEntries(metrics.map((m) => [m.name, m.value]));
		const lines: string[] = [];

		const heap = byName["JSHeapUsedSize"];
		const heapTotal = byName["JSHeapTotalSize"];
		if (heap != null) lines.push(`JS Heap:          ${fmtBytes(heap)} / ${fmtBytes(heapTotal ?? 0)}`);

		const nodes = byName["Nodes"];
		if (nodes != null) lines.push(`DOM Nodes:        ${Math.round(nodes)}`);

		const listeners = byName["JSEventListeners"];
		if (listeners != null) lines.push(`Event Listeners:  ${Math.round(listeners)}`);

		const layouts = byName["LayoutCount"];
		if (layouts != null) lines.push(`Layouts:          ${Math.round(layouts)}`);

		const scripts = byName["ScriptDuration"];
		if (scripts != null) lines.push(`Script Duration:  ${(scripts * 1000).toFixed(1)}ms`);

		const layout = byName["LayoutDuration"];
		if (layout != null) lines.push(`Layout Duration:  ${(layout * 1000).toFixed(1)}ms`);

		if (lines.length === 0) return { content: [{ type: "text", text: "No metrics available." }] };
		return { content: [{ type: "text", text: lines.join("\n") }] };
	});
}
