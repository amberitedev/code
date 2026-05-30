/**
 * Amberite devtools — single-file OpenCode plugin.
 *
 * Processes are started via Node.js spawn() (background, no visible terminal).
 * stdout/stderr are captured in module-level circular buffers (appLogLines / coreLogLines).
 * PID is persisted in app-state.json / core-state.json (TMP_DIR) for stop across sessions.
 * CDP is always enabled in debug builds (see apps/app/src/main.rs).
 *
 * All tool logic is module-private (no exports except AmberitePlugin).
 * appConsole, appLogs, appStart, appStop, appStatus (line 155+)
 * coreStart, coreStop, coreStatus, coreApi (line 345+)
 * Helpers: portOpen, waitPort, pwsh, killPort, killProcessTree, startSpawn (line 50+)
 */

import { type Plugin, tool } from "@opencode-ai/plugin";
import { spawn as spawnAsync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import net from "node:net";
import http from "node:http";
import WebSocket from "ws";

// ─── Config ───────────────────────────────────────────────────────────────────

const VITE_PORT = 1420;
const CDP_PORT = 9222;
const CORE_PORT = 16662;
const CORE_HOST = "127.0.0.1";

const TMP_DIR = join(tmpdir(), "opencode", "amberite");
const APP_STATE_FILE = join(TMP_DIR, "app-state.json");
const CORE_STATE_FILE = join(TMP_DIR, "core-state.json");

// ─── In-memory log buffers ────────────────────────────────────────────────────

const MAX_LOG_LINES = 2000;
const appLogLines: string[] = [];
const coreLogLines: string[] = [];

function appendLines(buf: string[], text: string) {
	const cleaned = text.replace(/\x1b\[[0-9;]*[mGKHJFA-Z]/g, "");
	for (const line of cleaned.split(/\r?\n/)) {
		if (!line.trim()) continue;
		buf.push(line);
		if (buf.length > MAX_LOG_LINES) buf.shift();
	}
}

function recentLines(buf: string[], n: number): string {
	return buf.slice(-n).join("\n") || "(no output captured yet)";
}

// ─── Utilities ────────────────────────────────────────────────────────────────

function sleep(ms: number) {
	return new Promise<void>((r) => setTimeout(r, ms));
}

async function portOpen(port: number, timeoutMs = 1200): Promise<boolean> {
	const check = (host: string) => new Promise<boolean>((resolve) => {
		const s = net.createConnection({ port, host });
		const t = setTimeout(() => { s.destroy(); resolve(false); }, timeoutMs);
		s.on("connect", () => { clearTimeout(t); s.destroy(); resolve(true); });
		s.on("error", () => { clearTimeout(t); resolve(false); });
	});
	// Try IPv4 and IPv6 loopback in parallel — Vite may bind [::1] on some Windows systems
	const [v4, v6] = await Promise.all([check("127.0.0.1"), check("::1")]);
	return v4 || v6;
}

async function waitPort(port: number, maxMs: number): Promise<boolean> {
	const deadline = Date.now() + maxMs;
	while (Date.now() < deadline) {
		if (await portOpen(port)) return true;
		await sleep(1500);
	}
	return false;
}

function pwsh(cmd: string, timeoutMs = 10000): string {
	const r = spawnSync(
		"powershell.exe",
		["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", cmd],
		{ timeout: timeoutMs, encoding: "utf8" },
	);
	return ((r.stdout as string) + (r.stderr as string)).trim();
}

function killPort(port: number) {
	pwsh(
		`Get-NetTCPConnection -LocalPort ${port} -ErrorAction SilentlyContinue` +
		` | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue }`,
	);
}

function killProcessTree(pid: number) {
	spawnSync("taskkill.exe", ["/PID", String(pid), "/T", "/F"], { windowsHide: true, timeout: 8000 });
}

// ─── State ────────────────────────────────────────────────────────────────────

interface ProcessState { terminalPid?: number }

function readState(file: string): ProcessState {
	if (!existsSync(file)) return {};
	try { return JSON.parse(readFileSync(file, "utf8")); } catch { return {}; }
}

function saveState(file: string, state: ProcessState) {
	mkdirSync(TMP_DIR, { recursive: true });
	writeFileSync(file, JSON.stringify(state, null, 2));
}

function isAlive(pid?: number): boolean {
	if (!pid) return false;
	try { process.kill(pid, 0); return true; } catch { return false; }
}

// ─── Process spawning ─────────────────────────────────────────────────────────

/**
 * Starts a shell command in the background (no visible window).
 * stdout/stderr are captured into `logBuf`. Returns the spawned PID.
 */
function startSpawn(cmd: string, cwd: string, logBuf: string[]): number {
	// shell:true on Windows routes through cmd.exe which properly inherits the pipe,
	// so child-process output flows back to us. No detach — we want the pipe alive.
	const proc = spawnAsync("powershell.exe", [
		"-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
		`& { ${cmd} } 2>&1`,
	], {
		cwd,
		windowsHide: true,
		stdio: ["ignore", "pipe", "pipe"],
	});
	if (!proc.pid) throw new Error("Process did not return a PID");
	const onData = (d: Buffer) => appendLines(logBuf, d.toString("utf8"));
	proc.stdout?.on("data", onData);
	proc.stderr?.on("data", onData);
	proc.unref();
	return proc.pid;
}

// ─── CDP ──────────────────────────────────────────────────────────────────────

async function cdpTargets(port: number): Promise<any[]> {
	return new Promise((resolve, reject) => {
		const req = http.get(`http://127.0.0.1:${port}/json`, (res) => {
			let raw = "";
			res.on("data", (d: Buffer) => (raw += d));
			res.on("end", () => {
				try { resolve(JSON.parse(raw)); }
				catch { reject(new Error("CDP /json returned invalid JSON")); }
			});
		});
		req.on("error", reject);
		req.setTimeout(3000, () => { req.destroy(); reject(new Error("CDP /json timed out")); });
	});
}

async function appConsole(code: string): Promise<string> {
	let targets: any[];
	try { targets = await cdpTargets(CDP_PORT); }
	catch (e: any) {
		return JSON.stringify({ success: false, message: `CDP not reachable on port ${CDP_PORT}: ${e.message}. Is the app running?` });
	}
	const target = targets.find((t) => t.type === "page" && !t.url?.startsWith("devtools://"))
		?? targets.find((t) => t.type === "page")
		?? targets[0];
	if (!target) return JSON.stringify({ success: false, message: "No CDP page target found." });

	return new Promise((resolve) => {
		const ws = new WebSocket(target.webSocketDebuggerUrl);
		const timer = setTimeout(() => {
			ws.close();
			resolve(JSON.stringify({ success: false, message: "CDP eval timed out after 10s." }));
		}, 10000);

		ws.on("open", () => {
			ws.send(JSON.stringify({
				id: 1,
				method: "Runtime.evaluate",
				params: { expression: code, awaitPromise: true, returnByValue: true, includeCommandLineAPI: true },
			}));
		});

		ws.on("message", (data: Buffer) => {
			const msg = JSON.parse(data.toString());
			if (msg.id !== 1) return;
			clearTimeout(timer);
			ws.close();
			if (msg.result?.exceptionDetails) {
				resolve(JSON.stringify({
					success: false,
					exception: msg.result.exceptionDetails.exception?.description ?? msg.result.exceptionDetails.text,
				}));
			} else {
				resolve(JSON.stringify({ success: true, result: msg.result?.result }));
			}
		});

		ws.on("error", (e: Error) => {
			clearTimeout(timer);
			resolve(JSON.stringify({ success: false, message: `WebSocket error: ${e.message}` }));
		});
	});
}

interface LogOpts { levels?: string[]; search?: string; limit?: number; clear?: boolean; }

function levelChar(level: string): string {
	switch ((level ?? "").toLowerCase()) {
		case "debug": return "D";
		case "info": case "log": return "I";
		case "warning": case "warn": return "W";
		case "error": return "E";
		default: return "-";
	}
}

/** Extract a compact `(file:line)` location suffix from a CDP event's params. */
function msgLocation(isConsole: boolean, params: any): string {
	if (isConsole) {
		const f = params.stackTrace?.callFrames?.[0];
		if (!f?.url) return "";
		return ` (${f.url.replace(/^.*\//, "").split("?")[0] || f.url}:${f.lineNumber + 1})`;
	}
	const e = params.entry;
	if (!e?.url) return "";
	const line = e.lineNumber != null ? `:${e.lineNumber}` : "";
	return ` (${e.url.replace(/^.*\//, "").split("?")[0] || e.url}${line})`;
}

async function appLogs(opts: LogOpts = {}): Promise<string> {
	const limit = Math.min(opts.limit ?? 200, 500);
	const levelChars = opts.levels?.map((l) => levelChar(l.toLowerCase())) ?? [];
	const searchRe = opts.search ? new RegExp(opts.search, "i") : null;
	const doClear = opts.clear ?? false;

	let targets: any[];
	try { targets = await cdpTargets(CDP_PORT); }
	catch (e: any) {
		return `ERROR: CDP not reachable on port ${CDP_PORT}: ${e.message}. Is the app running?`;
	}
	const target = targets.find((t) => t.type === "page" && !t.url?.startsWith("devtools://"))
		?? targets.find((t) => t.type === "page")
		?? targets[0];
	if (!target) return `ERROR: No CDP page target found.`;

	type RawMsg = { char: string; text: string; loc: string };
	const raw: RawMsg[] = [];

	return new Promise((resolve) => {
		const ws = new WebSocket(target.webSocketDebuggerUrl);
		let msgId = 0;

		ws.on("open", () => {
			// Console.enable replays all buffered console messages immediately.
			// Log.enable replays stored browser/network log entries.
			// Runtime.enable is intentionally omitted — it fires Runtime.consoleAPICalled
			// for the same messages Console.enable already replays, causing duplicates.
			ws.send(JSON.stringify({ id: ++msgId, method: "Console.enable" }));
			ws.send(JSON.stringify({ id: ++msgId, method: "Log.enable" }));
		});

		ws.on("message", (data: Buffer) => {
			const msg = JSON.parse(data.toString());
			if (msg.method === "Console.messageAdded") {
				const m = msg.params.message;
				const url: string = m.url ?? "";
				const loc = url ? ` (${url.replace(/^.*\//, "").split("?")[0] || url}:${m.line})` : "";
				raw.push({ char: levelChar(m.level), text: String(m.text ?? ""), loc });
			} else if (msg.method === "Log.entryAdded") {
				raw.push({
					char: levelChar(msg.params.entry.level),
					text: msg.params.entry.text as string,
					loc: msgLocation(false, msg.params),
				});
			}
		});

		ws.on("error", (e: Error) => {
			resolve(`ERROR: WebSocket error: ${e.message}`);
		});

		// Short window to receive the full replay burst.
		setTimeout(() => {
			if (doClear) {
				ws.send(JSON.stringify({ id: ++msgId, method: "Console.clearMessages" }));
				ws.send(JSON.stringify({ id: ++msgId, method: "Log.clear" }));
			}
			ws.close();

			let filtered = raw;
			if (levelChars.length > 0)
				filtered = filtered.filter((m) => levelChars.includes(m.char));
			if (searchRe)
				filtered = filtered.filter((m) => searchRe.test(m.text));
			filtered = filtered.slice(-limit);

			const lines: string[] = [];
			let prevKey = "", prevIdx = -1, rptCount = 0;
			for (const m of filtered) {
				const t = m.text.length > 160 ? m.text.slice(0, 157) + "..." : m.text;
				const key = m.char + t;
				const line = `${m.char} ${t}${m.loc}`;
				if (key === prevKey) { rptCount++; lines[prevIdx] = `${line} [x${rptCount + 1}]`; }
				else { prevKey = key; rptCount = 0; prevIdx = lines.length; lines.push(line); }
			}

			const header = `${filtered.length} messages`;
			resolve(lines.length ? `${header}\n${lines.join("\n")}` : `${header}\n(no messages)`);
		}, 500);
	});
}

// ─── App tools ────────────────────────────────────────────────────────────────

async function appStart(worktree: string): Promise<string> {
	const t0 = Date.now();

	// If the process is still alive, don't spawn another one regardless of CDP state
	const state = readState(APP_STATE_FILE);
	if (isAlive(state.terminalPid)) {
		const cdpUp = await portOpen(CDP_PORT);
		return JSON.stringify({
			success: cdpUp,
			alreadyRunning: true,
			terminalPid: state.terminalPid,
			message: cdpUp
				? "App is already running. Use app_status to verify."
				: "App process is alive but CDP is not up yet — still starting. Run app_status to check.",
		});
	}

	// Kill anything occupying app ports
	killPort(VITE_PORT);
	killPort(CDP_PORT);
	await sleep(800);

	appLogLines.length = 0;

	let terminalPid: number;
	try {
		terminalPid = startSpawn("pnpm app:dev", worktree, appLogLines);
		saveState(APP_STATE_FILE, { terminalPid });
	} catch (e: any) {
		return JSON.stringify({ success: false, message: `Failed to start process: ${e.message}` });
	}

	// Wait for Vite first (compile starts, Vite comes up fast)
	const viteUp = await waitPort(VITE_PORT, 90000);
	if (!viteUp) {
		return JSON.stringify({
			success: false, viteUp: false, cdpUp: false,
			terminalPid, waitedMs: Date.now() - t0,
			message: "Vite (port 1420) never came up in 90s. Check startupLog for errors.",
			startupLog: recentLines(appLogLines, 40),
		});
	}

	// Wait for CDP — Rust compile takes longer
	const cdpUp = await waitPort(CDP_PORT, 120000);
	return JSON.stringify({
		success: cdpUp, viteUp: true, cdpUp, terminalPid,
		waitedMs: Date.now() - t0,
		message: cdpUp
			? "App is ready. Console and log tools are available."
			: "Vite is up but CDP (port 9222) is not yet open. The Rust binary may still be compiling. Run app_status to check.",
		startupLog: recentLines(appLogLines, 40),
	});
}

async function appStop(): Promise<string> {
	const state = readState(APP_STATE_FILE);
	if (isAlive(state.terminalPid)) killProcessTree(state.terminalPid!);
	killPort(VITE_PORT);
	killPort(CDP_PORT);
	await sleep(1200);

	const viteStillUp = await portOpen(VITE_PORT);
	const cdpStillUp = await portOpen(CDP_PORT);
	const clean = !viteStillUp && !cdpStillUp;
	return JSON.stringify({
		success: clean, viteStillUp, cdpStillUp,
		message: clean ? "App stopped." : "Some processes still shutting down. Run app_stop again.",
	});
}

async function appStatus(): Promise<string> {
	const state = readState(APP_STATE_FILE);
	const terminalAlive = isAlive(state.terminalPid);
	const viteUp = await portOpen(VITE_PORT);
	const cdpUp = await portOpen(CDP_PORT);
	let cdpTargetCount = 0;
	if (cdpUp) {
		try { cdpTargetCount = (await cdpTargets(CDP_PORT)).length; } catch { /**/ }
	}
	return JSON.stringify({
		success: true, terminalPid: state.terminalPid, terminalAlive, viteUp, cdpUp, cdpTargetCount,
		message: cdpUp
			? `App running. Vite: ${viteUp}, CDP: up (${cdpTargetCount} target(s)). Console/log tools available.`
			: viteUp
				? "Vite is up but CDP is not open yet. Binary may still be compiling."
				: "App does not appear to be running.",
	});
}

// ─── Core tools ───────────────────────────────────────────────────────────────

async function coreFetch(method: string, path: string, body?: string): Promise<string> {
	return new Promise((resolve, reject) => {
		const opts: http.RequestOptions = {
			hostname: CORE_HOST, port: CORE_PORT,
			path, method: method.toUpperCase(),
			headers: body ? { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(body) } : {},
		};
		const req = http.request(opts, (res) => {
			let raw = "";
			res.on("data", (d: Buffer) => (raw += d));
			res.on("end", () => resolve(raw));
		});
		req.on("error", reject);
		req.setTimeout(8000, () => { req.destroy(); reject(new Error("Core API request timed out")); });
		if (body) req.write(body);
		req.end();
	});
}

async function coreStart(worktree: string): Promise<string> {
	const t0 = Date.now();

	// If the process is still alive, don't spawn another one
	const state = readState(CORE_STATE_FILE);
	if (isAlive(state.terminalPid)) {
		const portUp = await portOpen(CORE_PORT);
		return JSON.stringify({
			success: portUp,
			alreadyRunning: true,
			terminalPid: state.terminalPid,
			message: portUp
				? "Core is already running. Use core_status to verify."
				: "Core process is alive but port 16662 not up yet — still compiling. Run core_status to check.",
		});
	}

	killPort(CORE_PORT);
	await sleep(500);

	coreLogLines.length = 0;

	let terminalPid: number;
	try {
		terminalPid = startSpawn("pnpm core:dev", worktree, coreLogLines);
		saveState(CORE_STATE_FILE, { terminalPid });
	} catch (e: any) {
		return JSON.stringify({ success: false, message: `Failed to start process: ${e.message}` });
	}

	const up = await waitPort(CORE_PORT, 90000);
	if (!up) {
		return JSON.stringify({
			success: false, portUp: false, terminalPid,
			waitedMs: Date.now() - t0,
			message: "Core port 16662 never came up in 90s. Check startupLog for cargo errors.",
			startupLog: recentLines(coreLogLines, 40),
		});
	}

	// Try /health
	let healthy = false;
	try {
		const res = await coreFetch("GET", "/health");
		healthy = JSON.parse(res).status === "ok";
	} catch { /**/ }

	return JSON.stringify({
		success: true, portUp: true, healthy, terminalPid,
		waitedMs: Date.now() - t0,
		message: healthy ? "Core is ready." : "Core port is open but /health check failed.",
		startupLog: recentLines(coreLogLines, 40),
	});
}

async function coreStop(): Promise<string> {
	const state = readState(CORE_STATE_FILE);
	if (isAlive(state.terminalPid)) killProcessTree(state.terminalPid!);
	killPort(CORE_PORT);
	await sleep(1000);
	const stillUp = await portOpen(CORE_PORT);
	return JSON.stringify({
		success: !stillUp, portStillUp: stillUp,
		message: stillUp ? "Port 16662 still open. Run core_stop again." : "Core stopped.",
	});
}

async function coreStatus(): Promise<string> {
	const state = readState(CORE_STATE_FILE);
	const alive = isAlive(state.terminalPid);
	let healthy = false;
	let healthBody: any = null;
	if (alive) {
		try {
			healthBody = JSON.parse(await coreFetch("GET", "/health"));
			healthy = healthBody?.status === "ok";
		} catch { /**/ }
	}
	return JSON.stringify({
		success: alive,
		terminalPid: state.terminalPid,
		alive,
		healthy,
		healthBody,
		message: alive ? (healthy ? "Core running and healthy." : "Core process alive but /health not OK.") : "Core not running.",
	});
}

async function coreApi(method: string, path: string, body?: string): Promise<string> {
	const portUp = await portOpen(CORE_PORT);
	if (!portUp) return JSON.stringify({ success: false, message: "Core is not running (port 16662 is closed)." });
	try {
		const raw = await coreFetch(method, path, body);
		try { return JSON.stringify({ success: true, body: JSON.parse(raw) }); }
		catch { return JSON.stringify({ success: true, body: raw }); }
	} catch (e: any) {
		return JSON.stringify({ success: false, message: e.message });
	}
}

// ─── Plugin export ────────────────────────────────────────────────────────────

export const AmberitePlugin: Plugin = async () => {
	return {
		tool: {
			app_start: tool({
				description: [
					"Start the Amberite desktop app (pnpm app:dev) in a new terminal window.",
					"Waits up to 90s for Vite (port 1420) then up to 120s for CDP (port 9222).",
					"Kills any existing processes on those ports before starting.",
				].join(" "),
				args: {},
				async execute(_args, ctx) { return appStart(ctx.worktree); },
			}),

			app_stop: tool({
				description: "Stop the Amberite app. Kills Amberite.exe and any process on ports 1420 and 9222.",
				args: {},
				async execute() { return appStop(); },
			}),

			app_status: tool({
				description: "Check if the app is running. Reports Vite (1420) and CDP (9222) port status.",
				args: {},
				async execute() { return appStatus(); },
			}),

			app_console: tool({
				description: [
					"Runs a JavaScript expression in the live app WebView via CDP Runtime.evaluate —",
					"same as typing a command into DevTools console. Returns the result value or exception.",
					"console.log() calls inside the expression appear in the WebView console.",
					"Examples: document.title, window.__APP_VERSION__, fetch('/api').then(r=>r.json())",
				].join(" "),
				args: {
					code: tool.schema.string().describe("JavaScript expression to evaluate"),
				},
				async execute(args) { return appConsole(args.code); },
			}),

			app_logs: tool({
				description: [
					"Get all logs currently sitting in the app WebView console (CDP buffer replay).",
					"Returns everything like getLogs() with optional filters.",
					"Requires the app to be running with CDP available.",
				].join(" "),
				args: {
					levels: tool.schema.array(tool.schema.string()).optional().describe("Filter by levels — e.g. [\"error\", \"warn\"]. Omit for all levels."),
					search: tool.schema.string().optional().describe("Case-insensitive regex to filter message text"),
					limit: tool.schema.number().int().optional().describe("Max messages to return (default 200, max 500)"),
					clear: tool.schema.boolean().optional().describe("Clear CDP log buffer after reading — next call returns only new messages"),
				},
				async execute(args) {
					return appLogs({ levels: args.levels, search: args.search, limit: args.limit, clear: args.clear });
				},
			}),

			core_start: tool({
				description: [
					"Start the Amberite Core backend (pnpm core:dev) in a new terminal window.",
					"Waits up to 90s for port 16662 then checks /health.",
				].join(" "),
				args: {},
				async execute(_args, ctx) { return coreStart(ctx.worktree); },
			}),

			core_stop: tool({
				description: "Stop the Core backend. Kills process on port 16662 and amberite-core.exe.",
				args: {},
				async execute() { return coreStop(); },
			}),

			core_status: tool({
				description: "Check if Core is running. Reports whether the process is alive and calls /health.",
				args: {},
				async execute() { return coreStatus(); },
			}),

			core_api: tool({
				description: "Make an HTTP request to the Core backend at localhost:16662.",
				args: {
					method: tool.schema.string().optional().describe("HTTP method (default GET)"),
					path: tool.schema.string().describe("API path e.g. /health, /api/v1/instances"),
					body: tool.schema.string().optional().describe("JSON request body"),
				},
				async execute(args) { return coreApi(args.method ?? "GET", args.path, args.body); },
			}),
		},
	};
};
