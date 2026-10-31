#!/usr/bin/env node

import * as NodeChildProcess from 'node:child_process'
import * as NodeCrypto from 'node:crypto'
import * as NodeFS from 'node:fs'
import * as NodeNet from 'node:net'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'
import * as NodeReadline from 'node:readline'
import * as NodeURL from 'node:url'
import * as NodeUtil from 'node:util'
import { copyDataEntry, DATA_ENTRIES } from './dev-data.ts'
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
// How much bigger than the primary's a worktree's build output grows before it starts over.
const BUILD_GROWTH_LIMIT = 10 * 2 ** 30
const BUILD_LOCK_STALE_MS = 60 * 60_000
const BUILD_LOCK = 'building'
// Beside each cached program: the checkout it was built in, whose build output a new worktree may clone.
const CHECKOUT_FILE = 'checkout'
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
 * into `<cacheDir>/<key>/`, and run from that copy. A checkout whose Rust matches something
 * already built starts without Cargo, and a running program never blocks a build.
 */
export type ProgramBuild = {
	/** Programs built by every checkout, shared. Its lock lets one build run at a time. */
	readonly cacheDir: string
	readonly cargoArgs: ReadonlyArray<string>
	/** The primary checkout, whose `target` is cloned into `targetDir` before the first build. */
	readonly cloneFrom?: string
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
		const presetIndex = commandArgs.indexOf('--preset')
		const preset = presetIndex === -1 ? undefined : commandArgs[presetIndex + 1]
		if (presetIndex !== -1 && !preset) throw new DevRunnerError('--preset needs a preset name.')
		const args = (presetIndex === -1 ? commandArgs : commandArgs.toSpliced(presetIndex, 2)).filter(
			(arg) => arg !== '--show-windows',
		)
		// The mode is optional here: `up 1 2` means the full stack with those scenarios.
		const hasMode = DEV_MODES.some((mode) => mode === args[0])
		return up(
			paths,
			{
				...parseInput(hasMode ? args : ['dev', ...args], defaultScenarios),
				// Apps sign themselves in and are used through the browser bridge.
				accountWeb: false,
				hideWindows: !commandArgs.includes('--show-windows'),
			},
			preset,
		)
	}
	if (command === 'down') return down(paths)
	if (command === 'status') {
		if (!(await printStatus(paths))) process.exitCode = 1
		return
	}
	if (command === 'build') return buildPrograms(paths)
	if (command === 'preset') return savePreset(paths, commandArgs)
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
	const env = createRuntimeEnvironment({ baseEnv: readSharedEnv(paths), paths, ports })
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
			program = await ensureProgram(paths.worktree, spec, spec.build, builds, () => stopping)
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

/** Settings from the env files, overridden by the environment. */
function readSharedEnv(paths: WorktreePaths): NodeJS.ProcessEnv {
	return {
		...readEnv(NodePath.join(paths.worktree, 'packages', 'app-lib', '.env.prod')),
		...readEnv(NodePath.join(paths.worktree, 'packages', 'app-lib', '.env')),
		...readEnv(NodePath.join(paths.primary, '.env.local')),
		...readEnv(NodePath.join(paths.primary, 'apps', 'core', '.env.local')),
		...readEnv(NodePath.join(paths.worktree, '.env.local')),
		...process.env,
	}
}

/**
 * `build`: compiles the App and Core for this checkout's current Rust without starting anything,
 * so a later `up` finds them ready. Worktree setup runs it in the background.
 */
async function buildPrograms(paths: WorktreePaths): Promise<void> {
	const ports = portsForOffset(0)
	const specs = createProcessSpecs({
		accountWeb: false,
		branch: currentBranch(paths.worktree),
		env: createRuntimeEnvironment({ baseEnv: readSharedEnv(paths), paths, ports }),
		hideWindows: true,
		mode: 'dev',
		paths,
		ports,
		scenarios: [1],
	})
	for (const spec of specs) {
		if (!spec.build) continue
		if (!(await ensureProgram(paths.worktree, spec, spec.build, new Set(), () => false))) {
			process.exitCode = 1
		}
	}
}

/**
 * `preset` lists the saved datasets. `preset save <name>` saves this checkout's data under that
 * name, replacing any earlier one, so any checkout can start from it with `up --preset <name>`.
 * Presets live in the primary checkout's `.data/presets`.
 */
function savePreset(paths: WorktreePaths, args: ReadonlyArray<string>): void {
	const [action, name] = args
	if (action === undefined) {
		const presets = NodePath.join(paths.primary, '.data', 'presets')
		const names = NodeFS.existsSync(presets) ? NodeFS.readdirSync(presets) : []
		for (const preset of names.filter((entry) => !entry.endsWith('.partial'))) {
			const scenarios = NodePath.join(presets, preset, 'scenarios')
			const numbers = NodeFS.existsSync(scenarios) ? NodeFS.readdirSync(scenarios) : []
			runnerLog('info', `${preset} (scenarios ${numbers.join(' ') || 'none'})`)
		}
		if (names.length === 0) runnerLog('info', 'no presets. Save one with: preset save <name>')
		return
	}
	if (action !== 'save' || !name) {
		throw new DevRunnerError('Use preset to list presets, or preset save <name>.')
	}
	const target = presetPath(paths, name)
	if (readLiveRuntime(paths)) {
		throw new DevRunnerError(
			'Run node scripts/dev-runner.ts down first, so the data is not in use.',
		)
	}
	NodeFS.mkdirSync(target, { recursive: true })
	for (const entry of DATA_ENTRIES) {
		if (NodeFS.existsSync(NodePath.join(paths.data, entry)))
			copyDataEntry(paths.data, target, entry)
		else NodeFS.rmSync(NodePath.join(target, entry), { recursive: true, force: true })
	}
	runnerLog('info', `saved preset ${name}`)
}

function presetPath(paths: WorktreePaths, name: string): string {
	if (!/^[a-z0-9][a-z0-9-]*$/.test(name)) {
		throw new DevRunnerError(`Preset names use lowercase letters, digits, and dashes: ${name}`)
	}
	return NodePath.join(paths.primary, '.data', 'presets', name)
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
			args: ['run'],
			build: {
				cargoArgs: ['build', '--package', 'copal'],
				cwd: coreDir,
				env: {},
				ignoredInputs: [],
				inputs: workspaceInputs(input.paths, 'copal'),
				program: programFile('copal'),
				...workspaceBuildDirs(input.paths, input.env),
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
		// The App's version comes from the frontend.
		inputs: [...workspaceInputs(paths, 'theseus_gui'), 'apps/app-frontend/package.json'],
		program: programFile('theseus_gui'),
		...workspaceBuildDirs(paths, env),
	}
}

/**
 * What a program in the root Cargo workspace (the App and Core) may read: the workspace settings
 * and the folders of `packageName` and every local crate it builds with. Crates it does not use,
 * such as labrinth, change without rebuilding it. When unsure whether a build reads something,
 * list it: a stale program is the worst outcome.
 */
function workspaceInputs(paths: WorktreePaths, packageName: string): ReadonlyArray<string> {
	const settings = ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo']
	const crates = localCrates(paths.worktree)
	const root = [...crates.values()].find((crate) => crate.name === packageName)
	// The manifests do not read. Cargo prints why when it builds; until then, assume anything.
	if (!root) return [...settings, 'apps', 'packages']
	const folders = new Set<string>()
	const visit = (folder: string) => {
		if (folders.has(folder)) return
		folders.add(folder)
		for (const dependency of crates.get(folder)?.dependencies ?? []) visit(dependency)
	}
	visit(root.folder)
	const checkout = NodeFS.realpathSync.native(paths.worktree)
	return [
		...settings,
		...[...folders].map((folder) => NodePath.relative(checkout, folder).replaceAll('\\', '/')),
	]
}

type LocalCrate = {
	/** Folders of the local crates it builds with. Development-only ones are left out. */
	readonly dependencies: ReadonlyArray<string>
	readonly folder: string
	readonly name: string
}
const localCratesResults = new Map<string, ReadonlyMap<string, LocalCrate>>()
/** The workspace's crates by folder, as Cargo reports them. Empty when the manifests do not read. */
function localCrates(worktree: string): ReadonlyMap<string, LocalCrate> {
	const known = localCratesResults.get(worktree)
	if (known) return known
	const metadata = NodeChildProcess.spawnSync(
		'cargo',
		['metadata', '--no-deps', '--offline', '--format-version', '1'],
		{ cwd: worktree, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, windowsHide: true },
	)
	const crates = new Map<string, LocalCrate>()
	if (metadata.status === 0) {
		const { packages } = JSON.parse(metadata.stdout) as {
			packages: ReadonlyArray<{
				dependencies: ReadonlyArray<{ kind: string | null; path?: string }>
				manifest_path: string
				name: string
			}>
		}
		for (const crate of packages) {
			const folder = NodePath.dirname(crate.manifest_path)
			crates.set(folder, {
				dependencies: crate.dependencies.flatMap((dependency) =>
					dependency.path && dependency.kind !== 'dev' ? [dependency.path] : [],
				),
				folder,
				name: crate.name,
			})
		}
	}
	localCratesResults.set(worktree, crates)
	return crates
}

/**
 * Where a checkout compiles the workspace. On a drive that clones files without copying them
 * (a Windows Dev Drive), each worktree gets its own build output, cloned from the primary
 * checkout's, so worktrees keep their own compiler caches and take little extra space.
 * Elsewhere every checkout compiles into the primary checkout's build output, so a new worktree
 * still reuses the dependencies compiled there.
 */
function workspaceBuildDirs(
	paths: WorktreePaths,
	env: NodeJS.ProcessEnv,
): Pick<ProgramBuild, 'cacheDir' | 'cloneFrom' | 'targetDir'> {
	const shared = env.CARGO_TARGET_DIR ?? NodePath.join(paths.primary, 'target')
	const cacheDir = NodePath.join(shared, 'dev-bin')
	if (env.CARGO_TARGET_DIR || !clonesFiles(paths)) return { cacheDir, targetDir: shared }
	return { cacheDir, cloneFrom: paths.primary, targetDir: NodePath.join(paths.worktree, 'target') }
}

let clonesFilesResult: boolean | undefined
/** Whether this worktree and the primary checkout share a drive that clones files: ReFS. */
function clonesFiles(paths: WorktreePaths): boolean {
	if (clonesFilesResult !== undefined) return clonesFilesResult
	clonesFilesResult = false
	if (process.platform !== 'win32') return clonesFilesResult
	// T3 Code reaches its worktrees through a junction, so compare where the files really are.
	const worktree = NodeFS.realpathSync.native(paths.worktree)
	const primary = NodeFS.realpathSync.native(paths.primary)
	const drive = NodePath.parse(worktree).root.slice(0, 1).toUpperCase()
	if (worktree === primary || NodePath.parse(primary).root.slice(0, 1).toUpperCase() !== drive) {
		return clonesFilesResult
	}
	const result = NodeChildProcess.spawnSync(
		'powershell',
		['-NoProfile', '-Command', `(Get-Volume -DriveLetter ${drive}).FileSystem`],
		{ encoding: 'utf8', windowsHide: true },
	)
	clonesFilesResult = result.stdout.trim() === 'ReFS'
	return clonesFilesResult
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
	stopped: () => boolean,
): Promise<string | undefined> {
	const cache = build.cacheDir
	const cached = (key: string) => NodePath.join(cache, key, build.program)
	let key = programKey(worktree, build)
	if (NodeFS.existsSync(cached(key))) {
		runnerLog('info', `${spec.label} using ${cached(key)}`)
	} else {
		const unlock = await lockBuilds(cache, spec.label, stopped)
		if (!unlock) return undefined
		try {
			// Another runner may have built it while this one waited.
			while (!NodeFS.existsSync(cached(key))) {
				runnerLog('info', `${spec.label} building ${build.program} (${key})`)
				if (build.cloneFrom) cloneBuildOutput(worktree, spec, build, build.cloneFrom)
				else if (!markCheckoutCratesChanged(worktree, spec, build)) return undefined
				if (!(await runCargo(spec, build, builds, build.cargoArgs))) return undefined
				const builtKey = programKey(worktree, build)
				// An input edited during the build makes this program a mix. Build again.
				if (builtKey === key) {
					NodeFS.mkdirSync(NodePath.dirname(cached(key)), { recursive: true })
					const partial = `${cached(key)}.${process.pid}.partial`
					NodeFS.copyFileSync(NodePath.join(build.targetDir, 'debug', build.program), partial)
					NodeFS.renameSync(partial, cached(key))
					NodeFS.writeFileSync(
						NodePath.join(NodePath.dirname(cached(key)), CHECKOUT_FILE),
						worktree,
					)
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

/**
 * Gives a worktree its own build output by cloning another checkout's, then makes the clone look
 * built from this worktree: Cargo trusts file times, and build scripts record absolute paths.
 * Crates whose files match the source's are then up to date; changed ones compile again. The
 * source is whichever checkout behind a cached program, or the primary, has the fewest Rust files
 * that differ from this one. The primary is rarely built, so that is usually a recent worktree.
 *
 * Cargo never deletes old output, and every file a build writes stops being shared with the
 * source. So once a worktree's build output outgrows the source's by `BUILD_GROWTH_LIMIT`, it is
 * deleted and cloned again. That build compiles this worktree's changes without their compiler
 * caches, and the worktree is back to taking almost no space.
 */
function cloneBuildOutput(
	worktree: string,
	spec: ProcessSpec,
	build: ProgramBuild,
	primary: string,
): void {
	const source = closestBuiltCheckout(worktree, build, primary)
	const from = NodePath.join(source, 'target')
	if (!NodeFS.existsSync(from)) return
	if (NodeFS.existsSync(build.targetDir)) {
		const growth = folderSize(build.targetDir) - folderSize(from, build.cacheDir)
		if (growth < BUILD_GROWTH_LIMIT) return
		const old = `${build.targetDir}.old`
		try {
			NodeFS.rmSync(old, { recursive: true, force: true })
			NodeFS.renameSync(build.targetDir, old)
		} catch {
			// Something still has a file open, such as an older App run from it. Build there.
			return
		}
		runnerLog(
			'info',
			`${spec.label} deleting its build output, ${(growth / 2 ** 30).toFixed(1)} GB over ${source}'s`,
		)
		NodeFS.rmSync(old, { recursive: true, force: true })
	}
	runnerLog('info', `${spec.label} cloning the build output of ${source}`)
	const partial = `${build.targetDir}.partial`
	NodeFS.rmSync(partial, { recursive: true, force: true })
	NodeFS.cpSync(from, partial, {
		filter: (path) => path !== build.cacheDir && !path.endsWith('.checkout'),
		preserveTimestamps: true,
		recursive: true,
	})
	const buildScripts = NodePath.join(partial, 'debug', 'build')
	const paths = [source, worktree].map((path) => `${path}${NodePath.sep}`)
	const [sourceEscaped, worktreeEscaped] = paths.map((path) => path.replaceAll('\\', '\\\\'))
	for (const folder of NodeFS.existsSync(buildScripts) ? NodeFS.readdirSync(buildScripts) : []) {
		for (const name of ['output', 'root-output']) {
			const file = NodePath.join(buildScripts, folder, name)
			if (!NodeFS.existsSync(file)) continue
			const text = NodeFS.readFileSync(file, 'utf8')
			const rewritten = text
				.replaceAll(sourceEscaped!, worktreeEscaped!)
				.replaceAll(paths[0]!, paths[1]!)
			if (rewritten === text) continue
			// Rewritten in place with its old time, or Cargo would run the build script again.
			const { atime, mtime } = NodeFS.statSync(file)
			NodeFS.writeFileSync(file, rewritten)
			NodeFS.utimesSync(file, atime, mtime)
		}
	}
	NodeFS.renameSync(partial, build.targetDir)

	const sameFiles = [...sameInputFiles(worktree, source, build.inputs)]
	for (const file of build.ignoredInputs) {
		const original = NodePath.join(source, NodePath.relative(worktree, file))
		if (
			NodeFS.existsSync(file) &&
			NodeFS.existsSync(original) &&
			NodeFS.readFileSync(file).equals(NodeFS.readFileSync(original))
		) {
			sameFiles.push(NodePath.relative(worktree, file))
		}
	}
	for (const file of sameFiles) {
		const { atime, mtime } = NodeFS.statSync(NodePath.join(source, file))
		NodeFS.utimesSync(NodePath.join(worktree, file), atime, mtime)
	}
}

/**
 * Of the primary and the checkouts that built a cached program, the one with build output whose
 * committed Rust files differ least from `worktree`'s.
 */
function closestBuiltCheckout(worktree: string, build: ProgramBuild, primary: string): string {
	const builders = NodeFS.readdirSync(build.cacheDir).flatMap((entry) => {
		const file = NodePath.join(build.cacheDir, entry, CHECKOUT_FILE)
		return NodeFS.existsSync(file) ? [NodeFS.readFileSync(file, 'utf8')] : []
	})
	const candidates = [...new Set([primary, ...builders])].filter(
		(checkout) =>
			!samePath(checkout, worktree) && NodeFS.existsSync(NodePath.join(checkout, 'target')),
	)
	const inputs = inputBlobs(worktree, build.inputs).size
	let closest = { checkout: primary, differing: Number.POSITIVE_INFINITY }
	for (const checkout of candidates) {
		let differing: number
		try {
			differing = inputs - sameInputFiles(worktree, checkout, build.inputs).length
		} catch {
			// Deleted while this ran.
			continue
		}
		if (differing < closest.differing) closest = { checkout, differing }
	}
	return closest.checkout
}

/** Input files, relative to the checkouts, committed with the same content in both and edited in neither. */
function sameInputFiles(
	checkout: string,
	other: string,
	inputs: ReadonlyArray<string>,
): ReadonlyArray<string> {
	const otherBlobs = inputBlobs(other, inputs)
	const edited = new Set([...editedInputs(other, inputs), ...editedInputs(checkout, inputs)])
	return [...inputBlobs(checkout, inputs)]
		.filter(([file, blob]) => otherBlobs.get(file) === blob && !edited.has(file))
		.map(([file]) => file)
}

/** Each committed input file, relative to the checkout, with its content's hash. */
function inputBlobs(checkout: string, inputs: ReadonlyArray<string>): ReadonlyMap<string, string> {
	return new Map(
		git(['ls-files', '-s', '-z', '--', ...inputs], checkout)
			.split('\0')
			.flatMap((entry) => {
				const [details, file] = entry.split('\t')
				return details && file ? [[file, details.split(' ')[1]!] as const] : []
			}),
	)
}

function editedInputs(checkout: string, inputs: ReadonlyArray<string>): ReadonlyArray<string> {
	return git(['ls-files', '-m', '-z', '--', ...inputs], checkout).split('\0')
}

/** The total size in bytes of the files in a folder, leaving out `skip`. */
function folderSize(folder: string, skip?: string): number {
	return NodeFS.readdirSync(folder, { withFileTypes: true }).reduce((total, entry) => {
		const path = NodePath.join(folder, entry.name)
		if (path === skip) return total
		if (entry.isDirectory()) return total + folderSize(path, skip)
		return entry.isFile() ? total + NodeFS.statSync(path).size : total
	}, 0)
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
		// Compiles started by any checkout run behind whatever the person is doing. Windows gives
		// the compiler processes Cargo starts the same priority.
		try {
			if (child.pid) NodeOS.setPriority(child.pid, NodeOS.constants.priority.PRIORITY_BELOW_NORMAL)
		} catch {
			// Already finished.
		}
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
 * One build at a time across every checkout, or gives up with `undefined` once the runner is
 * stopping. Checkouts that share a build output need it: Cargo puts the finished program at the
 * same path for all of them. It also keeps several compiles from taking the whole machine.
 */
async function lockBuilds(
	cache: string,
	label: string,
	stopped: () => boolean,
): Promise<(() => void) | undefined> {
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
		if (stopped()) return undefined
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
		const noise = REPEATED_NOISE.find((text) => plainLine.includes(text))
		if (
			!plainLine.trim() ||
			isNoisyProgressLine(plainLine) ||
			plainLine === previousLine ||
			(noise && seenRepeatedNoise.has(noise))
		)
			return
		if (noise) seenRepeatedNoise.add(noise)
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

/** Harmless warnings that repeat on every request or page load. Only the first of each is shown. */
const REPEATED_NOISE = [
	'Skipping origin header as it is a forbidden header',
	'if keeping the header is a desired behavior',
	"[TAURI] Couldn't find callback id",
	'This project is using Custom Message Compiler',
	'Game setting locales: options file read failed',
]

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
		windowsHide: true,
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
	if (!open.every(Boolean)) return false
	const serving = await Promise.all(
		runtime.scenarios.map((scenario) => appPageLoads(scenario.bridgeUrl!)),
	)
	return serving.every(Boolean)
}

/**
 * Whether the App's page loads through a bridge: the modules that start it (the entry, and the App
 * and routes it imports) and the dependencies they import. In a new checkout the frontend server
 * spends a minute or two preparing those, and holds the requests until it is done, so asking here
 * keeps that wait inside `up`.
 */
async function appPageLoads(bridgeUrl: string): Promise<boolean> {
	/** The file's text, or `undefined` when the server answered with an error. */
	const load = async (path: string) => {
		const response = await fetch(`${bridgeUrl}${path}`, { signal: AbortSignal.timeout(60_000) })
		const text = await response.text()
		if (response.status === 500) throw new PageError(path, text)
		return response.ok ? text : undefined
	}
	try {
		const modules = await Promise.all(['/src/main.js', '/src/App.vue', '/src/routes.js'].map(load))
		if (modules.some((text) => text === undefined)) return false
		const dependencies = new Set(
			modules.flatMap((text) =>
				[...text!.matchAll(/"(\/node_modules\/\.vite\/deps\/[^"]+)"/g)].map(([, path]) => path!),
			),
		)
		const loaded = await Promise.all([...dependencies].map(load))
		return loaded.every((text) => text !== undefined)
	} catch (error) {
		if (error instanceof PageError) throw error
		// Not answering yet, or still preparing after a minute. Asked again on the next check.
		return false
	}
}

/**
 * The App's page does not compile, so it will never load: `up` stops waiting and shows why. The
 * frontend server answers such a request with an error page that carries the compiler's error.
 */
class PageError extends DevRunnerError {
	constructor(path: string, page: string) {
		let detail = ''
		try {
			const error: unknown = JSON.parse(/const error = (\{.*\})/.exec(page)?.[1] ?? '')
			if (isRecord(error)) {
				detail = [error.message, error.frame].filter((part) => typeof part === 'string').join('\n')
			}
		} catch {
			// Not the frontend server's error page.
		}
		super(`The App's page does not compile (${path}).${detail ? `\n${detail}` : ''}`)
		this.name = 'PageError'
	}
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
	let ready = false
	let pageError: string | undefined
	try {
		ready = await runtimeIsReady(runtime)
	} catch (error) {
		if (!(error instanceof PageError)) throw error
		pageError = error.message
	}
	const labels = processLabelsForMode(runtime.mode, runtime.accountWeb)
	const state = pageError ? 'page does not compile' : ready ? 'ready' : 'starting'
	runnerLog('info', `${runtime.mode} PID ${runtime.pid} · ${state}`)
	if (pageError) runnerLog('error', pageError)
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

/**
 * Starts the dev stack in the background and returns once it is ready to use. With `preset`, this
 * checkout's data is replaced by that saved dataset first.
 */
async function up(paths: WorktreePaths, input: RunnerInput, preset?: string): Promise<void> {
	let started: NodeChildProcess.ChildProcess | undefined
	const live = readLiveRuntime(paths)
	if (preset) {
		if (live) {
			throw new DevRunnerError(
				'Already running. Run node scripts/dev-runner.ts down before switching data.',
			)
		}
		const source = presetPath(paths, preset)
		if (!NodeFS.existsSync(source)) {
			throw new DevRunnerError(
				`There is no preset ${preset}. node scripts/dev-runner.ts preset lists them.`,
			)
		}
		for (const entry of DATA_ENTRIES) {
			if (NodeFS.existsSync(NodePath.join(source, entry))) copyDataEntry(source, paths.data, entry)
			else NodeFS.rmSync(NodePath.join(paths.data, entry), { recursive: true, force: true })
		}
		runnerLog('info', `data from preset ${preset}`)
	}
	// An App decides whether to show its window when it starts, so asking a running one is too late.
	if (live?.hideWindows && !input.hideWindows) {
		runnerLog(
			'error',
			'Already running with App windows hidden. Run node scripts/dev-runner.ts down first.',
		)
		process.exitCode = 1
		return
	}
	const liveScenarios = live?.scenarios.map((scenario) => scenario.number).join(' ')
	if (live && (live.mode !== input.mode || liveScenarios !== input.scenarios.join(' '))) {
		runnerLog(
			'error',
			`Already running ${[live.mode, liveScenarios].filter(Boolean).join(' ')}. Run node scripts/dev-runner.ts down first.`,
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
		let ready: boolean
		try {
			ready = current !== undefined && (await runtimeIsReady(current))
		} catch (error) {
			if (!(error instanceof PageError)) throw error
			runnerLog(
				'error',
				`${error.message}\nThe stack is still running and picks up the fix by itself. Fix it, then run up again.`,
			)
			process.exitCode = 1
			return
		}
		if (ready) {
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
