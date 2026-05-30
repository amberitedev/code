/**
 * Tests for amberite.ts plugin tools.
 *
 * Initializes AmberitePlugin and calls each tool's execute method directly —
 * same code path OpenCode uses, same args schema, same return values.
 * Run: npx tsx test-plugin.ts [suite]
 *
 * Suites: status | console | logs | start | stop | core | core-start | core-stop | all
 * Default: status
 */

import { AmberitePlugin } from "./plugins/amberite.js";

const WORKTREE = "C:\\Users\\ilai\\code";
const suite = process.argv[2] ?? "status";

// Initialize plugin to get tool definitions (factory is sync-fast, no I/O at init time)
const hooks = await AmberitePlugin({} as any);
const T = hooks.tool!;

// Minimal ToolContext for execute calls — only worktree/directory are used by our tools
const ctx: any = {
	worktree: WORKTREE,
	directory: WORKTREE,
	sessionID: "test",
	messageID: "test",
	agent: "test",
	abort: new AbortController().signal,
	metadata: () => {},
	ask: () => {},
};

function section(name: string) {
	console.log(`\n${"─".repeat(60)}\n  ${name}\n${"─".repeat(60)}`);
}

/** Print result — pretty-prints JSON, or prints plain text as-is. */
function result(label: string, raw: string | { output: string }) {
	const text = typeof raw === "string" ? raw : raw.output;
	console.log(`\n[${label}]`);
	try { console.log(JSON.stringify(JSON.parse(text), null, 2)); }
	catch { console.log(text); }
}

async function exec(name: keyof typeof T, args: Record<string, unknown> = {}) {
	return T[name].execute(args as any, ctx);
}

async function testStatus() {
	section("app_status");
	result("app_status", await exec("app_status"));
}

async function testConsole() {
	section("app_console");
	result("document.title",     await exec("app_console", { code: "document.title" }));
	result("window.location.href", await exec("app_console", { code: "window.location.href" }));
	result("navigator.userAgent", await exec("app_console", { code: "navigator.userAgent" }));
	result("1 + 1",              await exec("app_console", { code: "1 + 1" }));
	result("exception test",     await exec("app_console", { code: "undefinedVar.property" }));
}

async function testLogs() {
	section("app_logs — all levels");
	result("all logs", await exec("app_logs"));

	section("app_logs — errors + warnings");
	result("errors+warn", await exec("app_logs", { levels: ["error", "warn"] }));

	section("app_logs — errors only");
	result("errors", await exec("app_logs", { levels: ["error"] }));
}

async function testStart() {
	section("app_start");
	result("app_start", await exec("app_start"));
}

async function testStop() {
	section("app_stop");
	result("app_stop", await exec("app_stop"));
}

async function testCore() {
	section("core_status");
	result("core_status", await exec("core_status"));

	section("core_api /health");
	result("GET /health", await exec("core_api", { method: "GET", path: "/health" }));
}

async function testCoreStart() {
	section("core_start");
	result("core_start", await exec("core_start"));
}

async function testCoreStop() {
	section("core_stop");
	result("core_stop", await exec("core_stop"));
}

const suites: Record<string, () => Promise<void>> = {
	status: testStatus,
	console: testConsole,
	logs: testLogs,
	start: testStart,
	stop: testStop,
	core: testCore,
	"core-start": testCoreStart,
	"core-stop": testCoreStop,
	all: async () => {
		await testStatus();
		await testConsole();
		await testLogs();
		await testCore();
	},
};

const fn = suites[suite];
if (!fn) {
	console.error(`Unknown suite: "${suite}". Valid: ${Object.keys(suites).join(", ")}`);
	process.exit(1);
}

console.log(`Running suite: ${suite}`);
fn().catch((e) => { console.error(e); process.exit(1); });
