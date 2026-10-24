#!/usr/bin/env node

import * as NodeChildProcess from 'node:child_process'
import * as NodeCrypto from 'node:crypto'
import * as NodeFS from 'node:fs'
import * as NodeNet from 'node:net'
import * as NodePath from 'node:path'
import * as NodeReadline from 'node:readline'
import * as NodeURL from 'node:url'
import * as NodeUtil from 'node:util'
import { normalizeScenarioDatabase, runningAppCommands } from './scenario-state.ts'

const BASE_PORTS = {
	app: 1420,
	backend: 8787,
	accountWeb: 3100,
	core: 16662,
} as const
const MAX_HASH_OFFSET = 3000
const MAX_PORT = 65_535
const RESTART_FAILURE_LIMIT = 3
const RESTART_FAILURE_WINDOW_MS = 10_000
const PORT_PROBE_HOSTS = ['127.0.0.1', '::1'] as const
// Stays under the ten minute limit agent shells put on a foreground command.
const UP_WAIT_MS = 9 * 60_000
// One cached program per distinct set of Rust inputs. Older ones are deleted.
const CACHED_PROGRAMS = 6
const BUILD_LOCK_STALE_MS = 60 * 60_000
const BUILD_LOCK = 'building'
const BRIDGE_READY = /browser bridge ready.*?url=(http:\/\/[\d.]+:\d+)/
const FETCH_BAD_PORTS = new Set([
	0, 1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95, 101, 102,
	103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143, 161, 179, 389, 427, 465,
	512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563, 587, 601, 636, 989, 990, 993,
	995, 1719, 1720, 1723, 2049, 3659, 4045, 4190, 5060, 5061, 6000, 6566, 6665, 6666, 6667, 6668,
	6669, 6679, 6697, 10080,
])

export const DEV_MODES = ['dev', 'dev:app', 'dev:backend', 'dev:core'] as const
const RUNNER_FLAGS = ['--dry-run', '--no-account-web', '--hide-windows']
export type DevMode = (typeof DEV_MODES)[number]
export type DevPorts = {
	readonly app: number
	readonly backend: number
	readonly accountWeb: number
	readonly core: number
}

type PortName = keyof DevPorts
type PortAvailabilityCheck = (port: number, hosts: ReadonlyArray<string>) => Promise<boolean>
export type WorktreePaths = {
	readonly coreData: string
	readonly data: string
	readonly primary: string
	readonly runtime: string
	readonly scenariosData: string
	readonly worktree: string
}
type RunnerInput = {
	/** The account sign-in website. Only a person signing in by hand needs it. */
	readonly accountWeb: boolean
	readonly dryRun: boolean
	/** Apps run without a window on the desktop and are used through the browser bridge. */
	readonly hideWindows: boolean
	readonly mode: DevMode
	readonly scenarios: ReadonlyArray<number>
}
/**
 * A Rust program the runner builds itself. It is built once per distinct set of inputs, copied
 * into `<targetDir>/dev-bin/<key>/`, and run from that copy. A checkout whose Rust matches
 * something already built starts without Cargo, and a running program never blocks a build.
 */
export type ProgramBuild = {
	readonly cargoArgs: ReadonlyArray<string>
	/** Where Cargo runs. It takes the toolchain and `.cargo` settings from here. */
	readonly cwd: string
	/** Settings the build reads. They change the program, so they are part of its key. */
	readonly env: Record<string, string>
	/** Gitignored files, as absolute paths, that the build reads. */
	readonly ignoredInputs: ReadonlyArray<string>
	/** Files and folders, relative to the checkout, that the build reads. */
	readonly inputs: ReadonlyArray<string>
	/** The file Cargo writes in `<targetDir>/debug`. */
	readonly program: string
	readonly targetDir: string
}
export type ProcessSpec = {
	readonly args: ReadonlyArray<string>
	/** Set when the process is a Rust program. Otherwise the arguments go to `vp`. */
	readonly build?: ProgramBuild
	readonly cwd: string
	readonly env: NodeJS.ProcessEnv
	readonly label: string
}
type RunningProcess = {
	readonly child: NodeChildProcess.ChildProcess
	readonly done: Promise<number>
	readonly label: string
}
type LogLevel = 'error' | 'info' | 'warning'
/** .data/runtime.json: what the running (or last) dev runner started and where to reach it. */
type RuntimeFile = {
	/** The dev runner process. Absent once it has stopped. */
	pid?: number
	readonly branch: string
	readonly dataDir: string
	readonly mode: DevMode
	/** False when the account sign-in website was left out, so nothing waits for its port. */
	readonly accountWeb: boolean
	/** A process whose Rust build failed. `up` stops waiting when it sees one. */
	buildFailed?: string
	/** Whether the Apps were started without a window. `up --show-windows` cannot change it later. */
	readonly hideWindows: boolean
	readonly ports: DevPorts
	readonly scenarios: Array<{
		readonly dataDir: string
		readonly number: number
		readonly username: string
		/** Serves this scenario's real App to a browser. Set once the App has started. */
		bridgeUrl?: string
	}>
	readonly source: string
	readonly urls: Record<PortName, string>
}

export class DevRunnerError extends Error {
	constructor(message: string) {
		super(message)
		this.name = 'DevRunnerError'
	}
}

export function scenarioUsername(scenario: number): string {
	return `scenario_${scenario}`
}

export function isBrowserAllowedPort(port: number): boolean {
	return !FETCH_BAD_PORTS.has(port)
}

export function portsForOffset(offset: number): DevPorts {
	return {
		app: BASE_PORTS.app + offset,
		backend: BASE_PORTS.backend + offset,
		accountWeb: BASE_PORTS.accountWeb + offset,
		core: BASE_PORTS.core + offset,
	}
}

export function resolveStartOffset(input: {
	readonly devInstance?: string
	readonly explicitOffset?: string
	readonly primaryPath: string
	readonly worktreePath: string
}): { readonly offset: number; readonly source: string } {
	const explicit = input.explicitOffset?.trim()
	if (explicit) {
		if (!/^\d+$/.test(explicit)) {
			throw new DevRunnerError(
				`AMBERITE_PORT_OFFSET must be a non-negative integer; received ${explicit}.`,
			)
		}
		return { offset: Number(explicit), source: `AMBERITE_PORT_OFFSET=${explicit}` }
	}

	const instance = input.devInstance?.trim()
	if (instance) {
		if (/^\d+$/.test(instance)) {
			return { offset: Number(instance), source: `numeric AMBERITE_DEV_INSTANCE=${instance}` }
		}
		return {
			offset: (stableHash(instance) % MAX_HASH_OFFSET) + 1,
			source: `hashed AMBERITE_DEV_INSTANCE=${instance}`,
		}
	}

	if (samePath(input.primaryPath, input.worktreePath)) {
		return { offset: 0, source: 'primary checkout' }
	}

	return {
		offset: (stableHash(input.worktreePath) % MAX_HASH_OFFSET) + 1,
		source: `worktree ${input.worktreePath}`,
	}
}

export async function findFirstAvailableOffset(input: {
	readonly accountWeb?: boolean
	readonly checkPort?: PortAvailabilityCheck
	readonly mode: DevMode
	readonly startOffset: number
}): Promise<number> {
	const requiredPorts = requiredPortNames(input.mode, input.accountWeb)
	const checkPort = input.checkPort ?? portIsAvailable

	for (let offset = input.startOffset; offset <= MAX_PORT; offset += 1) {
		const ports = portsForOffset(offset)
		if (requiredPorts.some((name) => ports[name] > MAX_PORT)) break
		if (requiredPorts.some((name) => !isBrowserAllowedPort(ports[name]))) continue

		const available = await Promise.all(
			requiredPorts.map((name) => checkPort(ports[name], PORT_PROBE_HOSTS)),
		)
		if (available.every(Boolean)) return offset
	}

	throw new DevRunnerError(`No development ports are available from offset ${input.startOffset}.`)
}

export function createRuntimeEnvironment(input: {
	readonly baseEnv: NodeJS.ProcessEnv
	readonly paths: WorktreePaths
	readonly ports: DevPorts
}): NodeJS.ProcessEnv {
	const env: NodeJS.ProcessEnv = { ...input.baseEnv }
	delete env.HOST
	delete env.PORT
	// The dev Core reports the address it listens on, never one left in an env file.
	delete env.AMBERITE_PUBLIC_URL
	env.ACCOUNT_API_URL = `http://127.0.0.1:${input.ports.backend}`
	env.VITE_ACCOUNT_API_URL = env.ACCOUNT_API_URL
	env.ACCOUNT_WEB_URL = `http://127.0.0.1:${input.ports.accountWeb}`
	return {
		...env,
		AMBERITE_DATA_DIR: input.paths.data,
		AMBERITE_DEV_MODE: 'true',
		AMBERITE_LOCAL_CORE_DATA_DIR: input.paths.coreData,
	}
}

/** The browser bridge URL an App prints once it is usable, if this log line is that one. */
export function bridgeUrlFromLine(line: string): string | undefined {
	return BRIDGE_READY.exec(line)?.[1]
}

export function processLabelsForMode(mode: DevMode, accountWeb = true): ReadonlyArray<string> {
	const labels = {
		dev: ['backend', 'account-web', 'core', 'app-frontend'],
		'dev:backend': ['backend', 'account-web'],
		'dev:app': ['app-frontend'],
		'dev:core': ['core'],
	}[mode]
	return accountWeb ? labels : labels.filter((label) => label !== 'account-web')
}

async function main(): Promise<void> {
	const paths = resolveWorktreePaths()
	const defaultScenarios = readDefaultScenarios(NodePath.join(paths.worktree, 'dev.json'))
	const [command, ...commandArgs] = process.argv.slice(2)
	if (command === 'up') {
		const args = commandArgs.filter((arg) => arg !== '--show-windows')
		// The mode is optional here: `up 1 2` means the full stack with those scenarios.
		const hasMode = DEV_MODES.some((mode) => mode === args[0])
		return up(paths, {
			...parseInput(hasMode ? args : ['dev', ...args], defaultScenarios),
			// Apps sign themselves in and are used through the browser bridge.
			accountWeb: false,
			hideWindows: !commandArgs.includes('--show-windows'),
		})
	}
	if (command === 'down') return down(paths)
	if (command === 'status') {
		if (!(await printStatus(paths))) process.exitCode = 1
		return
	}
	const input = parseInput(process.argv.slice(2), defaultScenarios)
	const { offset: startOffset, source } = resolveStartOffset({
		devInstance: process.env.AMBERITE_DEV_INSTANCE,
		explicitOffset: process.env.AMBERITE_PORT_OFFSET,
		primaryPath: paths.primary,
		worktreePath: paths.worktree,
	})
	const selectedOffset = await findFirstAvailableOffset({
		accountWeb: input.accountWeb,
		mode: input.mode,
		startOffset,
	})
	const ports = portsForOffset(selectedOffset)
	const branch = currentBranch(paths.worktree)
	const sharedEnv = {
		...readEnv(NodePath.join(paths.worktree, 'packages', 'app-lib', '.env.prod')),
		...readEnv(NodePath.join(paths.worktree, 'packages', 'app-lib', '.env')),
		...readEnv(NodePath.join(paths.primary, '.env.local')),
		...readEnv(NodePath.join(paths.primary, 'apps', 'core', '.env.local')),
		...readEnv(NodePath.join(paths.worktree, '.env.local')),
		...process.env,
	}
	const env = createRuntimeEnvironment({ baseEnv: sharedEnv, paths, ports })
	if (!input.dryRun) {
		const secretPath = NodePath.join(paths.data, 'backend', 'dev-secret')
		NodeFS.mkdirSync(NodePath.dirname(secretPath), { recursive: true })
		if (!NodeFS.existsSync(secretPath))
			NodeFS.writeFileSync(secretPath, NodeCrypto.randomBytes(32).toString('hex'), { mode: 0o600 })
		env.AMBERITE_LOCAL_DEV_SECRET = NodeFS.readFileSync(secretPath, 'utf8').trim()
		if (processLabelsForMode(input.mode).includes('backend')) {
			prepareLocalBackend(paths, env)
		}
	}
	const specs = createProcessSpecs({
		accountWeb: input.accountWeb,
		branch,
		env,
		hideWindows: input.hideWindows,
		mode: input.mode,
		paths,
		ports,
		scenarios: input.scenarios,
	})
	printPlan({
		branch,
		mode: input.mode,
		paths,
		ports,
		scenarios: input.scenarios,
		source,
		specs,
	})
	if (input.dryRun) return

	ensureDataLayout(paths, input.scenarios)
	const runtime: RuntimeFile = {
		pid: process.pid,
		branch,
		dataDir: paths.data,
		mode: input.mode,
		accountWeb: input.accountWeb,
		hideWindows: input.hideWindows,
		ports,
		scenarios: input.scenarios.map((scenario) => ({
			dataDir: scenarioDataPath(paths, scenario),
			number: scenario,
			username: scenarioUsername(scenario),
		})),
		source,
		urls: {
			backend: `http://127.0.0.1:${ports.backend}`,
			accountWeb: `http://127.0.0.1:${ports.accountWeb}`,
			app: `http://localhost:${ports.app}`,
			core: `http://127.0.0.1:${ports.core}`,
		},
	}
	writeRuntimeFile(paths, runtime)
	const setBridgeUrl = (label: string, bridgeUrl: string | undefined) => {
		const scenario = runtime.scenarios.find((entry) => `app:${entry.number}` === label)
		if (!scenario || scenario.bridgeUrl === bridgeUrl) return
		scenario.bridgeUrl = bridgeUrl
		writeRuntimeFile(paths, runtime)
	}
	const setBuildFailed = (label: string, failed: boolean) => {
		if ((runtime.buildFailed === label) === failed) return
		runtime.buildFailed = failed ? label : undefined
		writeRuntimeFile(paths, runtime)
	}
	const specsByLabel = new Map(specs.map((spec) => [spec.label, spec]))
	const processes = new Map<string, RunningProcess>()
	/** The latest start of each process. A build that finishes after a newer start is dropped. */
	const starts = new Map<string, symbol>()
	const builds = new Set<NodeChildProcess.ChildProcess>()
	const restartFailures = new Map<string, number[]>()
	let stopping = false
	let finish: (() => void) | undefined
	const finished = new Promise<void>((resolve) => {
		finish = resolve
	})

	const stop = async (exitCode: number) => {
		if (stopping) return
		stopping = true
		commands.close()
		for (const build of builds) stopChild(build)
		for (const running of processes.values()) stopChild(running.child)
		processes.clear()
		writeRuntimeFile(paths, stoppedRuntime(runtime))
		process.exitCode = exitCode
		finish?.()
	}
	const start = async (spec: ProcessSpec) => {
		const token = Symbol(spec.label)
		starts.set(spec.label, token)
		setBridgeUrl(spec.label, undefined)
		let program: string | undefined
		if (spec.build) {
			setBuildFailed(spec.label, false)
			program = await ensureProgram(paths.worktree, spec, spec.build, builds)
			if (stopping || starts.get(spec.label) !== token) return
			if (!program) {
				setBuildFailed(spec.label, true)
				const command = spec.label === 'core' ? 'rs core' : `rs ${spec.label.slice(4)}`
				runnerLog('error', `${spec.label} did not build. Use ${command} after fixing it.`)
				return
			}
		}
		if (spec.label.startsWith('app:')) {
			// The App loads its page once, so the frontend has to be serving before it starts.
			while (!(await portIsOpen(ports.app))) {
				await new Promise((resolve) => setTimeout(resolve, 250))
				if (stopping || starts.get(spec.label) !== token) return
			}
			const dataDir = spec.env.THESEUS_CONFIG_DIR
			if (!dataDir) throw new DevRunnerError('App scenario is missing its data directory.')
			normalizeScenarioDatabase(paths.worktree, dataDir, runningAppCommands())
		}
		const running = spawnProcess(paths.worktree, spec, program, (line) => {
			const bridgeUrl = bridgeUrlFromLine(line)
			if (!bridgeUrl) return
			setBridgeUrl(spec.label, bridgeUrl)
			runnerLog('info', `${spec.label} ready ${bridgeUrl}`)
		})
		processes.set(spec.label, running)
		void running.done.then(
			(code) => handleProcessExit(spec, running, code),
			(error: unknown) => {
				runnerLog(
					'error',
					`${spec.label} failed: ${error instanceof Error ? error.message : String(error)}`,
				)
				void handleProcessExit(spec, running, 1)
			},
		)
	}
	const startOrStop = (spec: ProcessSpec) =>
		start(spec).catch((error: unknown) => {
			runnerLog(
				'error',
				`${spec.label} failed: ${error instanceof Error ? error.message : String(error)}`,
			)
			return stop(1)
		})
	const restart = async (label: string) => {
		const spec = specsByLabel.get(label)
		if (!spec || (label !== 'core' && !label.startsWith('app:'))) {
			runnerLog('warning', `Unknown restart target ${label}. Use rs <scenario> or rs core.`)
			return
		}
		const current = processes.get(label)
		if (current) {
			processes.delete(label)
			stopChild(current.child)
		}
		restartFailures.delete(label)
		runnerLog('info', `Restarting ${label}...`)
		await startOrStop(spec)
	}
	const handleProcessExit = async (spec: ProcessSpec, running: RunningProcess, code: number) => {
		if (processes.get(spec.label) !== running) return
		processes.delete(spec.label)
		if (stopping) return

		if (spec.label === 'core' || (spec.label.startsWith('app:') && code !== 0)) {
			const now = Date.now()
			const failures = [...(restartFailures.get(spec.label) ?? []), now].filter(
				(timestamp) => now - timestamp < RESTART_FAILURE_WINDOW_MS,
			)
			restartFailures.set(spec.label, failures)
			if (failures.length >= RESTART_FAILURE_LIMIT) {
				const command = spec.label === 'core' ? 'rs core' : `rs ${spec.label.slice(4)}`
				runnerLog(
					'error',
					`${spec.label} failed ${RESTART_FAILURE_LIMIT} times in 10 seconds; automatic restarts paused. Use ${command} after fixing it.`,
				)
				return
			}
			runnerLog('warning', `${spec.label} exited with code ${code}; restarting...`)
			await new Promise((resolve) => setTimeout(resolve, 750))
			if (!stopping && !processes.has(spec.label)) await startOrStop(spec)
			return
		}
		if (spec.label.startsWith('app:')) {
			runnerLog('info', `${spec.label} closed. Use rs ${spec.label.slice(4)} to start it again.`)
			return
		}

		runnerLog('error', `${spec.label} exited with code ${code}.`)
		await stop(code || 1)
	}
	const commands = createCommandInput({
		commandFile: NodePath.join(paths.data, 'dev-commands'),
		getCore: () => processes.get('core'),
		restart: (target) => restart(target === 'core' ? target : `app:${target}`),
		stop: () => stop(0),
	})

	process.once('SIGINT', () => void stop(130))
	process.once('SIGTERM', () => void stop(143))

	try {
		for (const spec of specs) void startOrStop(spec)
		await finished
	} finally {
		await stop(process.exitCode || 1)
	}
}

export function parseInput(
	args: ReadonlyArray<string>,
	defaultScenarios: ReadonlyArray<number> = [1],
): RunnerInput {
	const modeValue = args.find((arg) => !arg.startsWith('-')) ?? 'dev'
	if (!DEV_MODES.includes(modeValue as DevMode)) {
		throw new DevRunnerError(
			`Unknown development mode ${modeValue}. Expected ${DEV_MODES.join(', ')}.`,
		)
	}

	const unknown = args.find((arg) => arg.startsWith('--') && !RUNNER_FLAGS.includes(arg))
	if (unknown) throw new DevRunnerError(`Unknown option ${unknown}.`)
	const scenarioArgs = args.filter((arg) => arg !== modeValue && !arg.startsWith('--'))
	const startsApps = modeValue === 'dev' || modeValue === 'dev:app'
	if (!startsApps && scenarioArgs.length > 0) {
		throw new DevRunnerError(`${modeValue} does not start App scenarios.`)
	}
	const scenarios = startsApps
		? normalizeScenarios(scenarioArgs.length > 0 ? scenarioArgs : defaultScenarios)
		: []

	return {
		accountWeb: !args.includes('--no-account-web'),
		dryRun: args.includes('--dry-run'),
		hideWindows: args.includes('--hide-windows'),
		mode: modeValue as DevMode,
		scenarios,
	}
}

export function readDefaultScenarios(path: string): ReadonlyArray<number> {
	if (!NodeFS.existsSync(path)) return [1]
	let value: unknown
	try {
		value = JSON.parse(NodeFS.readFileSync(path, 'utf8'))
	} catch (error) {
		throw new DevRunnerError(`Could not read ${path}: ${String(error)}`)
	}
	if (!isRecord(value) || !Array.isArray(value.defaultScenarios)) {
		throw new DevRunnerError(`${path} must contain a defaultScenarios array.`)
	}
	return normalizeScenarios(value.defaultScenarios)
}

function normalizeScenarios(values: ReadonlyArray<unknown>): ReadonlyArray<number> {
	const scenarios = values.map((value) => {
		const scenario = typeof value === 'number' ? value : Number(value)
		if (!Number.isSafeInteger(scenario) || scenario < 1 || scenario > 999_999_999_999) {
			throw new DevRunnerError(`Scenario ${String(value)} must be a positive whole number.`)
		}
		return scenario
	})
	if (scenarios.length === 0) throw new DevRunnerError('At least one App scenario is required.')
	return [...new Set(scenarios)]
}

function resolveWorktreePaths(): WorktreePaths {
	const worktree = git(['rev-parse', '--show-toplevel'], process.cwd())
	const worktreeList = git(['worktree', 'list', '--porcelain'], worktree)
	const primary =
		worktreeList
			.split(/\r?\n/)
			.find((line) => line.startsWith('worktree '))
			?.slice('worktree '.length) ?? worktree
	const data = NodePath.join(worktree, '.data')

	return {
		coreData: NodePath.join(data, 'core'),
		data,
		primary: NodePath.resolve(primary),
		runtime: NodePath.join(data, 'runtime.json'),
		scenariosData: NodePath.join(data, 'scenarios'),
		worktree: NodePath.resolve(worktree),
	}
}

function ensureDataLayout(paths: WorktreePaths, scenarios: ReadonlyArray<number>): void {
	NodeFS.mkdirSync(paths.data, { recursive: true })
	NodeFS.mkdirSync(paths.coreData, { recursive: true })
	for (const scenario of new Set([1, 2, 3, 4, ...scenarios])) {
		NodeFS.mkdirSync(scenarioDataPath(paths, scenario), { recursive: true })
	}
}

export function createProcessSpecs(input: {
	readonly accountWeb: boolean
	readonly branch: string
	readonly env: NodeJS.ProcessEnv
	readonly hideWindows: boolean
	readonly mode: DevMode
	readonly paths: WorktreePaths
	readonly ports: DevPorts
	readonly scenarios: ReadonlyArray<number>
}): ReadonlyArray<ProcessSpec> {
	const labels = processLabelsForMode(input.mode, input.accountWeb)
	const backendUrl = `http://127.0.0.1:${input.ports.backend}`
	const coreDir = NodePath.join(input.paths.worktree, 'apps', 'core')
	// Core is its own Cargo workspace, so it has its own build output.
	const coreTargetDir =
		input.env.CARGO_TARGET_DIR ?? NodePath.join(input.paths.primary, 'apps', 'core', 'target')
	// Crates Core compiles from outside its own folder.
	const coreManifest = NodeFS.readFileSync(NodePath.join(coreDir, 'Cargo.toml'), 'utf8')
	const coreCrates = [...coreManifest.matchAll(/\bpath = "([^"]+)"/g)].map((match) =>
		NodePath.posix.join('apps/core', match[1]!),
	)
	const coreEnv = {
		...input.env,
		ALLOWED_ORIGIN: `http://localhost:${input.ports.app}`,
		AMBERITE_BIND_HOST: '127.0.0.1',
		CORE_DATA_DIR: input.paths.coreData,
		PORT: String(input.ports.core),
	}
	const specs: Record<string, ProcessSpec> = {
		backend: {
			args: [
				'exec',
				'wrangler',
				'dev',
				'--local',
				'--ip',
				'127.0.0.1',
				'--port',
				String(input.ports.backend),
				'--persist-to',
				NodePath.join(input.paths.data, 'backend'),
				'--env-file',
				NodePath.join(input.paths.data, 'backend', '.dev.vars'),
				'--assets',
				NodePath.join(input.paths.data, 'backend', 'assets'),
			],
			cwd: NodePath.join(input.paths.worktree, 'apps', 'backend'),
			env: input.env,
			label: 'backend',
		},
		'account-web': {
			args: [
				'exec',
				'nuxi',
				'dev',
				'--host',
				'127.0.0.1',
				'--port',
				String(input.ports.accountWeb),
			],
			cwd: NodePath.join(input.paths.worktree, 'apps', 'frontend'),
			// The website is upstream Modrinth; these are its own settings for which API to call.
			env: {
				...input.env,
				BASE_URL: `${backendUrl}/v2/`,
				BROWSER_BASE_URL: `${backendUrl}/v2/`,
				PORT: String(input.ports.accountWeb),
			},
			label: 'account-web',
		},
		'app-frontend': {
			args: ['dev', '--port', String(input.ports.app), '--strictPort'],
			cwd: NodePath.join(input.paths.worktree, 'apps', 'app-frontend'),
			env: input.env,
			label: 'app-frontend',
		},
		core: {
			args: [],
			build: {
				cargoArgs: ['build'],
				cwd: coreDir,
				env: {},
				ignoredInputs: [],
				// Cargo also applies the `.cargo` settings of the folders above Core, and the crates
				// outside Core take their edition, dependency versions and lints from the root manifest.
				inputs: ['apps/core', '.cargo', 'Cargo.toml', ...new Set(coreCrates)],
				program: programFile('copal'),
				targetDir: coreTargetDir,
			},
			cwd: coreDir,
			env: coreEnv,
			label: 'core',
		},
	}
	const shared = labels.map((label) => specs[label]!)
	if (input.mode !== 'dev' && input.mode !== 'dev:app') return shared
	return [
		...shared,
		...input.scenarios.map((scenario) =>
			createAppProcessSpec({
				branch: input.branch,
				backendUrl,
				devUrl: `http://localhost:${input.ports.app}`,
				env: input.env,
				hideWindows: input.hideWindows,
				paths: input.paths,
				scenario,
			}),
		),
	]
}

function createAppProcessSpec(input: {
	readonly backendUrl: string
	readonly branch: string
	readonly devUrl: string
	readonly env: NodeJS.ProcessEnv
	readonly hideWindows: boolean
	readonly paths: WorktreePaths
	readonly scenario: number
}): ProcessSpec {
	const dataDir = scenarioDataPath(input.paths, input.scenario)
	const namespace = `${input.branch}:scenario:${input.scenario}`
	// Everything that differs between runs reaches the App here, so one build serves them all.
	const appDevConfig = {
		backendUrl: input.backendUrl,
		authMode: 'dev',
		branch: input.branch,
		credentialNamespace: namespace,
		dataDir,
		devUrl: input.devUrl,
		hideWindow: input.hideWindows,
		title: `Modrinth ${input.scenario} - ${input.branch}`,
		username: scenarioUsername(input.scenario),
	}
	return {
		args: ['--amberite-dev-config', JSON.stringify(appDevConfig)],
		build: appProgramBuild(input.paths, input.env),
		cwd: NodePath.join(input.paths.worktree, 'apps', 'app'),
		env: {
			...input.env,
			RUST_LOG: 'theseus=warn,theseus_gui=warn,amberite_browser_bridge=info,webview=off',
			THESEUS_CONFIG_DIR: dataDir,
			WEBVIEW2_USER_DATA_FOLDER: NodePath.join(dataDir, 'webview2'),
		},
		label: `app:${input.scenario}`,
	}
}

function appProgramBuild(paths: WorktreePaths, env: NodeJS.ProcessEnv): ProgramBuild {
	const appDir = NodePath.join(paths.worktree, 'apps', 'app')
	const appLibDir = NodePath.join(paths.worktree, 'packages', 'app-lib')
	const manifest = NodeFS.readFileSync(NodePath.join(paths.worktree, 'Cargo.toml'), 'utf8')
	const members = /^members = \[([^\]]*)\]/m.exec(manifest)?.[1] ?? ''
	const crates = [...members.matchAll(/"([^"]+)"/g), ...manifest.matchAll(/\bpath = "([^"]+)"/g)]
	// The App library's build passes these to the compiler: from `.env`, or from the
	// environment when that file is missing.
	const compiled = Object.keys({
		...readEnv(NodePath.join(appLibDir, '.env.prod')),
		...readEnv(NodePath.join(appLibDir, '.env')),
	}).flatMap((name) => (env[name] === undefined ? [] : [[name, env[name]]]))
	return {
		// What `tauri dev` ran. Without `custom-protocol` the App loads its frontend from devUrl.
		cargoArgs: [
			'build',
			'--package',
			'theseus_gui',
			'--no-default-features',
			'--features',
			'browser-bridge',
		],
		cwd: appDir,
		env: {
			...Object.fromEntries(compiled),
			TAURI_CONFIG: JSON.stringify(
				JSON.parse(NodeFS.readFileSync(NodePath.join(appDir, 'tauri-local.conf.json'), 'utf8')),
			),
		},
		ignoredInputs: [NodePath.join(appLibDir, '.env')],
		// When unsure whether the build reads something, list it: a stale program is the worst
		// outcome. Every workspace crate counts, and the App's version comes from the frontend.
		inputs: [
			'Cargo.toml',
			'Cargo.lock',
			'rust-toolchain.toml',
			'.cargo',
			'apps/app-frontend/package.json',
			...new Set(crates.map((match) => match[1]!)),
		],
		program: programFile('theseus_gui'),
		// Every checkout compiles into the primary checkout's build output. A new worktree then
		// reuses the dependencies already compiled there and never grows its own target folder.
		targetDir: env.CARGO_TARGET_DIR ?? NodePath.join(paths.primary, 'target'),
	}
}

function programFile(name: string): string {
	return process.platform === 'win32' ? `${name}.exe` : name
}

/**
 * Names a program by everything that goes into building it, wherever the checkout is. Two
 * checkouts with the same key can run the same program.
 */
export function programKey(worktree: string, build: ProgramBuild): string {
	const hash = NodeCrypto.createHash('sha256')
	const add = (...parts: ReadonlyArray<string>) => {
		for (const part of parts) hash.update(part).update('\0')
	}
	add(process.platform, process.arch, ...build.cargoArgs)
	add(JSON.stringify(Object.entries(build.env).sort(([left], [right]) => (left < right ? -1 : 1))))
	// Every input's content as Git would store it, so the same content gives the same key
	// whether it is committed, staged, edited, or new.
	const contents = new Map<string, string>()
	for (const entry of git(['ls-files', '-s', '-z', '--', ...build.inputs], worktree).split('\0')) {
		const [details, file] = entry.split('\t')
		if (details && file) contents.set(file, details.split(' ')[1]!)
	}
	const edited = git(
		['ls-files', '-m', '-o', '--exclude-standard', '-z', '--', ...build.inputs],
		worktree,
	)
		.split('\0')
		.filter(Boolean)
	const present = edited.filter((file) => NodeFS.existsSync(NodePath.join(worktree, file)))
	for (const file of edited) contents.delete(file)
	if (present.length > 0) {
		const hashes = git(['hash-object', '--stdin-paths'], worktree, present.join('\n')).split('\n')
		present.forEach((file, index) => contents.set(file, hashes[index]!.trim()))
	}
	for (const file of build.ignoredInputs) {
		contents.set(
			NodePath.relative(worktree, file).replaceAll('\\', '/'),
			NodeFS.existsSync(file)
				? NodeCrypto.createHash('sha256').update(NodeFS.readFileSync(file)).digest('hex')
				: 'missing',
		)
	}
	for (const file of [...contents.keys()].sort()) add(file, contents.get(file)!)
	return hash.digest('hex').slice(0, 20)
}

/** The cached program for this build's current inputs, built first when there is none. */
async function ensureProgram(
	worktree: string,
	spec: ProcessSpec,
	build: ProgramBuild,
	builds: Set<NodeChildProcess.ChildProcess>,
): Promise<string | undefined> {
	const cache = NodePath.join(build.targetDir, 'dev-bin')
	const cached = (key: string) => NodePath.join(cache, key, build.program)
	let key = programKey(worktree, build)
	if (NodeFS.existsSync(cached(key))) {
		runnerLog('info', `${spec.label} using ${cached(key)}`)
	} else {
		const unlock = await lockBuilds(cache, spec.label)
		try {
			// Another runner may have built it while this one waited.
			while (!NodeFS.existsSync(cached(key))) {
				runnerLog('info', `${spec.label} building ${build.program} (${key})`)
				if (!markCheckoutCratesChanged(worktree, spec, build)) return undefined
				if (!(await runCargo(spec, build, builds, build.cargoArgs))) return undefined
				const builtKey = programKey(worktree, build)
				// An input edited during the build makes this program a mix. Build again.
				if (builtKey === key) {
					NodeFS.mkdirSync(NodePath.dirname(cached(key)), { recursive: true })
					const partial = `${cached(key)}.${process.pid}.partial`
					NodeFS.copyFileSync(NodePath.join(build.targetDir, 'debug', build.program), partial)
					NodeFS.renameSync(partial, cached(key))
				}
				key = builtKey
			}
		} finally {
			unlock()
		}
	}
	const used = new Date()
	NodeFS.utimesSync(NodePath.dirname(cached(key)), used, used)
	const keys = NodeFS.readdirSync(cache)
		.filter((entry) => entry !== BUILD_LOCK)
		.map((entry) => NodePath.join(cache, entry))
		.sort((left, right) => NodeFS.statSync(right).mtimeMs - NodeFS.statSync(left).mtimeMs)
	for (const old of keys.slice(CACHED_PROGRAMS)) {
		try {
			NodeFS.rmSync(old, { recursive: true })
		} catch {
			// Still running in another checkout.
		}
	}
	return cached(key)
}

/**
 * Cargo decides whether a crate needs compiling from file times alone, and checkouts share one
 * build output. A crate last compiled from another checkout looks finished whenever this
 * checkout's files are older, whatever they contain. So after another checkout's build, the
 * crates that live in the checkout are marked as changed. Cargo then compiles them again from
 * this checkout's files, and the compiler reuses whatever work still applies.
 */
function markCheckoutCratesChanged(
	worktree: string,
	spec: ProcessSpec,
	build: ProgramBuild,
): boolean {
	const lastCheckout = NodePath.join(build.targetDir, `${build.program}.checkout`)
	if (NodeFS.existsSync(lastCheckout) && NodeFS.readFileSync(lastCheckout, 'utf8') === worktree) {
		return true
	}
	const metadata = NodeChildProcess.spawnSync('cargo', ['metadata', '--format-version', '1'], {
		cwd: build.cwd,
		encoding: 'utf8',
		env: { ...spec.env, ...build.env },
		maxBuffer: 256 * 1024 * 1024,
		windowsHide: true,
	})
	if (metadata.status !== 0) {
		runnerLog('error', metadata.stderr.trim() || 'cargo metadata failed.')
		return false
	}
	const { packages } = JSON.parse(metadata.stdout) as {
		packages: ReadonlyArray<{
			source: string | null
			targets: ReadonlyArray<{ src_path: string }>
		}>
	}
	runnerLog('info', `${spec.label} last built from another checkout, compiling its crates again`)
	const now = new Date()
	// A crate without a source is read from a folder rather than downloaded. Each target's first
	// file (lib.rs, main.rs, build.rs) is enough: compiling it reads the rest.
	for (const crate of packages.filter((entry) => entry.source === null)) {
		for (const target of crate.targets) NodeFS.utimesSync(target.src_path, now, now)
	}
	NodeFS.writeFileSync(lastCheckout, worktree)
	return true
}

/** Whether Cargo succeeded. */
async function runCargo(
	spec: ProcessSpec,
	build: ProgramBuild,
	builds: Set<NodeChildProcess.ChildProcess>,
	args: ReadonlyArray<string>,
): Promise<boolean> {
	const debug = NodePath.join(build.targetDir, 'debug')
	// Cargo keeps each program in both folders, and either one can be the file moved aside below.
	for (const folder of [debug, NodePath.join(debug, 'deps')]) {
		for (const entry of NodeFS.existsSync(folder) ? NodeFS.readdirSync(folder) : []) {
			if (!entry.endsWith('.old')) continue
			try {
				NodeFS.rmSync(NodePath.join(folder, entry))
			} catch {
				// Still running.
			}
		}
	}
	for (let retried = false; ; retried = true) {
		const child = NodeChildProcess.spawn('cargo', [...args], {
			cwd: build.cwd,
			detached: process.platform !== 'win32',
			env: { ...spec.env, ...build.env, CARGO_TARGET_DIR: build.targetDir },
			stdio: ['ignore', 'pipe', 'pipe'],
			windowsHide: true,
		})
		builds.add(child)
		const failure: string[] = []
		const rememberFailure = (line: string) => {
			if (/Access is denied|failed to remove file/.test(line)) failure.push(line)
		}
		pipeOutput(child.stdout, spec.label, process.stdout)
		pipeOutput(child.stderr, spec.label, process.stderr, rememberFailure)
		const code = await new Promise<number>((resolve) => {
			child.once('error', (error) => {
				runnerLog('error', `Could not start cargo: ${error.message}`)
				resolve(1)
			})
			child.once('close', (exitCode) => resolve(exitCode ?? 1))
		})
		builds.delete(child)
		if (code === 0) return true
		// A checkout whose runner still starts programs straight from the build output has one
		// running. Windows cannot replace a running program but can rename it, which leaves that
		// one running.
		const locked = failure
			.map((line) => /failed to remove file `(.+\.exe)`/.exec(line)?.[1])
			.find(Boolean)
		if (retried || !locked || !failure.some((line) => line.includes('Access is denied'))) {
			return false
		}
		NodeFS.renameSync(locked, `${locked}.${Date.now()}.old`)
		runnerLog('info', `${spec.label} moved a running ${NodePath.basename(locked)} aside`)
	}
}

/**
 * One build and copy at a time per build output. Checkouts share that output, and Cargo puts the
 * finished program at the same path for all of them, so a second build finishing before the copy
 * would hand this checkout the other one's program.
 */
async function lockBuilds(cache: string, label: string): Promise<() => void> {
	NodeFS.mkdirSync(cache, { recursive: true })
	const lock = NodePath.join(cache, BUILD_LOCK)
	const owner = NodePath.join(lock, 'pid')
	for (let waited = false; ;) {
		try {
			NodeFS.mkdirSync(lock)
			break
		} catch (error) {
			if (!isRecord(error) || error.code !== 'EEXIST') throw error
		}
		let stale: boolean
		try {
			// A runner that was killed mid-build leaves its lock behind.
			stale =
				!processIsRunning(Number(NodeFS.readFileSync(owner, 'utf8'))) ||
				Date.now() - NodeFS.statSync(lock).mtimeMs > BUILD_LOCK_STALE_MS
		} catch {
			// Taken or released this instant.
			stale = false
		}
		if (stale) {
			NodeFS.rmSync(lock, { recursive: true, force: true })
			continue
		}
		if (!waited) runnerLog('info', `${label} waiting for another build to finish`)
		waited = true
		await new Promise((resolve) => setTimeout(resolve, 1_000))
	}
	NodeFS.writeFileSync(owner, String(process.pid))
	return () => NodeFS.rmSync(lock, { recursive: true, force: true })
}

function scenarioDataPath(paths: WorktreePaths, scenario: number): string {
	return NodePath.join(paths.scenariosData, String(scenario))
}

function spawnProcess(
	worktree: string,
	spec: ProcessSpec,
	program: string | undefined,
	onLine: (line: string) => void,
): RunningProcess {
	const child = NodeChildProcess.spawn(
		program ?? process.execPath,
		program ? [...spec.args] : [resolveVpPath(worktree), ...spec.args],
		{
			cwd: spec.cwd,
			detached: process.platform !== 'win32',
			env: spec.env,
			stdio: ['pipe', 'pipe', 'pipe'],
			windowsHide: true,
		},
	)
	runnerLog('info', `${spec.label} PID ${child.pid ?? 'unavailable'}`)
	pipeOutput(child.stdout, spec.label, process.stdout, onLine)
	pipeOutput(child.stderr, spec.label, process.stderr, onLine)

	const done = new Promise<number>((resolve, reject) => {
		child.once('error', (error) =>
			reject(new DevRunnerError(`Could not start ${spec.label}: ${error.message}`)),
		)
		child.once('close', (code, signal) => {
			if (code !== null) resolve(code)
			else resolve(signal ? 1 : 0)
		})
	})

	return { child, done, label: spec.label }
}

/** Stops a process the runner started, and everything that process started. */
function stopChild(child: NodeChildProcess.ChildProcess): void {
	const pid = child.pid
	if (!pid || child.exitCode !== null) return

	if (process.platform === 'win32') {
		NodeChildProcess.spawnSync('taskkill', ['/PID', String(pid), '/T', '/F'], {
			stdio: 'ignore',
			windowsHide: true,
		})
		return
	}

	try {
		process.kill(-pid, 'SIGTERM')
	} catch {
		child.kill('SIGTERM')
	}
}

function pipeOutput(
	stream: NodeJS.ReadableStream | null,
	label: string,
	destination: NodeJS.WritableStream,
	onLine?: (line: string) => void,
): void {
	if (!stream) return
	const lines = NodeReadline.createInterface({ input: stream })
	let previousLine = ''
	const seenRepeatedNoise = new Set<string>()
	lines.on('line', (line) => {
		const plainLine = stripAnsi(line).trimEnd()
		onLine?.(plainLine)
		if (
			!plainLine.trim() ||
			isNoisyProgressLine(plainLine) ||
			plainLine === previousLine ||
			(isRepeatedNoiseLine(plainLine) && seenRepeatedNoise.has(plainLine))
		)
			return
		if (isRepeatedNoiseLine(plainLine)) seenRepeatedNoise.add(plainLine)
		previousLine = plainLine
		const displayedLine =
			plainLine.length > 4_000 ? `${plainLine.slice(0, 4_000)} … truncated` : line.trimEnd()
		destination.write(`[${label}] ${displayedLine}\n`)
	})
}

/**
 * Commands come from stdin and from `.data/dev-commands`: a runner started detached (by an agent,
 * or from another terminal) has no usable stdin, so appending a line to that file is the way to
 * send it `rs core`, `rs 1`, or `quit`. The file is emptied when the runner starts.
 */
function createCommandInput(input: {
	readonly commandFile: string
	readonly getCore: () => RunningProcess | undefined
	readonly restart: (target: string) => Promise<void>
	readonly stop: () => Promise<void>
}): { close: () => void } {
	const commands = NodeReadline.createInterface({ input: process.stdin })
	commands.on('line', handle)
	NodeFS.writeFileSync(input.commandFile, '')
	let offset = 0
	const poll = setInterval(() => {
		let buffer: Buffer
		// The file can vanish or be replaced at any point; skip this poll and read it on the next.
		try {
			const size = NodeFS.statSync(input.commandFile).size
			if (size < offset) offset = 0
			if (size === offset) return
			buffer = Buffer.alloc(size - offset)
			const fd = NodeFS.openSync(input.commandFile, 'r')
			try {
				NodeFS.readSync(fd, buffer, 0, buffer.length, offset)
			} finally {
				NodeFS.closeSync(fd)
			}
			offset = size
		} catch {
			return
		}
		for (const line of buffer.toString('utf8').split(/\r?\n/)) handle(line)
	}, 500)
	poll.unref()

	function handle(line: string) {
		const command = line.trim()
		if (!command) return
		if (command === 'quit') {
			void input.stop()
			return
		}
		const restart = command.match(/^rs\s+(core|\d+)$/i)
		if (restart) {
			void input.restart(restart[1].toLowerCase())
			return
		}
		const coreCommand = command.match(/^core\s+(.+)$/i)
		if (coreCommand) {
			const core = input.getCore()
			if (!core?.child.stdin?.writable) {
				runnerLog('warning', 'Core is not running.')
				return
			}
			core.child.stdin.write(`${coreCommand[1]}\n`)
			return
		}
		if (command === 'help') {
			runnerLog('info', 'Commands: rs <scenario>, rs core, core <command>, quit')
			return
		}
		runnerLog('warning', `Unknown command ${command}. Type help for available commands.`)
	}
	return {
		close: () => {
			clearInterval(poll)
			commands.close()
		},
	}
}

function isNoisyProgressLine(line: string): boolean {
	return (
		/^\s*Finished\s+/.test(line) ||
		/^\s*(?:Building|Checking|Fetch)\s+\[[= >-]+\]/.test(line) ||
		/^Browserslist: browsers data \(caniuse-lite\) is .+ old\./.test(line) ||
		/^\s*npx update-browserslist-db@latest$/.test(line) ||
		/^\s*Why you should do it regularly: https:\/\/github\.com\/browserslist\/update-db#readme$/.test(
			line,
		) ||
		/^\s*➜\s+press h \+ enter to show help$/.test(line)
	)
}

function isRepeatedNoiseLine(line: string): boolean {
	return (
		line.includes('Skipping origin header as it is a forbidden header') ||
		line.includes('if keeping the header is a desired behavior')
	)
}

function stripAnsi(value: string): string {
	return NodeUtil.stripVTControlCharacters(value)
}

function runnerLog(level: LogLevel, message: string): void {
	const destination = level === 'error' ? process.stderr : process.stdout
	destination.write(`[dev-runner] ${message}\n`)
}

async function portIsAvailable(port: number, hosts: ReadonlyArray<string>): Promise<boolean> {
	for (const host of hosts) {
		if (!(await canListen(port, host))) return false
	}
	return true
}

function canListen(port: number, host: string): Promise<boolean> {
	return new Promise((resolve) => {
		const server = NodeNet.createServer()
		server.unref()
		server.once('error', () => resolve(false))
		server.listen({ host, port }, () => server.close(() => resolve(true)))
	})
}

function requiredPortNames(mode: DevMode, accountWeb = true): ReadonlyArray<PortName> {
	const names = {
		dev: ['app', 'backend', 'accountWeb', 'core'],
		'dev:backend': ['backend', 'accountWeb'],
		'dev:app': ['app'],
		'dev:core': ['core'],
	} as const
	return names[mode].filter((name) => accountWeb || name !== 'accountWeb')
}

function stableHash(value: string): number {
	return NodeCrypto.createHash('sha256').update(value).digest().readUInt32BE(0)
}

function samePath(left: string, right: string): boolean {
	const normalize = (value: string) => {
		const resolved = NodePath.resolve(value)
		return process.platform === 'win32' ? resolved.toLowerCase() : resolved
	}
	return normalize(left) === normalize(right)
}

function currentBranch(worktree: string): string {
	return git(['branch', '--show-current'], worktree) || NodePath.basename(worktree)
}

function git(args: ReadonlyArray<string>, cwd: string, input?: string): string {
	const result = NodeChildProcess.spawnSync('git', args, {
		cwd,
		encoding: 'utf8',
		input,
		maxBuffer: 256 * 1024 * 1024,
	})
	if (result.status !== 0) {
		throw new DevRunnerError(result.stderr.trim() || `git ${args.join(' ')} failed.`)
	}
	return result.stdout.trim()
}

function readEnv(path: string): NodeJS.ProcessEnv {
	if (!NodeFS.existsSync(path)) return {}
	return NodeUtil.parseEnv(NodeFS.readFileSync(path, 'utf8'))
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function prepareLocalBackend(paths: WorktreePaths, env: NodeJS.ProcessEnv): void {
	const ticketKey = localTicketKey(NodePath.join(paths.data, 'backend'))
	const variables = `LOCAL_DEV=true\nLOCAL_DEV_SECRET=${env.AMBERITE_LOCAL_DEV_SECRET}\nTICKET_KEY=${ticketKey}\n`
	NodeFS.writeFileSync(NodePath.join(paths.data, 'backend', '.dev.vars'), variables, {
		mode: 0o600,
	})
	const result = NodeChildProcess.spawnSync(
		process.execPath,
		[
			resolveVpPath(paths.worktree),
			'exec',
			'wrangler',
			'd1',
			'migrations',
			'apply',
			'accounts',
			'--local',
			'--persist-to',
			NodePath.join(paths.data, 'backend'),
		],
		{
			cwd: NodePath.join(paths.worktree, 'apps', 'backend'),
			env,
			stdio: 'inherit',
			windowsHide: true,
		},
	)
	if (result.status !== 0) throw new DevRunnerError('Local account database migration failed.')
}

/**
 * The local backend's ticket key, made once in `ticket-key.json`, with its public half served as
 * `assets/core/keys.json` in place of the production keys in `apps/backend/public`. Returns the
 * private JWK for `TICKET_KEY`.
 */
function localTicketKey(backendData: string): string {
	const keyPath = NodePath.join(backendData, 'ticket-key.json')
	if (!NodeFS.existsSync(keyPath)) {
		const { privateKey } = NodeCrypto.generateKeyPairSync('ed25519')
		const jwk = { ...privateKey.export({ format: 'jwk' }), kid: `local-${NodeCrypto.randomUUID()}` }
		NodeFS.writeFileSync(keyPath, JSON.stringify(jwk), { mode: 0o600 })
	}
	const secret = NodeFS.readFileSync(keyPath, 'utf8').trim()
	const { kty, crv, kid, x } = JSON.parse(secret)
	const keysPath = NodePath.join(backendData, 'assets', 'core', 'keys.json')
	NodeFS.mkdirSync(NodePath.dirname(keysPath), { recursive: true })
	NodeFS.writeFileSync(keysPath, JSON.stringify({ keys: [{ kty, crv, kid, x }] }))
	return secret
}

function resolveVpPath(worktree: string): string {
	const path = NodePath.join(worktree, 'node_modules', 'vite-plus', 'bin', 'vp')
	if (!NodeFS.existsSync(path)) {
		throw new DevRunnerError('Vite+ is not installed. Run vp install first.')
	}
	return path
}

function writeRuntimeFile(paths: WorktreePaths, runtime: RuntimeFile): void {
	NodeFS.writeFileSync(paths.runtime, `${JSON.stringify(runtime, null, '\t')}\n`)
}

function stoppedRuntime(runtime: RuntimeFile): RuntimeFile {
	return {
		...runtime,
		pid: undefined,
		scenarios: runtime.scenarios.map((scenario) => ({ ...scenario, bridgeUrl: undefined })),
	}
}

/** The runtime file, only while the dev runner that wrote it is still running. */
function readLiveRuntime(paths: WorktreePaths): RuntimeFile | undefined {
	if (!NodeFS.existsSync(paths.runtime)) return undefined
	let runtime: RuntimeFile
	try {
		runtime = JSON.parse(NodeFS.readFileSync(paths.runtime, 'utf8')) as RuntimeFile
	} catch {
		// The runner rewrites this file while it works; a torn read is simply "not yet".
		return undefined
	}
	return runtime.pid && processIsRunning(runtime.pid) ? runtime : undefined
}

function processIsRunning(pid: number): boolean {
	try {
		process.kill(pid, 0)
		return true
	} catch {
		return false
	}
}

// A recorded PID can be reused by an unrelated process after a crash or reboot.
function processIsDevRunner(pid: number): boolean {
	const result =
		process.platform === 'win32'
			? NodeChildProcess.spawnSync(
					'powershell.exe',
					[
						'-NoProfile',
						'-Command',
						`(Get-CimInstance Win32_Process -Filter "ProcessId=${pid}").CommandLine`,
					],
					{ encoding: 'utf8', windowsHide: true },
				)
			: NodeChildProcess.spawnSync('ps', ['-o', 'args=', '-p', String(pid)], { encoding: 'utf8' })
	return result.status === 0 && result.stdout.includes('dev-runner')
}

async function runtimeIsReady(runtime: RuntimeFile): Promise<boolean> {
	if (runtime.scenarios.some((scenario) => !scenario.bridgeUrl)) return false
	const open = await Promise.all(
		requiredPortNames(runtime.mode, runtime.accountWeb).map((name) =>
			portIsOpen(runtime.ports[name]),
		),
	)
	return open.every(Boolean)
}

function portIsOpen(port: number): Promise<boolean> {
	return new Promise((resolve) => {
		const socket = NodeNet.connect({ host: 'localhost', port })
		const finish = (open: boolean) => {
			socket.destroy()
			resolve(open)
		}
		socket.setTimeout(2_000)
		socket.once('connect', () => finish(true))
		socket.once('error', () => finish(false))
		socket.once('timeout', () => finish(false))
	})
}

/** Prints what is running and where. Returns whether everything is ready to use. */
async function printStatus(paths: WorktreePaths): Promise<boolean> {
	const runtime = readLiveRuntime(paths)
	if (!runtime) {
		runnerLog('info', 'not running. Start it with: node scripts/dev-runner.ts up')
		return false
	}
	const ready = await runtimeIsReady(runtime)
	const labels = processLabelsForMode(runtime.mode, runtime.accountWeb)
	runnerLog('info', `${runtime.mode} PID ${runtime.pid} · ${ready ? 'ready' : 'starting'}`)
	if (labels.includes('backend')) runnerLog('info', `Accounts and sharing ${runtime.urls.backend}`)
	if (labels.includes('account-web')) {
		runnerLog('info', `Account sign-in ${runtime.urls.accountWeb}`)
	}
	if (labels.includes('core')) runnerLog('info', `Core ${runtime.urls.core}`)
	for (const scenario of runtime.scenarios) {
		runnerLog(
			'info',
			`App ${scenario.number} (${scenario.username}) ${scenario.bridgeUrl ?? 'starting'}`,
		)
	}
	runnerLog('info', `log ${devLogPath(paths)}`)
	return ready
}

/** Starts the dev stack in the background and returns once it is ready to use. */
async function up(paths: WorktreePaths, input: RunnerInput): Promise<void> {
	let started: NodeChildProcess.ChildProcess | undefined
	const live = readLiveRuntime(paths)
	// An App decides whether to show its window when it starts, so asking a running one is too late.
	if (live?.hideWindows && !input.hideWindows) {
		runnerLog(
			'error',
			'Already running with App windows hidden. Run node scripts/dev-runner.ts down first.',
		)
		process.exitCode = 1
		return
	}
	if (!live) {
		NodeFS.mkdirSync(paths.data, { recursive: true })
		const log = NodeFS.openSync(devLogPath(paths), 'w')
		started = NodeChildProcess.spawn(
			process.execPath,
			[
				NodeURL.fileURLToPath(import.meta.url),
				input.mode,
				...input.scenarios.map(String),
				...(input.accountWeb ? [] : ['--no-account-web']),
				...(input.hideWindows ? ['--hide-windows'] : []),
			],
			{ cwd: paths.worktree, detached: true, stdio: ['ignore', log, log], windowsHide: true },
		)
		started.unref()
		NodeFS.closeSync(log)
		runnerLog('info', `started PID ${started.pid}`)
	}

	const deadline = Date.now() + UP_WAIT_MS
	while (Date.now() < deadline) {
		if (started && started.exitCode !== null) {
			runnerLog('error', `The dev runner stopped while starting. See ${devLogPath(paths)}`)
			process.exitCode = 1
			return
		}
		const runtime = readLiveRuntime(paths)
		// A runtime file left by an earlier run does not describe the runner started above.
		const current = runtime && (!started || runtime.pid === started.pid) ? runtime : undefined
		if (current?.buildFailed) {
			runnerLog('error', `${current.buildFailed} did not build. See ${devLogPath(paths)}`)
			process.exitCode = 1
			return
		}
		if (current && (await runtimeIsReady(current))) {
			await printStatus(paths)
			return
		}
		await new Promise((resolve) => setTimeout(resolve, 2_000))
	}
	runnerLog(
		'warning',
		`Still starting. Run node scripts/dev-runner.ts up again to keep waiting. Log: ${devLogPath(paths)}`,
	)
	// Not a failure: the runner is still working.
}

/** Stops the dev runner recorded in the runtime file, and everything it started. */
async function down(paths: WorktreePaths): Promise<void> {
	const runtime = readLiveRuntime(paths)
	if (!runtime?.pid || !processIsDevRunner(runtime.pid)) {
		runnerLog('info', 'not running')
		return
	}
	if (process.platform === 'win32') {
		NodeChildProcess.spawnSync('taskkill', ['/PID', String(runtime.pid), '/T', '/F'], {
			stdio: 'ignore',
			windowsHide: true,
		})
	} else {
		// The runner stops its own children on SIGTERM.
		process.kill(runtime.pid, 'SIGTERM')
	}
	for (let attempt = 0; attempt < 50 && processIsRunning(runtime.pid); attempt += 1) {
		await new Promise((resolve) => setTimeout(resolve, 200))
	}
	if (processIsRunning(runtime.pid)) {
		runnerLog('error', `PID ${runtime.pid} did not stop.`)
		process.exitCode = 1
		return
	}
	// What the runner started takes a moment longer to go. A start right after this must not
	// meet the old backend.
	const ports = requiredPortNames(runtime.mode, runtime.accountWeb).map(
		(name) => runtime.ports[name],
	)
	for (let attempt = 0; attempt < 50; attempt += 1) {
		if (!(await Promise.all(ports.map(portIsOpen))).some(Boolean)) break
		await new Promise((resolve) => setTimeout(resolve, 200))
	}
	writeRuntimeFile(paths, stoppedRuntime(runtime))
	runnerLog('info', `stopped PID ${runtime.pid}`)
}

function devLogPath(paths: WorktreePaths): string {
	return NodePath.join(paths.data, 'dev.log')
}

function printPlan(input: {
	readonly branch: string
	readonly mode: DevMode
	readonly paths: WorktreePaths
	readonly ports: DevPorts
	readonly scenarios: ReadonlyArray<number>
	readonly source: string
	readonly specs: ReadonlyArray<ProcessSpec>
}): void {
	runnerLog('info', `${input.branch} · ${input.mode}`)
	runnerLog('info', `data ${input.paths.data}`)
	runnerLog('info', `ports from ${input.source}`)
	const labels = input.specs.map((spec) => spec.label)
	if (labels.includes('app-frontend')) runnerLog('info', `App http://localhost:${input.ports.app}`)
	if (labels.includes('backend')) {
		runnerLog('info', `Accounts and sharing http://127.0.0.1:${input.ports.backend} (local)`)
	}
	if (labels.includes('account-web')) {
		runnerLog('info', `Account sign-in http://127.0.0.1:${input.ports.accountWeb}`)
	}
	if (labels.includes('core')) runnerLog('info', `Core http://127.0.0.1:${input.ports.core}`)
	if (input.scenarios.length > 0) {
		runnerLog('info', `scenarios ${input.scenarios.join(', ')}`)
	}
	runnerLog('info', `processes ${input.specs.map((spec) => spec.label).join(', ')}`)
}

const isMain = process.argv[1] && samePath(process.argv[1], NodeURL.fileURLToPath(import.meta.url))
if (isMain) {
	main().catch((error: unknown) => {
		runnerLog('error', error instanceof Error ? error.message : String(error))
		process.exitCode = 1
	})
}
