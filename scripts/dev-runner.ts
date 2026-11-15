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
import { recordTiming } from './timings.ts'

const BASE_PORTS = {
	app: 1420,
	backend: 8787,
	accountWeb: 3100,
	core: 16662,
} as const
const MAX_HASH_OFFSET = 3000
const MAX_PORT = 65_535
// A program that exits on its own is started again, waiting twice as long each time. After this
// many exits inside the window it stays down until someone runs `rs <name>`.
const RESTART_FAILURE_LIMIT = 5
const RESTART_FAILURE_WINDOW_MS = 60_000
const RESTART_FIRST_WAIT_MS = 750
/** How many finished `stop` and `rs` requests the runtime file remembers. */
const ANSWERED_REQUESTS = 50
/** The stack a checkout starts with before any fixture names one. */
const DEFAULT_FIXTURE = 'default'
const PORT_PROBE_HOSTS = ['127.0.0.1', '::1'] as const
// Stays under the ten minute limit agent shells put on a foreground command.
const UP_WAIT_MS = 9 * 60_000
// One cached program per distinct set of Rust inputs. Older ones are deleted.
const CACHED_PROGRAMS = 6
const BUILD_LOCK_STALE_MS = 60 * 60_000
const BUILD_LOCK = 'building'
// In the shared build output: each repo crate's content when last compiled, and for which programs.
const BUILT_CRATES = 'built-crates.json'
// Given to an unchanged crate's files so Cargo trusts its compiled output.
const TRUSTED_FILE_TIME = new Date('2000-01-01T00:00:00Z')
const FETCH_BAD_PORTS = new Set([
	0, 1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95, 101, 102,
	103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143, 161, 179, 389, 427, 465,
	512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563, 587, 601, 636, 989, 990, 993,
	995, 1719, 1720, 1723, 2049, 3659, 4045, 4190, 5060, 5061, 6000, 6566, 6665, 6666, 6667, 6668,
	6669, 6679, 6697, 10080,
])

const RUNNER_FLAGS = ['--no-account-web', '--hide-windows', '--held', '--background']
export type DevPorts = {
	readonly app: number
	readonly backend: number
	readonly accountWeb: number
	/** The first Core's port. Any further Core takes a free one when it starts. */
	readonly core: number
}

type PortName = keyof DevPorts
type PortAvailabilityCheck = (port: number, hosts: ReadonlyArray<string>) => Promise<boolean>
export type WorktreePaths = {
	/** One folder per App, numbered by its place in the stack. */
	readonly appsData: string
	/** One folder per Core, numbered the same way. */
	readonly coresData: string
	readonly data: string
	readonly primary: string
	readonly runtime: string
	/** The Apps and Cores the last applied fixture declared. See [`StackShape`]. */
	readonly stack: string
	readonly worktree: string
}
/**
 * `.data/stack.json`: which Apps and Cores run. A fixture writes it when it is applied, and a
 * later start reads it, so the stack comes back the way the fixture left it.
 */
export type StackShape = {
	/** Each App is signed in to the account of the same name. */
	readonly apps: ReadonlyArray<{ readonly name: string }>
	readonly cores: number
	/** Changes with every apply, so whoever wrote the file can tell when the runner has read it. */
	readonly id: string
}
type RunnerInput = {
	/** The account sign-in website. Only a person signing in by hand needs it. */
	readonly accountWeb: boolean
	/** Start only what holds no data, and leave the backend, Cores and Apps to a fixture. */
	readonly held: boolean
	/** Apps run without a window on the desktop and are used from browser tabs. */
	readonly hideWindows: boolean
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
/** `cargo test` of some crates, compiled into the programs' build output the way they are. */
export type TestBuild = Omit<ProgramBuild, 'program'> & {
	/** The tested crates that are a program's own crate, by folder, with that program's inputs. */
	readonly programInputs: Readonly<Record<string, ReadonlyArray<string>>>
	/** The tested crates' folders, sorted, as `inputs` names them. */
	readonly tested: ReadonlyArray<string>
}
export type ProcessSpec = {
	readonly args: ReadonlyArray<string>
	/** Set when the process is a Rust program. Otherwise the arguments go to `vp`. */
	readonly build?: ProgramBuild
	readonly cwd: string
	readonly env: NodeJS.ProcessEnv
	readonly label: string
}
/** Where a dev App serves its local API. Picked once per runner, so `rs` keeps it. */
export type LocalAddress = {
	readonly port: number
	readonly secret: string
}
type RunningProcess = {
	readonly child: NodeChildProcess.ChildProcess
	readonly done: Promise<number>
	readonly label: string
}
type LogLevel = 'error' | 'info' | 'warning'
/**
 * `stopped` was asked for and stays that way until `rs <name>`. `failed` kept exiting or did not
 * build, and also waits for `rs <name>`.
 */
export type ProgramState = 'starting' | 'running' | 'stopped' | 'failed'
/** .data/runtime.json: what the running (or last) dev runner started and where to reach it. */
export type RuntimeFile = {
	/** The dev runner process. Absent once it has stopped. */
	pid?: number
	readonly branch: string
	readonly dataDir: string
	/** False when the account sign-in website was left out, so nothing waits for its port. */
	readonly accountWeb: boolean
	/** A process whose Rust build failed. `up` stops waiting when it sees one. */
	buildFailed?: string
	/** Whether the Apps were started without a window. `up --show-windows` cannot change it later. */
	readonly hideWindows: boolean
	readonly ports: DevPorts
	/** Every program by label, such as `backend`, `core:1` and `app:2`. */
	programs: Record<string, ProgramState>
	/**
	 * The `stop` and `rs` requests that carried an id and have finished, with the state they left
	 * the program in. The newest few are kept.
	 */
	requests: Record<string, ProgramState | 'unknown'>
	/** The [`StackShape`] the Apps and Cores below were started from. */
	stackId?: string
	apps: Array<{
		readonly dataDir: string
		readonly index: number
		/** The account it signs in to, and its tab's host. */
		readonly name: string
		/** Opens this App in a plain browser tab, on its own origin. */
		readonly tabUrl?: string
	}>
	cores: Array<{
		readonly dataDir: string
		readonly index: number
		/** Where its API answers, over TLS with its own certificate. */
		readonly url: string
	}>
	readonly source: string
	readonly urls: Record<Exclude<PortName, 'core'>, string>
}

export class DevRunnerError extends Error {
	constructor(message: string) {
		super(message)
		this.name = 'DevRunnerError'
	}
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
			throw new DevRunnerError(`PORT_OFFSET must be a non-negative integer; received ${explicit}.`)
		}
		return { offset: Number(explicit), source: `PORT_OFFSET=${explicit}` }
	}

	const instance = input.devInstance?.trim()
	if (instance) {
		if (/^\d+$/.test(instance)) {
			return { offset: Number(instance), source: `numeric DEV_INSTANCE=${instance}` }
		}
		return {
			offset: (stableHash(instance) % MAX_HASH_OFFSET) + 1,
			source: `hashed DEV_INSTANCE=${instance}`,
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
	readonly startOffset: number
}): Promise<number> {
	const requiredPorts = requiredPortNames(input.accountWeb)
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
	readonly ports: DevPorts
}): NodeJS.ProcessEnv {
	const env: NodeJS.ProcessEnv = { ...input.baseEnv }
	delete env.HOST
	delete env.PORT
	env.ACCOUNT_API_URL = `http://127.0.0.1:${input.ports.backend}`
	env.VITE_ACCOUNT_API_URL = env.ACCOUNT_API_URL
	env.ACCOUNT_WEB_URL = `http://127.0.0.1:${input.ports.accountWeb}`
	return env
}

/** That many free loopback ports. */
async function pickFreePorts(count: number): Promise<ReadonlyArray<number>> {
	// Every listener stays open until all ports are read, so no two picks get the same one.
	const servers = await Promise.all(
		Array.from(
			{ length: count },
			() =>
				new Promise<{ readonly port: number; readonly server: NodeNet.Server }>(
					(resolve, reject) => {
						const server = NodeNet.createServer()
						server.once('error', reject)
						server.listen({ host: '127.0.0.1', port: 0 }, () => {
							// Keeps no runner alive when another pick fails before these are closed.
							server.unref()
							const address = server.address()
							if (typeof address === 'object' && address) resolve({ port: address.port, server })
							else reject(new DevRunnerError('Could not pick a free local port.'))
						})
					},
				),
		),
	)
	for (const { server } of servers) server.close()
	return servers.map(({ port }) => port)
}

/** A free loopback port and a new secret for each App. */
async function pickLocalAddresses(count: number): Promise<ReadonlyArray<LocalAddress>> {
	return (await pickFreePorts(count)).map((port) => ({
		port,
		secret: NodeCrypto.randomBytes(32).toString('hex'),
	}))
}

/**
 * An App's tab: its own origin, named after its account, with the App's local API base in the
 * `#local=` fragment, where the frontend reads it.
 */
export function appTabUrl(name: string, vitePort: number, address: LocalAddress): string {
	const url = new URL(`http://${name.replaceAll('_', '-')}.localhost:${vitePort}`)
	url.hash = `local=${encodeURIComponent(`http://127.0.0.1:${address.port}/${address.secret}`)}`
	return url.href
}

/** The programs every stack runs. Its Cores and Apps come on top of these. */
export function sharedLabels(accountWeb = true): ReadonlyArray<string> {
	const labels = ['backend', 'account-web', 'app-frontend']
	return accountWeb ? labels : labels.filter((label) => label !== 'account-web')
}

/** The stack file, or `undefined` before any fixture was applied. */
export function readStackShape(path: string): StackShape | undefined {
	if (!NodeFS.existsSync(path)) return undefined
	let value: unknown
	try {
		value = JSON.parse(NodeFS.readFileSync(path, 'utf8'))
	} catch (error) {
		throw new DevRunnerError(`Could not read ${path}: ${String(error)}`)
	}
	if (
		!isRecord(value) ||
		!Array.isArray(value.apps) ||
		!value.apps.every((app) => isRecord(app) && typeof app.name === 'string') ||
		typeof value.cores !== 'number' ||
		typeof value.id !== 'string'
	) {
		throw new DevRunnerError(`${path} is not a stack file. Apply a fixture to write it again.`)
	}
	return { apps: value.apps as StackShape['apps'], cores: value.cores, id: value.id }
}

async function main(): Promise<void> {
	if (process.argv.includes('--background')) runInBackground()
	const paths = resolveWorktreePaths()
	const [command, ...commandArgs] = process.argv.slice(2)
	if (command === 'up' || command === 'fixture') {
		const args = commandArgs.filter((arg) => arg !== '--show-windows')
		const background = {
			// Apps sign themselves in and are used from browser tabs.
			accountWeb: false,
			hideWindows: !commandArgs.includes('--show-windows'),
		}
		if (command === 'fixture') {
			if (args.length > 1) throw new DevRunnerError('fixture takes one name or file.')
			return applyFixture(paths, args[0] ?? DEFAULT_FIXTURE, {
				...parseInput([]),
				...background,
			})
		}
		const input = { ...parseInput(args), ...background }
		// A checkout that never had a fixture applied has no data to start on.
		if (!NodeFS.existsSync(paths.stack) && !readLiveRuntime(paths)) {
			return applyFixture(paths, DEFAULT_FIXTURE, input)
		}
		return up(paths, input)
	}
	if (command === 'down') return down(paths)
	if (command === 'status') {
		if (!(await printStatus(paths))) process.exitCode = 1
		return
	}
	if (command === 'stop' || command === 'rs') return sendProgramCommand(paths, command, commandArgs)
	if (command === 'build') return buildPrograms(paths)
	if (command === 'test') return testCrates(paths, commandArgs)
	if (command !== 'dev') {
		throw new DevRunnerError(
			`Unknown command ${command ?? ''}. Expected fixture, up, dev, status, stop, rs, down, build or test.`,
		)
	}
	const input = parseInput(commandArgs)
	const { offset: startOffset, source } = resolveStartOffset({
		devInstance: process.env.DEV_INSTANCE,
		explicitOffset: process.env.PORT_OFFSET,
		primaryPath: paths.primary,
		worktreePath: paths.worktree,
	})
	const selectedOffset = await findFirstAvailableOffset({
		accountWeb: input.accountWeb,
		startOffset,
	})
	const ports = portsForOffset(selectedOffset)
	const branch = currentBranch(paths.worktree)
	const env = createRuntimeEnvironment({ baseEnv: readSharedEnv(paths), ports })
	const saved = readStackShape(paths.stack)
	// A stack with no data yet starts held, and the default fixture then fills it.
	const applyDefault = !input.held && !saved
	const secretPath = NodePath.join(paths.data, 'backend', 'dev-secret')
	NodeFS.mkdirSync(NodePath.dirname(secretPath), { recursive: true })
	if (!NodeFS.existsSync(secretPath))
		NodeFS.writeFileSync(secretPath, NodeCrypto.randomBytes(32).toString('hex'), { mode: 0o600 })
	env.LOCAL_DEV_SECRET = NodeFS.readFileSync(secretPath, 'utf8').trim()
	prepareLocalBackend(paths, env)
	// A fixture brings the backend's database with it.
	const started = input.held ? undefined : saved
	if (started) migrateLocalBackend(paths, env)
	const shared = createSharedSpecs({ accountWeb: input.accountWeb, env, paths, ports })
	printPlan({ branch, paths, ports, shape: started, source, specs: shared })

	const runtime: RuntimeFile = {
		pid: process.pid,
		branch,
		dataDir: paths.data,
		accountWeb: input.accountWeb,
		hideWindows: input.hideWindows,
		ports,
		programs: {},
		requests: {},
		apps: [],
		cores: [],
		source,
		urls: {
			backend: `http://127.0.0.1:${ports.backend}`,
			accountWeb: `http://127.0.0.1:${ports.accountWeb}`,
			app: `http://localhost:${ports.app}`,
		},
	}
	writeRuntimeFile(paths, runtime)
	const setBuildFailed = (label: string, failed: boolean) => {
		if ((runtime.buildFailed === label) === failed) return
		runtime.buildFailed = failed ? label : undefined
		writeRuntimeFile(paths, runtime)
	}
	const setState = (label: string, state: ProgramState | undefined) => {
		if (runtime.programs[label] === state) return
		if (state) runtime.programs[label] = state
		else delete runtime.programs[label]
		writeRuntimeFile(paths, runtime)
	}
	const specsByLabel = new Map(shared.map((spec) => [spec.label, spec]))
	for (const spec of shared) runtime.programs[spec.label] = 'stopped'
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
		setState(spec.label, 'starting')
		let program: string | undefined
		if (spec.build) {
			setBuildFailed(spec.label, false)
			program = await ensureProgram(paths.worktree, spec, spec.build, builds, () => stopping)
			if (stopping || starts.get(spec.label) !== token) return
			if (!program) {
				setBuildFailed(spec.label, true)
				setState(spec.label, 'failed')
				runnerLog('error', `${spec.label} did not build. Use rs ${spec.label} after fixing it.`)
				return
			}
		}
		if (spec.label.startsWith('app:')) {
			// The App loads its page once, so the frontend has to be serving before it starts.
			while (!(await portIsOpen(ports.app))) {
				await new Promise((resolve) => setTimeout(resolve, 250))
				if (stopping || starts.get(spec.label) !== token) return
			}
		}
		const running = spawnProcess(paths.worktree, spec, program)
		processes.set(spec.label, running)
		setState(spec.label, 'running')
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
	/** One program that cannot start never takes the others down. */
	const startProgram = (spec: ProcessSpec) =>
		start(spec).catch((error: unknown) => {
			runnerLog(
				'error',
				`${spec.label} failed: ${error instanceof Error ? error.message : String(error)}`,
			)
			setState(spec.label, 'failed')
		})
	/** Stops a program on purpose: it stays down until `rs`. Returns once it has exited. */
	const stopProgram = async (label: string) => {
		// Ends a start that is still building or waiting.
		starts.set(label, Symbol(label))
		const current = processes.get(label)
		processes.delete(label)
		if (current) {
			stopChild(current.child)
			await current.done.catch(() => undefined)
		}
		// Only now: whoever waits on this state is about to replace the program's files.
		if (specsByLabel.has(label)) setState(label, 'stopped')
	}
	/** Resolves once the old process is gone, with `started` for when the new one is up or has failed. */
	const restart = async (label: string) => {
		const spec = specsByLabel.get(label)
		if (!spec) {
			runnerLog(
				'warning',
				`Nothing here is named ${label}. Running: ${[...specsByLabel.keys()].join(', ')}`,
			)
			return undefined
		}
		await stopProgram(label)
		restartFailures.delete(label)
		runnerLog('info', `Starting ${label}...`)
		// Not waited for: a Rust program may build first, and the next command should not queue behind it.
		return { started: startProgram(spec) }
	}
	const handleProcessExit = async (spec: ProcessSpec, running: RunningProcess, code: number) => {
		// Stopped or replaced on purpose.
		if (processes.get(spec.label) !== running) return
		processes.delete(spec.label)
		if (stopping) return
		// A person closing an App's window asked for it to stop.
		if (spec.label.startsWith('app:') && code === 0) {
			setState(spec.label, 'stopped')
			runnerLog('info', `${spec.label} closed. Use rs ${spec.label} to start it again.`)
			return
		}
		const now = Date.now()
		const failures = [...(restartFailures.get(spec.label) ?? []), now].filter(
			(timestamp) => now - timestamp < RESTART_FAILURE_WINDOW_MS,
		)
		restartFailures.set(spec.label, failures)
		if (failures.length >= RESTART_FAILURE_LIMIT) {
			setState(spec.label, 'failed')
			runnerLog(
				'error',
				`${spec.label} exited ${RESTART_FAILURE_LIMIT} times in a minute and stays down. Use rs ${spec.label} after fixing it.`,
			)
			return
		}
		const wait = RESTART_FIRST_WAIT_MS * 2 ** (failures.length - 1)
		const token = Symbol(spec.label)
		starts.set(spec.label, token)
		setState(spec.label, 'starting')
		runnerLog('warning', `${spec.label} exited with code ${code}; starting it again in ${wait} ms.`)
		await new Promise((resolve) => setTimeout(resolve, wait))
		// A stop or a restart asked for meanwhile wins.
		if (!stopping && starts.get(spec.label) === token) await startProgram(spec)
	}

	/** Whether a program is one of the stack's Apps and Cores, which change with the stack file. */
	const isStackLabel = (label: string) => label.startsWith('app:') || label.startsWith('core:')
	/** Stops everything that holds data, so a fixture can replace that data. */
	const halt = async () => {
		const labels = [...specsByLabel.keys()].filter(
			(label) => label === 'backend' || isStackLabel(label),
		)
		await Promise.all(labels.map(stopProgram))
		runnerLog('info', 'Backend, Cores and Apps stopped for a fixture.')
	}
	/** Starts the Apps and Cores `shape` names, replacing the ones that ran before. */
	const startStack = async (shape: StackShape) => {
		await halt()
		for (const label of [...specsByLabel.keys()].filter(isStackLabel)) {
			specsByLabel.delete(label)
			setState(label, undefined)
		}
		const appAddresses = await pickLocalAddresses(shape.apps.length)
		const corePorts =
			shape.cores === 0 ? [] : [ports.core, ...(await pickFreePorts(shape.cores - 1))]
		const stack = createStackSpecs({
			appAddresses,
			branch,
			corePorts,
			env,
			hideWindows: input.hideWindows,
			paths,
			ports,
			shape,
		})
		for (const spec of stack) {
			specsByLabel.set(spec.label, spec)
			// Not `stopped`: readiness leaves a stopped program out, and these are about to start.
			runtime.programs[spec.label] = 'starting'
		}
		runtime.stackId = shape.id
		runtime.apps = shape.apps.map(({ name }, index) => ({
			dataDir: appDataPath(paths, index + 1),
			index: index + 1,
			name,
			tabUrl: appTabUrl(name, ports.app, appAddresses[index]!),
		}))
		runtime.cores = corePorts.map((port, index) => ({
			dataDir: coreDataPath(paths, index + 1),
			index: index + 1,
			url: `https://127.0.0.1:${port}`,
		}))
		for (const { dataDir } of [...runtime.apps, ...runtime.cores]) {
			NodeFS.mkdirSync(dataDir, { recursive: true })
		}
		writeRuntimeFile(paths, runtime)
		const backend = specsByLabel.get('backend')
		if (backend) {
			await startProgram(backend)
			// An App signs in once as it starts and a Core asks for its pairing code as it starts, so
			// the backend has to answer before either does.
			while (!stopping && !(await portIsOpen(ports.backend))) {
				if (runtime.programs.backend === 'failed') return
				await new Promise((resolve) => setTimeout(resolve, 100))
			}
		}
		if (runtime.stackId !== shape.id) return
		// One stopped on purpose while the backend came up stays down.
		for (const spec of stack) {
			if (runtime.programs[spec.label] === 'starting') void startProgram(spec)
		}
	}
	/** Reads the stack file a fixture just wrote and starts it. */
	const startSavedStack = async () => {
		const shape = readStackShape(paths.stack)
		if (!shape) {
			runnerLog('warning', `No stack to start: ${paths.stack} is missing.`)
			return
		}
		await startStack(shape)
	}
	/** `backend`, `core:2`, `app:1`, plus `core` for the first Core, a bare number or name for an App. */
	const labelFor = (name: string) => {
		if (specsByLabel.has(name)) return name
		if (name === 'core') return 'core:1'
		if (name === 'frontend') return 'app-frontend'
		const app = runtime.apps.find((app) => String(app.index) === name || app.name === name)
		return app ? `app:${app.index}` : name
	}
	const commands = createCommandInput({
		answered: (request, name) => {
			const kept = Object.entries(runtime.requests).slice(1 - ANSWERED_REQUESTS)
			const state = runtime.programs[labelFor(name)] ?? 'unknown'
			runtime.requests = Object.fromEntries([...kept, [request, state]])
			writeRuntimeFile(paths, runtime)
		},
		commandFile: NodePath.join(paths.data, 'dev-commands'),
		getCore: () => processes.get('core:1'),
		halt,
		restart: (name) => restart(labelFor(name)),
		startStack: startSavedStack,
		stop: () => stop(0),
		stopProgram: async (name) => {
			const label = labelFor(name)
			if (!specsByLabel.has(label)) {
				runnerLog(
					'warning',
					`Nothing here is named ${name}. Running: ${[...specsByLabel.keys()].join(', ')}`,
				)
				return
			}
			await stopProgram(label)
			runnerLog('info', `${label} stopped. Use rs ${label} to start it again.`)
		},
	})

	process.once('SIGINT', () => void stop(130))
	process.once('SIGTERM', () => void stop(143))

	try {
		for (const spec of shared) if (spec.label !== 'backend') void startProgram(spec)
		if (started) {
			void startStack(started)
		} else {
			runnerLog('info', 'Backend, Cores and Apps wait for a fixture.')
			if (applyDefault) applyDefaultFixture(paths, builds)
		}
		await finished
	} finally {
		await stop(process.exitCode || 1)
	}
}

/**
 * Fills a checkout that has no data yet. The runner in the foreground does this for itself; a
 * background `up` applies the fixture from its own process instead.
 */
function applyDefaultFixture(
	paths: WorktreePaths,
	children: Set<NodeChildProcess.ChildProcess>,
): void {
	const child = NodeChildProcess.spawn(
		process.execPath,
		[NodeURL.fileURLToPath(import.meta.url), 'fixture', DEFAULT_FIXTURE],
		{ cwd: paths.worktree, stdio: 'inherit', windowsHide: true },
	)
	children.add(child)
	child.once('close', () => children.delete(child))
}

export function parseInput(args: ReadonlyArray<string>): RunnerInput {
	const unknown = args.find((arg) => arg.startsWith('--') && !RUNNER_FLAGS.includes(arg))
	if (unknown) throw new DevRunnerError(`Unknown option ${unknown}.`)
	const extra = args.find((arg) => !arg.startsWith('--'))
	if (extra) {
		throw new DevRunnerError(
			`Unexpected ${extra}. A fixture names the Apps and Cores: vp run fixture <name>`,
		)
	}

	return {
		accountWeb: !args.includes('--no-account-web'),
		held: args.includes('--held'),
		hideWindows: args.includes('--hide-windows'),
	}
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
		appsData: NodePath.join(data, 'apps'),
		coresData: NodePath.join(data, 'cores'),
		data,
		primary: NodePath.resolve(primary),
		runtime: NodePath.join(data, 'runtime.json'),
		stack: NodePath.join(data, 'stack.json'),
		worktree: NodePath.resolve(worktree),
	}
}

/** Settings from the env files, overridden by the environment. */
function readSharedEnv(paths: WorktreePaths): NodeJS.ProcessEnv {
	return {
		...readEnv(NodePath.join(paths.worktree, 'packages', 'engine', '.env.prod')),
		...readEnv(NodePath.join(paths.worktree, 'packages', 'engine', '.env')),
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
	const specs = createStackSpecs({
		// Only the builds are used, so no App ever gets this address.
		appAddresses: [{ port: 0, secret: '' }],
		branch: currentBranch(paths.worktree),
		corePorts: [ports.core],
		env: createRuntimeEnvironment({ baseEnv: readSharedEnv(paths), ports }),
		hideWindows: true,
		paths,
		ports,
		shape: { apps: [{ name: 'alice' }], cores: 1, id: 'build' },
	})
	for (const spec of specs) {
		if (!spec.build) continue
		if (!(await ensureProgram(paths.worktree, spec, spec.build, new Set(), () => false))) {
			process.exitCode = 1
		}
	}
}

/**
 * `test <crate>...`: one `cargo test` of the crates in the shared build output, one build at a
 * time like the programs, so a checkout that runs tests never fills its own `target`.
 */
async function testCrates(paths: WorktreePaths, crates: ReadonlyArray<string>): Promise<void> {
	const known = new Set([...localCrates(paths.worktree).values()].map((crate) => crate.name))
	const unknown = crates.filter((crate) => !known.has(crate))
	if (crates.length === 0) throw new DevRunnerError('test needs the Cargo package names to test.')
	if (unknown.length > 0) {
		throw new DevRunnerError(`No workspace crate is named ${unknown.join(', ')}.`)
	}
	// As in CI, the env files reach the tests only through the engine's compiled settings.
	const env = { ...process.env, SQLX_OFFLINE: 'true' }
	const build = testBuild(paths, readSharedEnv(paths), crates)
	if (!(await runTests(paths, env, crates, build))) process.exitCode = 1
}

function testBuild(
	paths: WorktreePaths,
	env: NodeJS.ProcessEnv,
	crates: ReadonlyArray<string>,
): TestBuild {
	const checkout = NodeFS.realpathSync.native(paths.worktree)
	const tested = [...localCrates(paths.worktree).values()]
		.filter((crate) => crates.includes(crate.name))
		.map((crate) => ({
			folder: NodePath.relative(checkout, crate.folder).replaceAll('\\', '/'),
			name: crate.name,
		}))
	const programs = tested.flatMap(({ folder, name }) =>
		name === 'copal' || name === 'theseus_gui'
			? [[folder, programInputs(paths, name)] as const]
			: [],
	)
	return {
		// One run, so the tested crates share one feature set, as they do in a single cargo test.
		cargoArgs: ['test', '--locked', ...crates.flatMap((crate) => ['--package', crate])],
		cwd: paths.worktree,
		// The same compiled settings as the programs, so the crates they share stay compiled.
		...engineBuildSettings(paths, env),
		inputs: [
			...new Set([
				...crates.flatMap((crate) => workspaceInputs(paths, crate, { tests: true })),
				...programs.flatMap(([, inputs]) => inputs),
			]),
		],
		programInputs: Object.fromEntries(programs),
		tested: tested.map(({ folder }) => folder).sort(),
		...workspaceBuildDirs(paths),
	}
}

/** Whether the crates' tests compiled and passed. */
async function runTests(
	paths: WorktreePaths,
	env: NodeJS.ProcessEnv,
	crates: ReadonlyArray<string>,
	build: TestBuild,
): Promise<boolean> {
	const spec = { env, label: `test ${crates.join(' ')}` }
	const started = performance.now()
	const unlock = await lockBuilds(build.cacheDir, spec.label, () => false)
	if (!unlock) return false
	const lockWait = performance.now() - started
	let compiled: number | undefined
	let changedCrates = 0
	let result: 'ok' | 'failed' = 'failed'
	try {
		let key = programKey(paths.worktree, build)
		for (;;) {
			const run = await compileShared(
				paths.worktree,
				spec,
				build,
				new Set(),
				[...build.cargoArgs, '--no-run'],
				key,
			)
			changedCrates += run.changed
			if (!run.compiled) return false
			if (run.key === key) break
			key = run.key
		}
		compiled = performance.now()
		// Still locked: another checkout's build would replace these test programs.
		if (await runCargo(spec, build, new Set(), build.cargoArgs)) result = 'ok'
		// Cargo compiles what was edited since `--no-run` again, which the record does not describe.
		if (programKey(paths.worktree, build) !== key) {
			NodeFS.rmSync(NodePath.join(build.targetDir, BUILT_CRATES), { force: true })
		}
		return result === 'ok'
	} finally {
		unlock()
		const finished = performance.now()
		recordTiming(paths.primary, {
			kind: 'test',
			worktree: paths.worktree,
			crates,
			result,
			// Crates in the repo compiled again; third-party crates are not counted.
			changedCrates,
			ms: {
				lockWait: Math.round(lockWait),
				compile: Math.round((compiled ?? finished) - started - lockWait),
				...(compiled === undefined ? {} : { test: Math.round(finished - compiled) }),
				total: Math.round(finished - started),
			},
		})
	}
}

/** The programs every stack runs: the ones that are not a Core or an App. */
export function createSharedSpecs(input: {
	readonly accountWeb: boolean
	readonly env: NodeJS.ProcessEnv
	readonly paths: WorktreePaths
	readonly ports: DevPorts
}): ReadonlyArray<ProcessSpec> {
	const labels = sharedLabels(input.accountWeb)
	const backendUrl = `http://127.0.0.1:${input.ports.backend}`
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
	}
	return labels.map((label) => specs[label]!)
}

/** One program per Core and per App of `shape`, the Cores first. */
export function createStackSpecs(input: {
	/** One per App, in the stack's order. */
	readonly appAddresses: ReadonlyArray<LocalAddress>
	readonly branch: string
	/** One per Core, in the stack's order. */
	readonly corePorts: ReadonlyArray<number>
	readonly env: NodeJS.ProcessEnv
	readonly hideWindows: boolean
	readonly paths: WorktreePaths
	readonly ports: DevPorts
	readonly shape: StackShape
}): ReadonlyArray<ProcessSpec> {
	const coreDir = NodePath.join(input.paths.worktree, 'apps', 'core')
	const cores = input.corePorts.slice(0, input.shape.cores).map((port, index) => ({
		args: ['run'],
		build: {
			cargoArgs: ['build', '--package', 'copal'],
			cwd: coreDir,
			...engineBuildSettings(input.paths, input.env),
			inputs: programInputs(input.paths, 'copal'),
			program: programFile('copal'),
			...workspaceBuildDirs(input.paths),
		},
		cwd: coreDir,
		env: {
			...input.env,
			ALLOWED_ORIGIN: `http://localhost:${input.ports.app}`,
			BIND_HOST: '127.0.0.1',
			CORE_DATA_DIR: coreDataPath(input.paths, index + 1),
			PORT: String(port),
		},
		label: `core:${index + 1}`,
	}))
	const apps = input.shape.apps.map(({ name }, index) => {
		const dataDir = appDataPath(input.paths, index + 1)
		const address = input.appAddresses[index]!
		// Everything that differs between runs reaches the App here, so one build serves them all.
		const appDevConfig = {
			backendUrl: `http://127.0.0.1:${input.ports.backend}`,
			authMode: 'dev',
			branch: input.branch,
			credentialNamespace: `${input.branch}:app:${index + 1}`,
			dataDir,
			devUrl: `http://localhost:${input.ports.app}`,
			hideWindow: input.hideWindows,
			localPort: address.port,
			localSecret: address.secret,
			title: `Modrinth ${name} - ${input.branch}`,
			username: name,
		}
		return {
			args: ['--amberite-dev-config', JSON.stringify(appDevConfig)],
			build: appProgramBuild(input.paths, input.env),
			cwd: NodePath.join(input.paths.worktree, 'apps', 'app'),
			env: {
				...input.env,
				RUST_LOG: 'engine=warn,theseus_gui=warn,webview=off',
				THESEUS_CONFIG_DIR: dataDir,
				WEBVIEW2_USER_DATA_FOLDER: NodePath.join(dataDir, 'webview2'),
			},
			label: `app:${index + 1}`,
		}
	})
	return [...cores, ...apps]
}

function appProgramBuild(paths: WorktreePaths, env: NodeJS.ProcessEnv): ProgramBuild {
	const appDir = NodePath.join(paths.worktree, 'apps', 'app')
	const engine = engineBuildSettings(paths, env)
	return {
		// What `tauri dev` ran. Without `custom-protocol` the App loads its frontend from devUrl.
		cargoArgs: ['build', '--package', 'theseus_gui', '--no-default-features'],
		cwd: appDir,
		env: {
			...engine.env,
			TAURI_CONFIG: JSON.stringify(
				JSON.parse(NodeFS.readFileSync(NodePath.join(appDir, 'tauri-local.conf.json'), 'utf8')),
			),
		},
		ignoredInputs: engine.ignoredInputs,
		inputs: programInputs(paths, 'theseus_gui'),
		program: programFile('theseus_gui'),
		...workspaceBuildDirs(paths),
	}
}

/**
 * The engine's build passes these to the compiler: from its `.env`, or from the environment when
 * that file is missing. Every program that may build the engine takes them.
 */
function engineBuildSettings(
	paths: WorktreePaths,
	env: NodeJS.ProcessEnv,
): Pick<ProgramBuild, 'env' | 'ignoredInputs'> {
	const engineDir = NodePath.join(paths.worktree, 'packages', 'engine')
	const names = Object.keys({
		...readEnv(NodePath.join(engineDir, '.env.prod')),
		...readEnv(NodePath.join(engineDir, '.env')),
	})
	return {
		env: Object.fromEntries(
			names.flatMap((name) => {
				const value = env[name]
				return value === undefined ? [] : [[name, value] as const]
			}),
		),
		ignoredInputs: [NodePath.join(engineDir, '.env')],
	}
}

/** What the App's or Core's build reads. Its tests count the same files toward its own crate. */
function programInputs(
	paths: WorktreePaths,
	packageName: 'copal' | 'theseus_gui',
): ReadonlyArray<string> {
	const inputs = workspaceInputs(paths, packageName)
	// The App's version comes from the frontend.
	return packageName === 'theseus_gui' ? [...inputs, 'apps/app-frontend/package.json'] : inputs
}

/**
 * What a program in the root Cargo workspace (the App and Core) may read: the workspace settings
 * and the folders of `packageName` and every local crate it builds with. Crates it does not use,
 * such as labrinth, change without rebuilding it. When unsure whether a build reads something,
 * list it: a stale program is the worst outcome. Its tests also build its development-only crates.
 */
function workspaceInputs(
	paths: WorktreePaths,
	packageName: string,
	options?: { readonly tests: boolean },
): ReadonlyArray<string> {
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
	if (options?.tests) for (const dependency of root.devDependencies) visit(dependency)
	const checkout = NodeFS.realpathSync.native(paths.worktree)
	return [
		...settings,
		...[...folders].map((folder) => NodePath.relative(checkout, folder).replaceAll('\\', '/')),
	]
}

type LocalCrate = {
	/** Folders of the local crates it builds with. Development-only ones are left out. */
	readonly dependencies: ReadonlyArray<string>
	/** Folders of the local crates only its tests, examples and benchmarks build with. */
	readonly devDependencies: ReadonlyArray<string>
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
			const local = (dev: boolean) =>
				crate.dependencies.flatMap((dependency) =>
					dependency.path && (dependency.kind === 'dev') === dev ? [dependency.path] : [],
				)
			crates.set(folder, {
				dependencies: local(false),
				devDependencies: local(true),
				folder,
				name: crate.name,
			})
		}
	}
	localCratesResults.set(worktree, crates)
	return crates
}

/**
 * Every checkout compiles into one build output, like a pnpm store: each dependency is compiled
 * once and kept once. Builds already run one at a time. Only the runner compiles there, so
 * `BUILT_CRATES` describes everything in it; a person's own Cargo runs use `target` as usual.
 */
function workspaceBuildDirs(paths: WorktreePaths): Pick<ProgramBuild, 'cacheDir' | 'targetDir'> {
	const targetDir = NodePath.join(paths.primary, 'target', 'dev-shared')
	return { cacheDir: NodePath.join(targetDir, 'dev-bin'), targetDir }
}

function programFile(name: string): string {
	return process.platform === 'win32' ? `${name}.exe` : name
}

/**
 * Names a program by everything that goes into building it, wherever the checkout is. Two
 * checkouts with the same key can run the same program.
 */
export function programKey(
	worktree: string,
	build: Pick<ProgramBuild, 'cargoArgs' | 'env' | 'ignoredInputs' | 'inputs'>,
): string {
	const hash = NodeCrypto.createHash('sha256')
	const add = (...parts: ReadonlyArray<string>) => {
		for (const part of parts) hash.update(part).update('\0')
	}
	add(process.platform, process.arch, ...build.cargoArgs)
	add(JSON.stringify(Object.entries(build.env).sort(([left], [right]) => (left < right ? -1 : 1))))
	const contents = inputContents(worktree, build)
	for (const file of [...contents.keys()].sort()) add(file, contents.get(file)!)
	return hash.digest('hex').slice(0, 20)
}

/**
 * Every file the build reads, relative to the checkout with `/`, and its content as Git would
 * store it, so the same content gives the same hash whether it is committed, staged, edited, or new.
 */
function inputContents(
	worktree: string,
	build: Pick<ProgramBuild, 'ignoredInputs' | 'inputs'>,
): Map<string, string> {
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
	return contents
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
		const started = performance.now()
		const unlock = await lockBuilds(cache, spec.label, stopped)
		if (!unlock) return undefined
		const lockWait = performance.now() - started
		let changedCrates = 0
		let result: 'ok' | 'failed' = 'failed'
		try {
			// Another runner may have built it while this one waited.
			while (!NodeFS.existsSync(cached(key))) {
				runnerLog('info', `${spec.label} building ${build.program} (${key})`)
				const run = await compileShared(worktree, spec, build, builds, build.cargoArgs, key)
				changedCrates += run.changed
				if (!run.compiled) return undefined
				if (run.key === key) {
					NodeFS.mkdirSync(NodePath.dirname(cached(key)), { recursive: true })
					const partial = `${cached(key)}.${process.pid}.partial`
					NodeFS.copyFileSync(NodePath.join(build.targetDir, 'debug', build.program), partial)
					NodeFS.renameSync(partial, cached(key))
				}
				key = run.key
			}
			result = 'ok'
		} finally {
			unlock()
			const total = performance.now() - started
			// The shared build output is <primary>/target/dev-shared.
			recordTiming(NodePath.resolve(build.targetDir, '..', '..'), {
				kind: 'build',
				worktree,
				program: build.program,
				key,
				result,
				// Crates in the repo compiled again; third-party crates are not counted.
				changedCrates,
				ms: {
					lockWait: Math.round(lockWait),
					compile: Math.round(total - lockWait),
					total: Math.round(total),
				},
			})
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
 * One Cargo run into the shared build output, with the checkout's crates dated for it first and
 * recorded once it succeeds. An input edited during the run makes the output a mix, and Cargo may
 * have compiled crates the record calls unchanged, so the record is forgotten and the returned key
 * differs from `key`: the caller compiles again.
 */
async function compileShared(
	worktree: string,
	spec: Pick<ProcessSpec, 'env' | 'label'>,
	build: ProgramBuild | TestBuild,
	builds: Set<NodeChildProcess.ChildProcess>,
	args: ReadonlyArray<string>,
	key: string,
): Promise<{ readonly changed: number; readonly compiled: boolean; readonly key: string }> {
	const prepared = prepareCheckoutCrates(worktree, build)
	const compiled = await runCargo(spec, build, builds, args)
	const builtKey = programKey(worktree, build)
	if (builtKey !== key) {
		NodeFS.rmSync(NodePath.join(build.targetDir, BUILT_CRATES), { force: true })
	} else if (compiled) prepared.recordBuilt()
	return { changed: prepared.changed, compiled, key: builtKey }
}

type BuiltCrate = {
	readonly hash: string
	/**
	 * The builds whose output of it is from this content: program files, and `test <folder>...`
	 * for test runs. Named `programs` because checkouts on older branches share this record.
	 */
	readonly programs: ReadonlyArray<string>
	/** The checkout every listed test run of this crate compiled its own tests in. */
	readonly tested?: string
}

/**
 * Cargo decides whether a crate needs compiling from file times alone, and every checkout shares
 * one build output. So before a build, each crate in the repo is checked against the content it
 * was last compiled from for this build: a program, or one crate's tests. A crate that matches gets
 * old file times, so Cargo keeps its compiled output even though this checkout's files are newer.
 * A crate that differs, or was last compiled for other builds only, gets the current time, so Cargo
 * compiles it again from this checkout. Returns how many crates will compile, and what to record
 * once the build succeeds.
 *
 * A crate's own tests can hold their checkout's path (`env!("CARGO_MANIFEST_DIR")`), which Cargo
 * does not compare, so a test build trusts its tested crates only when they were compiled in this
 * checkout.
 */
export function prepareCheckoutCrates(
	worktree: string,
	build: ProgramBuild | TestBuild,
): { readonly changed: number; readonly recordBuilt: () => void } {
	const recordFile = NodePath.join(build.targetDir, BUILT_CRATES)
	let record: Record<string, BuiltCrate> = {}
	try {
		record = JSON.parse(NodeFS.readFileSync(recordFile, 'utf8'))
	} catch {
		// First build into this folder: every crate compiles.
	}
	const tested = 'tested' in build ? build.tested : undefined
	const buildName = 'tested' in build ? `test ${build.tested.join(' ')}` : build.program
	const folders = build.inputs.filter((input) =>
		NodeFS.existsSync(NodePath.join(worktree, input, 'Cargo.toml')),
	)
	const files = new Map<string, Array<[string, string]>>(folders.map((folder) => [folder, []]))
	// Inputs outside every crate folder, such as the App's version in `package.json`, count toward
	// the program's own crate (the first input folder), in its tests too, so the two keep trusting
	// each other's output. Cargo judges them through build scripts that recorded another checkout's
	// paths, so only their content can say they changed.
	const own = 'tested' in build ? build.programInputs : { [folders[0] ?? '']: build.inputs }
	const loose: Array<[string, string]> = []
	for (const [file, blob] of inputContents(worktree, build)) {
		const folder = folders
			.filter((candidate) => file.startsWith(`${candidate}/`))
			.sort((left, right) => right.length - left.length)[0]
		if (folder) files.get(folder)!.push([file, blob])
		else loose.push([file, blob])
	}
	const hashes = new Map<string, string>()
	const now = new Date()
	let changed = 0
	for (const [folder, entries] of files) {
		const hash = NodeCrypto.createHash('sha256')
		const inputs = own[folder] ?? []
		const hashed = [
			...entries,
			...loose.filter(([file]) =>
				inputs.some((input) => file === input || file.startsWith(`${input}/`)),
			),
		]
		for (const [file, blob] of hashed.sort(([left], [right]) => (left < right ? -1 : 1))) {
			hash.update(file).update('\0').update(blob).update('\0')
		}
		const digest = hash.digest('hex')
		hashes.set(folder, digest)
		const built = record[folder]
		const trusted =
			built?.hash === digest &&
			built.programs.includes(buildName) &&
			(!tested?.includes(folder) || built.tested === worktree)
		if (!trusted) {
			changed += 1
			// Compiling from other content may replace what other builds use, whether or not this
			// build succeeds. From the same content, only this crate's own tests change checkout.
			if (built?.hash !== digest) delete record[folder]
			else if (tested?.includes(folder) && built.tested !== worktree) {
				const testsOf = (name: string) =>
					name.startsWith('test ') && name.split(' ').includes(folder)
				record[folder] = { hash: digest, programs: built.programs.filter((name) => !testsOf(name)) }
			}
		}
		const time = trusted ? TRUSTED_FILE_TIME : now
		for (const [file] of entries) {
			try {
				NodeFS.utimesSync(NodePath.join(worktree, file), time, time)
			} catch {
				// Deleted since it was listed.
			}
		}
	}
	writeRecord(recordFile, record)
	const recordBuilt = () => {
		for (const [folder, hash] of hashes) {
			const built = record[folder]?.hash === hash ? record[folder] : undefined
			record[folder] = {
				hash,
				programs: [...new Set([...(built?.programs ?? []), buildName])],
				tested: tested?.includes(folder) ? worktree : built?.tested,
			}
		}
		writeRecord(recordFile, record)
	}
	return { changed, recordBuilt }
}

/** Written whole, then renamed into place, so a stopped runner never leaves half a record. */
function writeRecord(file: string, record: unknown): void {
	NodeFS.mkdirSync(NodePath.dirname(file), { recursive: true })
	const partial = `${file}.${process.pid}.partial`
	NodeFS.writeFileSync(partial, JSON.stringify(record, null, '\t'))
	NodeFS.renameSync(partial, file)
}

/** Whether Cargo succeeded. */
async function runCargo(
	spec: Pick<ProcessSpec, 'env' | 'label'>,
	build: Pick<ProgramBuild, 'cwd' | 'env' | 'targetDir'>,
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

/** Numbered by place in the stack, so the same folder serves whatever a fixture names its App. */
function appDataPath(paths: WorktreePaths, index: number): string {
	return NodePath.join(paths.appsData, String(index))
}

function coreDataPath(paths: WorktreePaths, index: number): string {
	return NodePath.join(paths.coresData, String(index))
}

function spawnProcess(
	worktree: string,
	spec: ProcessSpec,
	program: string | undefined,
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
	pipeOutput(child.stdout, spec.label, process.stdout)
	pipeOutput(child.stderr, spec.label, process.stderr)

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
 * send it `rs core:1`, `stop backend`, or `quit`. The file is emptied when the runner starts.
 * `halt` and `stack` are how a fixture swaps the data: stop what holds it, then start the stack
 * file it wrote.
 *
 * A `stop` or `rs` may end in `#<id>`. `answered` is called with that id once that very request
 * has finished, which is how its sender tells it from an earlier state or another restart.
 */
export function createCommandInput(input: {
	readonly answered: (request: string, name: string) => void
	readonly commandFile: string
	readonly getCore: () => RunningProcess | undefined
	readonly halt: () => Promise<void>
	/** Resolves once the old process is gone. `started` settles when the new one is up or failed. */
	readonly restart: (name: string) => Promise<{ readonly started: Promise<void> } | undefined>
	readonly startStack: () => Promise<void>
	readonly stop: () => Promise<void>
	readonly stopProgram: (name: string) => Promise<void>
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

	// One at a time, in the order they were sent: a fixture's `stack` must not start what its
	// `halt` is still stopping.
	let last = Promise.resolve()
	const inOrder = (work: () => Promise<void>) => {
		last = last.then(work).catch((error: unknown) => {
			runnerLog('error', error instanceof Error ? error.message : String(error))
		})
	}

	function handle(line: string) {
		const command = line.trim()
		if (!command) return
		if (command === 'quit') {
			void input.stop()
			return
		}
		if (command === 'halt') {
			inOrder(input.halt)
			return
		}
		if (command === 'stack') {
			inOrder(input.startStack)
			return
		}
		const program = command.match(/^(rs|stop)\s+(\S+)(?:\s+#(\S+))?$/i)
		if (program) {
			const name = program[2]!.toLowerCase()
			const request = program[3]
			const answer = () => request && input.answered(request, name)
			inOrder(async () => {
				if (program[1]!.toLowerCase() === 'stop') {
					await input.stopProgram(name)
					answer()
					return
				}
				const restart = await input.restart(name)
				// The next command runs while a Rust program builds; the answer waits for the start.
				void (restart?.started ?? Promise.resolve()).then(answer)
			})
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
			runnerLog('info', 'Commands: rs <name>, stop <name>, core <command>, quit')
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

function requiredPortNames(accountWeb = true): ReadonlyArray<PortName> {
	const names = ['app', 'backend', 'accountWeb', 'core'] as const
	return names.filter((name) => accountWeb || name !== 'accountWeb')
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
	const variables = `LOCAL_DEV=true\nLOCAL_DEV_SECRET=${env.LOCAL_DEV_SECRET}\nTICKET_KEY=${ticketKey}\n`
	NodeFS.writeFileSync(NodePath.join(paths.data, 'backend', '.dev.vars'), variables, {
		mode: 0o600,
	})
}

/** Brings the backend's database up to the current schema, creating it when it is missing. */
function migrateLocalBackend(paths: WorktreePaths, env: NodeJS.ProcessEnv): void {
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
			encoding: 'utf8',
			env,
			windowsHide: true,
		},
	)
	if (result.status !== 0) {
		throw new DevRunnerError(
			`Local account database migration failed.\n${result.stdout}${result.stderr}`,
		)
	}
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
		programs: {},
		requests: {},
		apps: runtime.apps.map((app) => ({ ...app, tabUrl: undefined })),
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

/** The ports the stack's programs listen on, by program label. */
function programPorts(runtime: RuntimeFile): ReadonlyMap<string, number> {
	const labels = sharedLabels(runtime.accountWeb)
	const shared = [
		['backend', runtime.ports.backend],
		['account-web', runtime.ports.accountWeb],
		['app-frontend', runtime.ports.app],
	] as const
	return new Map([
		...shared.filter(([label]) => labels.includes(label)),
		...runtime.cores.map((core) => [`core:${core.index}`, Number(new URL(core.url).port)] as const),
	])
}

/** Whether everything that is meant to run answers. A program stopped on purpose is left out. */
async function runtimeIsReady(runtime: RuntimeFile): Promise<boolean> {
	const expected = (label: string) => runtime.programs[label] !== 'stopped'
	const ports = [...programPorts(runtime)].filter(([label]) => expected(label))
	const open = await Promise.all(ports.map(([, port]) => portIsOpen(port)))
	if (!open.every(Boolean)) return false
	const apps = runtime.apps.filter((app) => expected(`app:${app.index}`))
	const answering = await Promise.all(apps.map((app) => appAnswers(app.tabUrl)))
	if (!answering.every(Boolean)) return false
	return apps.length === 0 || appPageLoads(runtime.urls.app)
}

/** Whether an App answers on the local API base its tab URL carries. */
async function appAnswers(tabUrl: string | undefined): Promise<boolean> {
	const base = tabUrl && new URLSearchParams(new URL(tabUrl).hash.slice(1)).get('local')
	if (!base) return false
	try {
		const response = await fetch(`${base}/health`, { signal: AbortSignal.timeout(2_000) })
		return response.ok
	} catch {
		return false
	}
}

/**
 * Whether the frontend serves the App's page: the modules that start it (the entry, and the App
 * and routes it imports) and the dependencies they import. In a new checkout the frontend server
 * spends a minute or two preparing those, and holds the requests until it is done, so asking here
 * keeps that wait inside `up`.
 */
async function appPageLoads(frontendUrl: string): Promise<boolean> {
	/** The file's text, or `undefined` when the server answered with an error. */
	const load = async (path: string) => {
		const response = await fetch(new URL(path, frontendUrl), {
			signal: AbortSignal.timeout(60_000),
		})
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
		runnerLog('info', 'not running. Start it with: vp run up')
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
	const labels = sharedLabels(runtime.accountWeb)
	const state = pageError ? 'page does not compile' : ready ? 'ready' : 'starting'
	runnerLog('info', `dev PID ${runtime.pid} · ${state}`)
	if (pageError) runnerLog('error', pageError)
	/** Says so when a program is not simply running. */
	const note = (label: string) => {
		const program = runtime.programs[label]
		return program && program !== 'running' ? ` · ${program}` : ''
	}
	if (labels.includes('backend')) {
		runnerLog('info', `backend ${runtime.urls.backend}${note('backend')}`)
	}
	if (labels.includes('account-web')) {
		runnerLog('info', `account-web ${runtime.urls.accountWeb}${note('account-web')}`)
	}
	for (const core of runtime.cores) {
		runnerLog('info', `core:${core.index} ${core.url}${note(`core:${core.index}`)}`)
	}
	for (const app of runtime.apps) {
		runnerLog('info', `app:${app.index} (${app.name})${note(`app:${app.index}`)}`)
		if (app.tabUrl) runnerLog('info', `  tab: ${app.tabUrl}`)
	}
	runnerLog('info', `log ${devLogPath(paths)}`)
	return ready
}

/** Where a command leaves its environment for the background runner it starts on Windows. */
function runnerEnvPath(worktree: string): string {
	return NodePath.join(worktree, '.data', 'runner-env.json')
}

/**
 * Starts a dev runner that outlives this command and writes to the dev log. Returns its PID.
 *
 * On Windows `vp run` puts its command in a job that kills every process in it when vp returns,
 * and a child cannot leave that job. So Windows itself is asked to create the runner, which makes
 * it nobody's child. Such a process gets a fresh environment, so this one's is left in a file.
 */
function startRunner(paths: WorktreePaths, input: RunnerInput): number {
	NodeFS.mkdirSync(paths.data, { recursive: true })
	const args = [
		NodeURL.fileURLToPath(import.meta.url),
		'dev',
		...(input.accountWeb ? [] : ['--no-account-web']),
		...(input.hideWindows ? ['--hide-windows'] : []),
		...(input.held ? ['--held'] : []),
		'--background',
	]
	let pid: number | undefined
	if (process.platform === 'win32') {
		NodeFS.writeFileSync(runnerEnvPath(paths.worktree), JSON.stringify(process.env), {
			mode: 0o600,
		})
		// `ShowWindow = 0` keeps its console off the desktop.
		const create = [
			'$startup = New-CimInstance -ClassName Win32_ProcessStartup -ClientOnly -Property @{ ShowWindow = [uint16]0 }',
			'(Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{ CommandLine = $env:RUNNER_COMMAND; CurrentDirectory = $env:RUNNER_DIRECTORY; ProcessStartupInformation = $startup }).ProcessId',
		].join('; ')
		const created = NodeChildProcess.spawnSync(
			'powershell.exe',
			['-NoProfile', '-Command', create],
			{
				encoding: 'utf8',
				env: {
					...process.env,
					RUNNER_COMMAND: [process.execPath, ...args].map((part) => `"${part}"`).join(' '),
					RUNNER_DIRECTORY: paths.worktree,
				},
				windowsHide: true,
			},
		)
		pid = Number(created.stdout.trim()) || undefined
		if (!pid) throw new DevRunnerError(`Could not start the dev runner. ${created.stderr.trim()}`)
	} else {
		const started = NodeChildProcess.spawn(process.execPath, args, {
			cwd: paths.worktree,
			detached: true,
			stdio: 'ignore',
		})
		started.unref()
		pid = started.pid
		if (!pid) throw new DevRunnerError('Could not start the dev runner.')
	}
	runnerLog('info', `started PID ${pid}`)
	return pid
}

/**
 * What a runner with no terminal does first: take the environment of the command that started
 * it, and write everything it and its programs print to the dev log.
 */
function runInBackground(): void {
	const env = runnerEnvPath(process.cwd())
	if (NodeFS.existsSync(env)) {
		Object.assign(process.env, JSON.parse(NodeFS.readFileSync(env, 'utf8')))
		NodeFS.rmSync(env)
	}
	NodeFS.mkdirSync(NodePath.join(process.cwd(), '.data'), { recursive: true })
	const log = NodeFS.openSync(NodePath.join(process.cwd(), '.data', 'dev.log'), 'w')
	const write = (chunk: string | Uint8Array) => {
		NodeFS.writeSync(log, chunk as string)
		return true
	}
	process.stdout.write = write as typeof process.stdout.write
	process.stderr.write = write as typeof process.stderr.write
}

/** Sends the running dev runner one line, the way `rs` and `stop` reach it. */
function sendRunnerCommand(paths: WorktreePaths, command: string): void {
	NodeFS.appendFileSync(NodePath.join(paths.data, 'dev-commands'), `${command}\n`)
}

/**
 * `fixture <name>`: the whole setup in one command. Starts the dev stack when none is running,
 * replaces its data with what the fixture builds, and prints where to reach it.
 */
async function applyFixture(paths: WorktreePaths, name: string, input: RunnerInput): Promise<void> {
	const source = (file: string) =>
		NodeURL.pathToFileURL(NodePath.join(paths.worktree, 'packages', 'fixtures', 'src', file)).href
	// First, so the kit's own imports of the contracts resolve.
	;(await import(source('packages.ts'))).resolvePackages()
	const fixtures = await import(source('run.ts'))
	const file: string = fixtures.resolveFixture(paths.worktree, name)
	if (!(await fixtures.checkTypes(paths.worktree, file))) {
		process.exitCode = 1
		return
	}
	const live = readLiveRuntime(paths)
	// An App decides whether to show its window when it starts, so asking a running one is too late.
	if (live?.hideWindows && !input.hideWindows) {
		throw new DevRunnerError('Already running with App windows hidden. Run vp run down first.')
	}
	const started = live ? undefined : startRunner(paths, { ...input, held: true })
	const waitStarted = performance.now()
	const record = (result: 'ok' | 'failed') =>
		recordTiming(paths.primary, {
			kind: 'fixture',
			worktree: paths.worktree,
			result,
			startedRunner: started !== undefined,
			fixture: NodePath.basename(file),
			ms: { total: Math.round(performance.now() - waitStarted) },
		})
	/** The runner this command talks to, or an error once it can no longer finish the job. */
	const runtime = (): RuntimeFile | undefined => {
		if (started && !processIsRunning(started)) {
			throw new DevRunnerError(`The dev runner stopped. See ${devLogPath(paths)}`)
		}
		const current = readLiveRuntime(paths)
		if (current?.buildFailed) {
			throw new DevRunnerError(`${current.buildFailed} did not build. See ${devLogPath(paths)}`)
		}
		return current && (!started || current.pid === started) ? current : undefined
	}
	try {
		while (!runtime()) await new Promise((resolve) => setTimeout(resolve, 200))
		await fixtures.runFixture({
			data: paths.data,
			file,
			primary: paths.primary,
			runtime,
			send: (command: string) => sendRunnerCommand(paths, command),
			worktree: paths.worktree,
		})
		const deadline = Date.now() + UP_WAIT_MS
		while (!(await runtimeIsReady(runtime()!))) {
			if (Date.now() > deadline) throw new DevRunnerError('The stack did not become ready.')
			await new Promise((resolve) => setTimeout(resolve, 500))
		}
	} catch (error) {
		record('failed')
		runnerLog('error', error instanceof Error ? error.message : String(error))
		runnerLog('info', 'The stack stays up as far as the fixture got. Applying again starts over.')
		process.exitCode = 1
		return
	}
	record('ok')
	await printStatus(paths)
	if (live) runnerLog('info', 'Every App restarted, so reload any tab that was open.')
}

/** `stop <name>` and `rs <name>`: asks the runner, then waits until the program is there. */
async function sendProgramCommand(
	paths: WorktreePaths,
	command: 'stop' | 'rs',
	args: ReadonlyArray<string>,
): Promise<void> {
	const [name] = args
	const before = readLiveRuntime(paths)
	if (!before) throw new DevRunnerError('not running. Start it with: vp run up')
	const known = Object.keys(before.programs)
	const app = before.apps.find((app) => String(app.index) === name || app.name === name)
	const label = app ? `app:${app.index}` : name === 'core' ? 'core:1' : (name ?? '')
	if (args.length !== 1 || !known.includes(label)) {
		throw new DevRunnerError(`${command} takes one of: ${known.join(', ')}`)
	}
	// The id marks this request: the program's state alone could be left from before it, or come
	// from a restart the runner made on its own.
	const request = NodeCrypto.randomUUID()
	sendRunnerCommand(paths, `${command} ${label} #${request}`)
	const wanted = command === 'stop' ? 'stopped' : 'running'
	const deadline = Date.now() + UP_WAIT_MS
	while (Date.now() < deadline) {
		const state = readLiveRuntime(paths)?.requests[request]
		if (state === wanted) {
			runnerLog('info', `${label} ${wanted}`)
			return
		}
		if (state) throw new DevRunnerError(`${label} is ${state}. See ${devLogPath(paths)}`)
		await new Promise((resolve) => setTimeout(resolve, 200))
	}
	throw new DevRunnerError(`${label} is still not ${wanted}. See ${devLogPath(paths)}`)
}

/**
 * Starts the dev stack in the background and returns once it is ready to use.
 */
async function up(paths: WorktreePaths, input: RunnerInput): Promise<void> {
	let started: number | undefined
	const live = readLiveRuntime(paths)
	// An App decides whether to show its window when it starts, so asking a running one is too late.
	if (live?.hideWindows && !input.hideWindows) {
		runnerLog('error', 'Already running with App windows hidden. Run vp run down first.')
		process.exitCode = 1
		return
	}
	if (!live) started = startRunner(paths, input)

	const waitStarted = performance.now()
	// How long an agent waited for a usable stack. `waiting` means up gave up before it was ready.
	const record = (result: 'ok' | 'failed' | 'waiting') =>
		recordTiming(paths.primary, {
			kind: 'up',
			worktree: paths.worktree,
			result,
			startedRunner: started !== undefined,
			ms: { total: Math.round(performance.now() - waitStarted) },
		})
	const deadline = Date.now() + UP_WAIT_MS
	while (Date.now() < deadline) {
		if (started && !processIsRunning(started)) {
			runnerLog('error', `The dev runner stopped while starting. See ${devLogPath(paths)}`)
			process.exitCode = 1
			record('failed')
			return
		}
		const runtime = readLiveRuntime(paths)
		// A runtime file left by an earlier run does not describe the runner started above.
		const current = runtime && (!started || runtime.pid === started) ? runtime : undefined
		if (current?.buildFailed) {
			runnerLog('error', `${current.buildFailed} did not build. See ${devLogPath(paths)}`)
			process.exitCode = 1
			record('failed')
			return
		}
		const failed = Object.entries(current?.programs ?? {}).find(([, state]) => state === 'failed')
		if (failed) {
			runnerLog('error', `${failed[0]} keeps exiting. See ${devLogPath(paths)}`)
			process.exitCode = 1
			record('failed')
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
			record('failed')
			return
		}
		if (ready) {
			await printStatus(paths)
			record('ok')
			return
		}
		await new Promise((resolve) => setTimeout(resolve, 2_000))
	}
	runnerLog(
		'warning',
		`Still starting. Run vp run up again to keep waiting. Log: ${devLogPath(paths)}`,
	)
	record('waiting')
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
	const ports = [...programPorts(runtime).values()]
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
	readonly paths: WorktreePaths
	readonly ports: DevPorts
	/** Absent when the Apps and Cores wait for a fixture. */
	readonly shape: StackShape | undefined
	readonly source: string
	readonly specs: ReadonlyArray<ProcessSpec>
}): void {
	runnerLog('info', input.branch)
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
	if (input.shape?.cores) runnerLog('info', `Cores ${input.shape.cores}`)
	if (input.shape?.apps.length) {
		runnerLog('info', `Apps ${input.shape.apps.map((app) => app.name).join(', ')}`)
	}
	runnerLog('info', `processes ${labels.join(', ')}`)
}

const isMain = process.argv[1] && samePath(process.argv[1], NodeURL.fileURLToPath(import.meta.url))
if (isMain) {
	main().catch((error: unknown) => {
		runnerLog('error', error instanceof Error ? error.message : String(error))
		process.exitCode = 1
	})
}
