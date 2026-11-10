#!/usr/bin/env node

// Prepares a checkout for work: dependencies, development data, and the App library's environment
// file. T3 Code runs this when it creates a worktree (see t3.json); it is safe to run again by hand.
// Progress is recorded in .data/setup.json so an agent that starts before this finishes can tell
// "still running" from "never ran". While running it names this process's pid: a pid that no longer
// exists means setup was killed, so run it again.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'
import { copyDataEntry, DATA_ENTRIES } from './dev-data.ts'
import { recordTiming } from './timings.ts'

const ENV_FILE = NodePath.join('packages', 'app-lib', '.env')

const worktree = git(['rev-parse', '--show-toplevel'], process.cwd())
const primary = primaryWorktree(worktree)
const data = NodePath.join(worktree, '.data')
const statusPath = NodePath.join(data, 'setup.json')
const startedAt = new Date().toISOString()
const started = performance.now()
const ms: Record<string, number> = {}
let error = ''
let cancelled = false

NodeFS.mkdirSync(data, { recursive: true })
writeStatus('running')
// Every way out ends with a final status, so an agent never waits on a setup that is gone. A hard
// kill skips this, which the dead pid shows.
process.on('uncaughtExceptionMonitor', (thrown) => {
	error = thrown instanceof Error ? (thrown.stack ?? thrown.message) : String(thrown)
})
process.on('exit', (code) => {
	const result = cancelled ? 'cancelled' : code === 0 ? 'ok' : 'failed'
	writeStatus(result === 'ok' ? 'done' : 'failed')
	ms.total = Math.round(performance.now() - started)
	recordTiming(primary, { kind: 'setup', worktree, branch: currentBranch(), result, ms })
})
let install: NodeChildProcess.ChildProcess | undefined
for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP'] as const) {
	process.on(signal, () => {
		cancelled = true
		error = `Cancelled by ${signal}.`
		// The install ends on the signal, and setup exits once it has.
		if (install) stopInstall(install, signal)
		else process.exit(1)
	})
}

// The install is the slow part, so the copies run while it works.
const installed = timed('install', installDependencies)

try {
	if (!samePath(worktree, primary)) {
		await timed('data', async () => copyDevelopmentData())
		copyEnvFile()
	}
	for (const path of [
		NodePath.join(data, 'core'),
		...[1, 2, 3, 4].map((scenario) => NodePath.join(data, 'scenarios', String(scenario))),
	]) {
		NodeFS.mkdirSync(path, { recursive: true })
	}
} catch (thrown) {
	// A failed copy must not hide the install result, or stop the install from finishing.
	error = thrown instanceof Error ? thrown.message : String(thrown)
	console.error(`[setup] ${error}`)
	process.exitCode = 1
}

if (!(await installed)) process.exitCode = 1
if (cancelled) process.exitCode = 1
console.log(
	process.exitCode ? '[setup] failed. Fix the error above, then run: vp run setup' : '[setup] done',
)

// Only what is missing, so running setup again never replaces data the work has changed.
function copyDevelopmentData(): void {
	const source = NodePath.join(primary, '.data')
	for (const entry of DATA_ENTRIES) {
		if (!NodeFS.existsSync(NodePath.join(source, entry))) continue
		if (NodeFS.existsSync(NodePath.join(data, entry))) continue
		copyDataEntry(source, data, entry)
		console.log(`[setup] copied ${entry} from ${source}`)
	}
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
// One retry clears that. A stale lockfile fails instead of being rewritten, and with stdin closed a
// prompt fails instead of waiting forever.
async function installDependencies(): Promise<boolean> {
	for (const attempt of [1, 2]) {
		if (cancelled) return false
		if ((await run('vp i --frozen-lockfile -- --yes')) === 0) return true
		if (cancelled) return false
		error = 'vp i failed. Its output is in the setup terminal.'
		console.error(`[setup] vp i failed${attempt === 1 ? '; retrying once' : ''}`)
	}
	return false
}

// Output goes straight to the terminal: the data copy blocks this process, and a piped install
// would stall behind it.
function run(command: string): Promise<number> {
	return new Promise((resolve) => {
		// The shell resolves vp.cmd on Windows.
		const child = NodeChildProcess.spawn(command, {
			cwd: worktree,
			shell: true,
			// Its own process group off Windows, so a cancel can stop the shell and vp together.
			detached: process.platform !== 'win32',
			stdio: ['ignore', 'inherit', 'inherit'],
		})
		install = child
		child.once('error', () => resolve(1))
		child.once('close', (code) => {
			install = undefined
			resolve(code ?? 1)
		})
	})
}

// Killing only the shell would leave vp installing, and a retry could start a second install.
function stopInstall(child: NodeChildProcess.ChildProcess, signal: NodeJS.Signals): void {
	if (child.pid === undefined) return
	if (process.platform === 'win32') {
		NodeChildProcess.spawnSync('taskkill', ['/pid', String(child.pid), '/T', '/F'])
	} else {
		process.kill(-child.pid, signal)
	}
}

async function timed<T>(step: string, work: () => Promise<T>): Promise<T> {
	const start = performance.now()
	try {
		return await work()
	} finally {
		ms[step] = Math.round(performance.now() - start)
	}
}

function writeStatus(status: 'running' | 'done' | 'failed'): void {
	const finishedAt = status === 'running' ? undefined : new Date().toISOString()
	const details = status === 'running' ? { pid: process.pid } : status === 'failed' ? { error } : {}
	// Replaced whole, so an agent reading it mid-write never sees half a file.
	const partial = `${statusPath}.partial`
	NodeFS.writeFileSync(
		partial,
		`${JSON.stringify({ status, startedAt, finishedAt, ...details }, null, '\t')}\n`,
	)
	NodeFS.renameSync(partial, statusPath)
}

function currentBranch(): string {
	try {
		return git(['branch', '--show-current'], worktree)
	} catch {
		return ''
	}
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
