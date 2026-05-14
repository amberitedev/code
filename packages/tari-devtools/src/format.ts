// format.ts: compact, token-efficient output formatters for DevTools tool responses

import type { ConsoleEntry } from "./store.js";

function fmtTime(ts: number): string {
	const d = new Date(ts);
	const h = d.getHours().toString().padStart(2, "0");
	const m = d.getMinutes().toString().padStart(2, "0");
	const s = d.getSeconds().toString().padStart(2, "0");
	return `${h}:${m}:${s}`;
}

function fmtDuration(ms: number): string {
	if (ms >= 60_000) return `${(ms / 60_000).toFixed(1)}m`;
	if (ms >= 10_000) return `${(ms / 1000).toFixed(1)}s`;
	if (ms >= 1_000) return `${(ms / 1000).toFixed(2)}s`;
	return `${Math.round(ms).toLocaleString()}ms`;
}

function fmtAge(ms: number): string {
	if (ms < 60_000) return `${Math.round(ms / 1000)}s`;
	if (ms < 3600_000) return `${Math.round(ms / 60_000)}m`;
	return `${Math.round(ms / 3600_000)}h`;
}

function fmtBytes(bytes: number): string {
	if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GB`;
	if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MB`;
	if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`;
	return `${bytes} B`;
}

function truncUrl(url: string, max = 65): string {
	if (url.length <= max) return url;
	try {
		const u = new URL(url);
		const base = u.hostname + (u.port ? `:${u.port}` : "");
		const path = u.pathname + u.search;
		const avail = max - base.length - 4;
		if (avail > 8) return `${base}/\u2026${path.slice(-avail)}`;
	} catch {}
	return `\u2026${url.slice(-(max - 1))}`;
}

function cleanStack(raw: string | string[] | undefined, maxFrames = 4): string[] {
	if (!raw) return [];
	const lines = Array.isArray(raw) ? raw : raw.split("\n").filter((l) => l.trim().startsWith("at "));
	return lines
		.slice(0, maxFrames)
		.map((l) =>
			l
				.trim()
				.replace(/\(https?:\/\/localhost:\d+\//g, "(")
				.replace(/https?:\/\/localhost:\d+\//g, "")
				.replace(/\(.*?node_modules.*?\)/g, "(node_modules)")
		);
}

function levelTag(level: string): string {
	const map: Record<string, string> = {
		error: "ERROR",
		warning: "WARN ",
		warn: "WARN ",
		info: "INFO ",
		log: "LOG  ",
		debug: "DEBUG",
	};
	return map[level] ?? level.slice(0, 5).toUpperCase().padEnd(5);
}

function isVueWarning(msg: string): boolean {
	return msg.includes("[Vue warn]");
}

function dedupeConsole(entries: readonly ConsoleEntry[]): ConsoleEntry[] {
	const result: ConsoleEntry[] = [];
	for (const entry of entries) {
		const last = result[result.length - 1];
		if (last && last.level === entry.level && last.message === entry.message) {
			last.count = (last.count ?? 1) + 1;
		} else {
			result.push({ ...entry, count: 1 });
		}
	}
	return result;
}

export { fmtTime, fmtDuration, fmtAge, fmtBytes, truncUrl, cleanStack, levelTag, isVueWarning, dedupeConsole };
