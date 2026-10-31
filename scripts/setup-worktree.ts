#!/usr/bin/env node

// Prepares a checkout for work: dependencies, development data, and the App library's environment
// file. T3 Code runs this when it creates a worktree (see t3.json); it is safe to run again by hand.
// Progress is recorded in .data/setup.json so an agent that starts before this finishes can tell
// "still running" from "never ran".

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'

// Local Minecraft, account, and server state. Everything else in the primary .data is scratch.
const DATA_ENTRIES = ['backend', 'core', 'scenarios'] as const
// Inside a scenario: Minecraft's game assets and the App window's browser caches. They are most
// of the files, nothing reads them until a game starts or a page loads, and both are fetched again
// on demand.
const SKIPPED_SCENARIO_PATHS = [
	/^meta\/assets$/,
	/^webview2\/EBWebView\/(?:.+\/)?(?:Cache|Code Cache|GPUCache|GrShaderCache|ShaderCache|Dawn\w+Cache)$/,
]
const ENV_FILE = NodePath.join('packages', 'app-lib', '.env')

const worktree = git(['rev-parse', '--show-toplevel'], process.cwd())
const primary = primaryWorktree(worktree)
const data = NodePath.join(worktree, '.data')
const statusPath = NodePath.join(data, 'setup.json')
const startedAt = new Date().toISOString()

NodeFS.mkdirSync(data, { recursive: true })
writeStatus('running')

// The install is the slow part, so the copies run while it works.
const install = installDependencies()

try {
	if (!samePath(worktree, primary)) {
		copyDevelopmentData()
		copyEnvFile()
	}
	for (const path of [
		NodePath.join(data, 'core'),
		...[1, 2, 3, 4].map((scenario) => NodePath.join(data, 'scenarios', String(scenario))),
	]) {
		NodeFS.mkdirSync(path, { recursive: true })
	}
	startBuild()
} catch (error) {
	// A failed copy must not hide the install result, or stop the install from finishing.
	console.error(`[setup] ${error instanceof Error ? error.message : String(error)}`)
	process.exitCode = 1
}

if (!(await install)) process.exitCode = 1
writeStatus(process.exitCode ? 'failed' : 'done')
console.log(
	process.exitCode
		? '[setup] failed. Fix the error above, then run: node scripts/setup-worktree.ts'
		: '[setup] done',
)

function copyDevelopmentData(): void {
	const source = NodePath.join(primary, '.data')
	for (const entry of DATA_ENTRIES) {
		const from = NodePath.join(source, entry)
		const to = NodePath.join(data, entry)
		if (!NodeFS.existsSync(from) || NodeFS.existsSync(to)) continue
		// Copied beside the target and moved into place, so a failed copy is retried next run
		// instead of being mistaken for finished data.
		const partial = `${to}.partial`
		NodeFS.rmSync(partial, { recursive: true, force: true })
		NodeFS.cpSync(from, partial, {
			filter: (path) =>
				entry !== 'scenarios' || !skippedScenarioPath(NodePath.relative(from, path)),
			recursive: true,
		})
		NodeFS.renameSync(partial, to)
		console.log(`[setup] copied ${entry} from ${source}`)
	}
}

/** Whether `path`, relative to the scenarios folder, is left out of the copy. */
function skippedScenarioPath(path: string): boolean {
	const inScenario = path.split(NodePath.sep).slice(1).join('/')
	return SKIPPED_SCENARIO_PATHS.some((pattern) => pattern.test(inScenario))
}

// A copy, not a link: Windows only allows symlinks with Developer Mode or elevation.
function copyEnvFile(): void {
	const from = NodePath.join(primary, ENV_FILE)
	const to = NodePath.join(worktree, ENV_FILE)
	if (!NodeFS.existsSync(from) || NodeFS.existsSync(to)) return
	NodeFS.copyFileSync(from, to)
	console.log(`[setup] copied ${ENV_FILE}`)
}

// Install scripts write the shared .git/config, so two worktrees set up at once can collide.
// One retry clears that.
async function installDependencies(): Promise<boolean> {
	for (const attempt of [1, 2]) {
		if ((await run('vp', ['i'])) === 0) return true
		console.error(`[setup] vp i failed${attempt === 1 ? '; retrying once' : ''}`)
	}
	return false
}

/**
 * Builds the App and Core in the background, so `up` has them ready by the time the work needs
 * them. It runs behind everything else, waits its turn behind other checkouts' builds, and finds
 * nothing to do when another checkout already built the same Rust. Its log is `.data/build.log`.
 */
function startBuild(): void {
	// Optional: `up` builds whatever is missing, so a build that cannot start never fails setup.
	const notStarted = (error: unknown) =>
		console.error(
			`[setup] background build not started: ${error instanceof Error ? error.message : String(error)}`,
		)
	try {
		const log = NodeFS.openSync(NodePath.join(data, 'build.log'), 'w')
		const child = NodeChildProcess.spawn(
			process.execPath,
			[NodePath.join(worktree, 'scripts', 'dev-runner.ts'), 'build'],
			{ cwd: worktree, detached: true, stdio: ['ignore', log, log], windowsHide: true },
		)
		child.once('error', notStarted)
		child.unref()
		NodeFS.closeSync(log)
	} catch (error) {
		notStarted(error)
	}
}

function run(command: string, args: ReadonlyArray<string>): Promise<number> {
	return new Promise((resolve) => {
		// The shell resolves vp.cmd on Windows.
		const child = NodeChildProcess.spawn([command, ...args].join(' '), {
			cwd: worktree,
			shell: true,
			stdio: 'inherit',
		})
		child.once('error', () => resolve(1))
		child.once('close', (code) => resolve(code ?? 1))
	})
}

function writeStatus(status: 'running' | 'done' | 'failed'): void {
	const finishedAt = status === 'running' ? undefined : new Date().toISOString()
	// Replaced whole, so an agent reading it mid-write never sees half a file.
	const partial = `${statusPath}.partial`
	NodeFS.writeFileSync(
		partial,
		`${JSON.stringify({ status, startedAt, finishedAt }, null, '\t')}\n`,
	)
	NodeFS.renameSync(partial, statusPath)
}

function primaryWorktree(cwd: string): string {
	const output = git(['worktree', 'list', '--porcelain'], cwd)
	const first = output
		.split(/\r?\n/)
		.find((line) => line.startsWith('worktree '))
		?.slice('worktree '.length)
	return NodePath.resolve(first ?? cwd)
}

function git(args: ReadonlyArray<string>, cwd: string): string {
	const result = NodeChildProcess.spawnSync('git', args, { cwd, encoding: 'utf8' })
	if (result.status !== 0) throw new Error(result.stderr.trim() || `git ${args.join(' ')} failed`)
	return result.stdout.trim()
}

function samePath(left: string, right: string): boolean {
	const normalize = (path: string) => {
		const resolved = NodePath.resolve(path)
		return process.platform === 'win32' ? resolved.toLowerCase() : resolved
	}
	return normalize(left) === normalize(right)
}
