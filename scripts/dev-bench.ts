#!/usr/bin/env node

// How fast the dev loop is for an agent in a new worktree, as one score. Each scenario is timed
// the way an agent waits on it: worktree setup, `up` until the App is usable, and for a change
// that would ship, `check:changed`. A scenario scores 100 when it takes its target time, 200 at
// half of it. The overall score is their harmonic mean, so one slow scenario holds it down: a
// 20 minute cold start cannot be bought back with a 1 second warm one.
//
//   node scripts/dev-bench.ts                  the quick scenarios, about 12 minutes
//   node scripts/dev-bench.ts --full           every scenario, about 40 minutes
//   node scripts/dev-bench.ts app rebuild      only these
//
// Scenarios not run keep their last result in the score, marked with the snapshot they ran on.
//
// It benchmarks this checkout's current files, uncommitted ones included, in a sandbox: a clone
// of the primary checkout at `<primary>-bench` (or BENCH_ROOT) with its own shared build folder,
// timings log and data. It never touches the primary's builds or data. Scenarios start from empty
// development data, which every service creates on its own: a copied dataset ties the result to
// whatever schema the primary's data happens to have. Results go to `<root>/results.jsonl`;
// command output to `<root>/logs`.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'

/** One-line edits, each made unique per run so no build cache already has them. */
const EDITS = {
	ui: (nonce: string) => [
		append('apps/app-frontend/src/App.vue', `<!-- bench ${nonce} -->\n`),
		append('apps/backend/src/index.ts', `// bench ${nonce}\n`),
	],
	app: (nonce: string) => [rustFn('apps/app/src/main.rs', nonce)],
	core: (nonce: string) => [rustFn('apps/core/src/main.rs', nonce)],
	/** Only for warming the fixtures typecheck, which a contracts edit also runs. */
	fixtures: (nonce: string) => [
		append(
			'packages/fixtures/src/index.ts',
			`// bench ${nonce}
`,
		),
	],
	contracts: (nonce: string) => [
		append(
			'packages/contracts/src/lib.rs',
			`\n/// Benchmark edit.\npub const BENCH_EDIT: u32 = ${nonce};\n`,
		),
	],
} as const
type Edit = keyof typeof EDITS

/** The state the `state` scenario describes: one of everything a fixture makes. */
const STATE = `import { friends, instance, stack } from '@amberite/fixtures'

const { alice, bob } = await stack({
	apps: ['alice', 'bob'],
	cores: [{ owner: 'alice' }, { owner: 'bob' }],
})
await friends(alice, bob)
await instance(alice, {
	name: 'Bench',
	gameVersion: '1.21.1',
	files: { 'config/bench.toml': 'edited = true\\n' },
	sharedWith: [bob],
})
`

type Scenario = {
	/** Seconds this scenario should take. It scores 100 there. */
	readonly target: number
	readonly about: string
	readonly run: (bench: Bench) => Promise<Steps>
}
/** Seconds per step an agent waits on. */
type Steps = Record<string, number>

const SCENARIOS = {
	cold: {
		target: 600,
		about: 'new worktree, nothing compiled anywhere: setup, up',
		run: async (bench) => {
			wipe(NodePath.join(bench.primary, 'target', 'dev-shared'))
			return bench.worktree('cold', [], { check: false })
		},
	},
	ui: {
		target: 90,
		about: 'new worktree, one-line App UI and backend edit: setup, up, check',
		run: (bench) => bench.worktree('ui', ['ui']),
	},
	app: {
		target: 180,
		about: 'new worktree, one-line App Rust edit: setup, up, check',
		run: (bench) => bench.worktree('app', ['app']),
	},
	'app-core': {
		target: 240,
		about: 'new worktree, one-line App and Core Rust edit: setup, up, check',
		run: (bench) => bench.worktree('app-core', ['app', 'core']),
	},
	rebuild: {
		target: 60,
		about: 'running worktree, another one-line App Rust edit: down, up',
		run: (bench) => bench.rebuild(),
	},
	parallel: {
		target: 240,
		about: 'two new worktrees at once, each a one-line App Rust edit: setup, up (slower one)',
		run: (bench) => bench.parallel(),
	},
	state: {
		target: 60,
		about: 'running worktree, a described state applied, then applied again: fixture, fixture',
		run: (bench) => bench.state(),
	},
	contracts: {
		target: 360,
		about: 'new worktree, one-line contracts edit (everything above it rebuilds): setup, up, check',
		run: (bench) => bench.worktree('contracts', ['contracts']),
	},
} satisfies Record<string, Scenario>
type ScenarioName = keyof typeof SCENARIOS
const ORDER = Object.keys(SCENARIOS) as ReadonlyArray<ScenarioName>
/** Fast enough to run after every change. The rest only move when the Rust build itself changes. */
const QUICK: ReadonlyArray<ScenarioName> = ['ui', 'app', 'rebuild', 'state']

type Result = {
	readonly at: string
	readonly snapshot: string
	readonly scenario: ScenarioName
	readonly ok: boolean
	readonly seconds: number
	readonly steps: Steps
	/** Compilers running for other checkouts when it started, which makes the run noisy. */
	readonly busy: number
}

class Bench {
	readonly logs: string
	/** The Node `vp run` gives agents in this checkout, which every step runs on. */
	readonly node: string
	readonly primary: string
	readonly snapshot: string
	readonly worktrees: string
	// Node runs this file without compiling it, so no constructor parameter properties.
	constructor(root: string, primary: string, snapshot: string) {
		this.logs = NodePath.join(root, 'logs')
		this.node = agentNode(primary)
		this.primary = primary
		this.snapshot = snapshot
		this.worktrees = NodePath.join(root, 'wt')
	}

	/**
	 * A new worktree with `edits`, through setup and `up`, then `check:changed` unless told not.
	 * `after` runs next in the same worktree, and only its steps count.
	 */
	async worktree(
		name: string,
		edits: ReadonlyArray<Edit>,
		options: {
			readonly check?: boolean
			readonly after?: (path: string) => Promise<Steps>
		} = {},
	): Promise<Steps> {
		const path = this.addWorktree(name)
		const steps: Steps = {}
		try {
			applyEdits(path, edits)
			steps.setup = await this.step(name, 'setup', path, ['scripts/setup-worktree.ts'])
			steps.up = await this.up(name, path)
			if (options.check !== false) {
				steps.check = await this.step(name, 'check', path, [
					'scripts/check-changed.ts',
					this.snapshot,
				])
			}
			return options.after ? await options.after(path) : steps
		} finally {
			await this.removeWorktree(path)
		}
	}

	/**
	 * Every scenario but `cold` starts from the snapshot already built, tested and typechecked, as
	 * agents keep the shared state of `main` warm in practice. Not timed.
	 */
	async prime(): Promise<void> {
		console.log('[bench] building, testing and typechecking the snapshot (not timed)')
		await this.exec('prime', 'build', this.primary, ['scripts/dev-runner.ts', 'build'])
		// As check:changed does: Tauri only checks that the frontend's build folder exists.
		NodeFS.mkdirSync(NodePath.join(this.primary, 'apps', 'app-frontend', 'dist'), {
			recursive: true,
		})
		await this.exec('prime', 'test', this.primary, [
			'scripts/dev-runner.ts',
			'test',
			'amberite-app',
			'copal',
		])
		await this.worktree('prime', ['ui', 'fixtures'])
	}

	/** The edit-and-restart loop inside a worktree whose stack is already running. */
	rebuild(): Promise<Steps> {
		return this.worktree('rebuild', ['app'], {
			check: false,
			after: async (path) => {
				applyEdits(path, ['app'])
				const down = await this.step('rebuild', 'down', path, ['scripts/dev-runner.ts', 'down'])
				return { down, up: await this.up('rebuild', path) }
			},
		})
	}

	/** An agent putting its stack into the state its change needs, then trying it once more. */
	state(): Promise<Steps> {
		return this.worktree('state', [], {
			check: false,
			after: async (path) => {
				const file = NodePath.join(path, 'packages', 'fixtures', 'fixtures', 'bench.ts')
				NodeFS.writeFileSync(file, STATE)
				const fixture = ['scripts/dev-runner.ts', 'fixture', 'bench']
				return {
					fixture: await this.step('state', 'fixture', path, fixture),
					again: await this.step('state', 'again', path, fixture),
				}
			},
		})
	}

	async parallel(): Promise<Steps> {
		// Both finish before either failure counts, so the next scenario never shares the machine.
		const settled = await Promise.allSettled(
			['parallel-1', 'parallel-2'].map((name) => this.worktree(name, ['app'], { check: false })),
		)
		const [first, second] = settled.map((result) => {
			if (result.status === 'rejected') throw result.reason
			return result.value
		})
		const total = (steps: Steps) => Object.values(steps).reduce((sum, value) => sum + value, 0)
		return total(first!) >= total(second!) ? first! : second!
	}

	/** `up` again while it reports still starting, as an agent does. */
	async up(name: string, path: string): Promise<number> {
		let seconds = 0
		for (;;) {
			const started = performance.now()
			let output: string
			try {
				output = await this.exec(name, 'up', path, ['scripts/dev-runner.ts', 'up'])
			} catch (error) {
				// The default fixture gives up on a build that outlasts its wait and asks for a rerun.
				if (!/still being built|did not become ready/.test(String(error))) throw error
				output = 'Still starting'
			}
			seconds += (performance.now() - started) / 1000
			if (!output.includes('Still starting')) return seconds
		}
	}

	async step(
		name: string,
		step: string,
		cwd: string,
		args: ReadonlyArray<string>,
	): Promise<number> {
		const started = performance.now()
		await this.exec(name, step, cwd, args)
		return (performance.now() - started) / 1000
	}

	/** Runs `node <args>` in `cwd`, logging its output. Throws when it fails. */
	async exec(
		name: string,
		step: string,
		cwd: string,
		args: ReadonlyArray<string>,
	): Promise<string> {
		NodeFS.mkdirSync(this.logs, { recursive: true })
		const log = NodePath.join(this.logs, `${name}.log`)
		NodeFS.appendFileSync(log, `\n=== ${step}: node ${args.join(' ')}\n`)
		const child = NodeChildProcess.spawn(this.node, [...args], {
			cwd,
			stdio: ['ignore', 'pipe', 'pipe'],
			windowsHide: true,
		})
		let output = ''
		const collect = (chunk: Buffer) => {
			output += chunk.toString()
			NodeFS.appendFileSync(log, chunk)
		}
		child.stdout.on('data', collect)
		child.stderr.on('data', collect)
		const code = await new Promise<number>((resolve) => {
			child.once('error', () => resolve(1))
			child.once('close', (exit) => resolve(exit ?? 1))
		})
		if (code !== 0) {
			const tail = output.trimEnd().split('\n').slice(-5).join('\n')
			throw new Error(`${name} ${step} failed (exit ${code}). See ${log}\n${tail}`)
		}
		return output
	}

	addWorktree(name: string): string {
		let path = NodePath.join(this.worktrees, name)
		// A run that was interrupted can leave its stack running there. One that will not stop
		// keeps its folder, and this run takes a new one.
		const stopped =
			!NodeFS.existsSync(NodePath.join(path, '.data', 'runtime.json')) ||
			NodeChildProcess.spawnSync(this.node, ['scripts/dev-runner.ts', 'down'], {
				cwd: path,
				stdio: 'ignore',
				windowsHide: true,
			}).status === 0
		try {
			if (!stopped) throw new Error('still running')
			wipe(path)
		} catch {
			// Still held by a process from an earlier run.
			path = `${path}-${Date.now()}`
		}
		git(['worktree', 'prune'], this.primary)
		git(['worktree', 'add', '--detach', '--force', path, this.snapshot], this.primary)
		return path
	}

	async removeWorktree(path: string): Promise<void> {
		if (!NodeFS.existsSync(path)) return
		try {
			await this.exec(NodePath.basename(path), 'down', path, ['scripts/dev-runner.ts', 'down'])
		} catch {
			// Nothing was running.
		}
		const devLog = NodePath.join(path, '.data', 'dev.log')
		if (NodeFS.existsSync(devLog)) {
			NodeFS.copyFileSync(devLog, NodePath.join(this.logs, `${NodePath.basename(path)}-dev.log`))
		}
		try {
			wipe(path)
		} catch (error) {
			// A process that just stopped can hold files a while; the next run replaces the folder.
			console.error(`[bench] could not delete ${path}: ${String(error)}`)
		}
		git(['worktree', 'prune'], this.primary)
	}
}

async function main(): Promise<void> {
	const args = process.argv.slice(2)
	const requested = args.filter((arg) => arg !== '--full')
	const unknown = requested.filter((name) => !ORDER.includes(name as ScenarioName))
	if (unknown.length > 0) {
		throw new Error(`Unknown scenario ${unknown.join(', ')}. Scenarios: ${ORDER.join(', ')}`)
	}
	const wanted = args.includes('--full') ? ORDER : requested.length > 0 ? requested : QUICK
	const selected = ORDER.filter((name) => wanted.includes(name))
	const checkout = git(['rev-parse', '--show-toplevel'], process.cwd())
	const realPrimary = NodePath.resolve(
		git(['worktree', 'list', '--porcelain'], checkout).split(/\r?\n/)[0]!.slice('worktree '.length),
	)
	const root = NodePath.resolve(process.env.BENCH_ROOT ?? `${realPrimary}-bench`)
	const snapshot = snapshotCommit(checkout)
	const primary = preparePrimary(root, realPrimary, snapshot)
	const bench = new Bench(root, primary, snapshot)
	// Logs of the scenarios not run this time stay, for comparing.
	for (const log of NodeFS.existsSync(bench.logs) ? NodeFS.readdirSync(bench.logs) : []) {
		const name = log.replace(/(?:-dev)?\.log$/, '').replace(/^parallel-\d$/, 'parallel')
		if (name === 'prime' || selected.some((scenario) => scenario === name)) {
			NodeFS.rmSync(NodePath.join(bench.logs, log))
		}
	}
	console.log(`[bench] ${snapshot.slice(0, 9)} in ${root} on ${bench.node}`)

	if (!selected.includes('cold')) await bench.prime()

	const resultsFile = NodePath.join(root, 'results.jsonl')
	for (const scenario of selected) {
		const busy = otherCompilers()
		console.log(`[bench] ${scenario}: ${SCENARIOS[scenario].about}`)
		const started = performance.now()
		let steps: Steps = {}
		let ok = true
		try {
			steps = await SCENARIOS[scenario].run(bench)
		} catch (error) {
			ok = false
			console.error(`[bench] ${error instanceof Error ? error.message : String(error)}`)
		}
		const seconds = ok
			? Object.values(steps).reduce((sum, value) => sum + value, 0)
			: (performance.now() - started) / 1000
		const result: Result = {
			at: new Date().toISOString(),
			snapshot,
			scenario,
			ok,
			seconds: round(seconds),
			steps: Object.fromEntries(Object.entries(steps).map(([step, value]) => [step, round(value)])),
			busy,
		}
		NodeFS.appendFileSync(resultsFile, `${JSON.stringify(result)}\n`)
		console.log(`[bench]   ${formatResult(result)}`)
		// The rest start warm. A failure here stops them, since they would measure the warmup.
		if (scenario === 'cold' && scenario !== selected.at(-1)) await bench.prime()
	}
	printReport(resultsFile, snapshot)
}

/** The latest result of every scenario: this run's, or for scenarios it skipped, earlier ones. */
function printReport(resultsFile: string, snapshot: string): void {
	const latest = new Map<ScenarioName, Result>()
	for (const line of NodeFS.readFileSync(resultsFile, 'utf8').split('\n').filter(Boolean)) {
		const result = JSON.parse(line) as Result
		if (result.scenario in SCENARIOS) latest.set(result.scenario, result)
	}
	console.log('\n[bench] scenario     score  seconds  steps')
	let inverseSum = 0
	for (const name of ORDER) {
		const result = latest.get(name)
		if (!result) {
			console.log(`[bench] ${name.padEnd(11)}      -  (never run)`)
			inverseSum = Number.POSITIVE_INFINITY
			continue
		}
		const score = result.ok ? scoreOf(name, result.seconds) : 0
		inverseSum += 1 / score
		const stale = result.snapshot === snapshot ? '' : `  (from ${result.snapshot.slice(0, 9)})`
		console.log(
			`[bench] ${name.padEnd(11)} ${String(Math.round(score)).padStart(5)}  ${formatResult(result)}${stale}`,
		)
	}
	const overall = ORDER.length / inverseSum
	console.log(`[bench] overall     ${String(Math.round(overall)).padStart(5)}`)
}

function scoreOf(name: ScenarioName, seconds: number): number {
	return (100 * SCENARIOS[name].target) / Math.max(seconds, 1)
}

function formatResult(result: Result): string {
	const steps = Object.entries(result.steps)
		.map(([step, value]) => `${step} ${value}`)
		.join(', ')
	const busy = result.busy > 0 ? `  busy ${result.busy}` : ''
	return `${String(result.seconds).padStart(7)}  ${result.ok ? steps : 'FAILED'}${busy}`
}

/**
 * This checkout's files as a commit, uncommitted and untracked ones included, without touching its
 * index or stash. Worktrees share objects, so the sandbox clone can check it out.
 */
function snapshotCommit(checkout: string): string {
	const index = NodePath.join(NodeOS.tmpdir(), `dev-bench-index-${process.pid}`)
	const env = { ...process.env, GIT_INDEX_FILE: index }
	try {
		git(['read-tree', 'HEAD'], checkout, env)
		git(['add', '-A'], checkout, env)
		const tree = git(['write-tree'], checkout, env)
		return git(['commit-tree', tree, '-p', 'HEAD', '-m', 'dev-bench snapshot'], checkout, env)
	} finally {
		NodeFS.rmSync(index, { force: true })
	}
}

/** The sandbox's primary checkout at `snapshot`, with the real primary's engine settings. */
function preparePrimary(root: string, realPrimary: string, snapshot: string): string {
	const primary = NodePath.join(root, 'primary')
	if (!NodeFS.existsSync(NodePath.join(primary, '.git'))) {
		NodeFS.mkdirSync(root, { recursive: true })
		git(['clone', '--shared', '--no-checkout', realPrimary, primary], root)
	}
	git(['checkout', '--detach', '--force', snapshot], primary)
	git(['worktree', 'prune'], primary)
	// The engine's settings; a primary still on an older layout keeps them at app-lib.
	const env = ['packages/engine/.env', 'packages/app-lib/.env']
		.map((file) => NodePath.join(realPrimary, file))
		.find((file) => NodeFS.existsSync(file))
	if (env) NodeFS.copyFileSync(env, NodePath.join(primary, 'packages', 'engine', '.env'))
	return primary
}

function applyEdits(worktree: string, edits: ReadonlyArray<Edit>): void {
	const nonce = String(Date.now() % 1_000_000_000)
	for (const edit of edits) for (const apply of EDITS[edit](nonce)) apply(worktree)
}

function append(file: string, text: string): (worktree: string) => void {
	return (worktree) => NodeFS.appendFileSync(NodePath.join(worktree, file), text)
}

/** A one-line change to a Rust crate, formatted as rustfmt wants it. */
function rustFn(file: string, nonce: string): (worktree: string) => void {
	return append(file, `\n#[allow(dead_code)]\nfn bench_edit_${nonce}() -> u32 {\n    ${nonce}\n}\n`)
}

/**
 * The Node `vp run` uses in `checkout`, which can differ from the one running this script: Node
 * 24.15.0, for one, crashes now and then on Windows with 0xC0000409 while it makes HTTP requests.
 */
function agentNode(checkout: string): string {
	// Through the shell, which also finds vp.cmd, what an npm install of vp puts on PATH.
	const result = NodeChildProcess.spawnSync('vp env which node', {
		cwd: checkout,
		encoding: 'utf8',
		shell: true,
		windowsHide: true,
	})
	const path = result.status === 0 ? result.stdout.split('\n')[0]!.trim() : ''
	return path && NodeFS.existsSync(path) ? path : process.execPath
}

/** rustc processes running now, all of which belong to other checkouts when a scenario starts. */
function otherCompilers(): number {
	if (process.platform !== 'win32') return 0
	const result = NodeChildProcess.spawnSync('tasklist', ['/FI', 'IMAGENAME eq rustc.exe', '/NH'], {
		encoding: 'utf8',
		windowsHide: true,
	})
	return result.stdout.split('\n').filter((line) => line.startsWith('rustc.exe')).length
}

/** Deletes a folder, retrying while Windows still holds files of a process that just stopped. */
function wipe(path: string): void {
	NodeFS.rmSync(path, { recursive: true, force: true, maxRetries: 10, retryDelay: 500 })
}

function round(seconds: number): number {
	return Math.round(seconds * 10) / 10
}

function git(args: ReadonlyArray<string>, cwd: string, env?: NodeJS.ProcessEnv): string {
	const result = NodeChildProcess.spawnSync('git', args, {
		cwd,
		encoding: 'utf8',
		env,
		maxBuffer: 64 * 1024 * 1024,
		windowsHide: true,
	})
	if (result.status !== 0) throw new Error(result.stderr.trim() || `git ${args.join(' ')} failed`)
	return result.stdout.trim()
}

main().catch((error: unknown) => {
	console.error(`[bench] ${error instanceof Error ? error.message : String(error)}`)
	process.exitCode = 1
})
