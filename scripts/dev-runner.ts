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
import {
	type CargoTimings,
	recordTiming,
	summarizeCargoTimings,
	takeCargoTimings,
} from './timings.ts'

const BASE_PORTS = {
	app: 1420,
	backend: 8787,
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
const NOT_RUNNING = 'No stack is running. Start one with: vp run dev [fixture]'
const RUNNER_FLAGS = ['--hide-windows', '--held']
/** Set, to the runner's PID, on the fixture a runner applies as its first start. */
const FIRST_APPLY = 'DEV_RUNNER_FIRST_APPLY'
const PORT_PROBE_HOSTS = ['127.0.0.1', '::1'] as const
// Stays under the ten minute limit agent shells put on a foreground command.
const READY_WAIT_MS = 9 * 60_000
// One cached program per distinct set of Rust inputs. Older ones are deleted.
const CACHED_PROGRAMS = 6
// In a checkout's build output: the latest test runs that passed, by everything that went into them.
const PASSED_TESTS = 'passed-tests.json'
const PASSED_TEST_RUNS = 200
// In a checkout's build output: rewritten as each Cargo run starts, and one `<name>.<pid>.<id>`
// file per Cargo run in progress. Another checkout copies the output only while neither changes.
const CARGO_STARTED = 'cargo-started'
const CARGO_RUNNING = 'cargo-running'
// What a new build output takes from another: third-party crates, compiled the same anywhere.
const SEEDED_FOLDERS = ['deps', 'build', '.fingerprint']
const SEED_COPIES = 64
const FETCH_BAD_PORTS = new Set([
	0, 1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95, 101, 102,
	103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143, 161, 179, 389, 427, 465,
	512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563, 587, 601, 636, 989, 990, 993,
	995, 1719, 1720, 1723, 2049, 3659, 4045, 4190, 5060, 5061, 6000, 6566, 6665, 6666, 6667, 6668,
	6669, 6679, 6697, 10080,
])

export type DevPorts = {
	readonly app: number
	readonly backend: number
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
	/** Applied as the stack starts. Without one it starts on the checkout's data. */
	readonly fixture: string | undefined
	/** Start only what holds no data, and leave the backend, Cores and Apps to a fixture. */
	readonly held: boolean
	/** Apps run without a window on the desktop and are used from browser tabs. */
	readonly hideWindows: boolean
}
/**
 * Rust programs the runner builds itself, in one Cargo run. They are built once per distinct set
 * of inputs, copied into `<cacheDir>/<key>/`, and run from that copy. A checkout whose Rust matches
 * something already built starts without Cargo, and a running program never blocks a build.
 */
export type ProgramBuild = {
	/** Programs built by every checkout, shared. */
	readonly cacheDir: string
	readonly cargoArgs: ReadonlyArray<string>
	/** The checkout to build, where Cargo runs. */
	readonly cwd: string
	/** Settings the build reads. They change the program, so they are part of its key. */
	readonly env: Record<string, string>
	/** Gitignored files, as absolute paths, that the build reads. */
	readonly ignoredInputs: ReadonlyArray<string>
	/** Files and folders, relative to the checkout, that the build reads. */
	readonly inputs: ReadonlyArray<string>
	/** The files Cargo writes in `<targetDir>/debug`. */
	readonly programs: ReadonlyArray<string>
	/** This checkout's own build output. */
	readonly targetDir: string
}
/** `cargo test` of some crates, compiled into the programs' build output the way they are. */
export type TestBuild = Omit<ProgramBuild, 'programs'>
export type ProcessSpec = {
	readonly args: ReadonlyArray<string>
	/** Set when the process is a Rust program: the build and its file. Otherwise the arguments go to `vp`. */
	readonly program?: { readonly build: ProgramBuild; readonly file: string }
	readonly cwd: string
	readonly env: NodeJS.ProcessEnv
	readonly label: string
}
/** Where a dev App serves its local API. See [`appLocalAddresses`]. */
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
	/** A process whose Rust build failed. A fixture stops waiting when it sees one. */
	buildFailed?: string
	/**
	 * Set until the runner has built the programs and started its first stack's programs, or
	 * applied its first fixture. Until then nothing else changes the stack's data.
	 */
	starting?: boolean
	/** When the runner's process was created, as the OS reports it: a reused PID is not this runner. */
	readonly started?: string
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

/** The stack is still starting when a wait gives up: slow, such as a cold build, not failed. */
class StillStarting extends DevRunnerError {}

export function isBrowserAllowedPort(port: number): boolean {
	return !FETCH_BAD_PORTS.has(port)
}

export function portsForOffset(offset: number): DevPorts {
	return {
		app: BASE_PORTS.app + offset,
		backend: BASE_PORTS.backend + offset,
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
	readonly checkPort?: PortAvailabilityCheck
	readonly startOffset: number
}): Promise<number> {
	const requiredPorts = ['app', 'backend', 'core'] as const
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

/**
 * Each App's local address, kept in `.data/local-api.json` by the App's name, which its tab URL
 * carries, so the URL stays the same across restarts and fixtures. A kept port that is taken or
 * unusable is replaced, and so is an unusable secret.
 */
export async function appLocalAddresses(
	data: string,
	names: ReadonlyArray<string>,
): Promise<ReadonlyArray<LocalAddress>> {
	const file = NodePath.join(data, 'local-api.json')
	let kept: Record<string, unknown> = {}
	try {
		const value: unknown = JSON.parse(NodeFS.readFileSync(file, 'utf8'))
		if (isRecord(value)) kept = value
	} catch {
		// None kept yet.
	}
	const keptFor = (name: string) => {
		const entry = kept[name]
		if (!isRecord(entry)) return {}
		const { port, secret } = entry
		return {
			port:
				typeof port === 'number' &&
				Number.isInteger(port) &&
				port > 0 &&
				port <= MAX_PORT &&
				isBrowserAllowedPort(port)
					? port
					: undefined,
			secret: typeof secret === 'string' && /^[0-9a-f]{64}$/.test(secret) ? secret : undefined,
		}
	}
	const ports = await Promise.all(
		names.map(async (name, index) => {
			const { port } = keptFor(name)
			// Two Apps kept with one port: the later one gets a new port.
			const twice = names.slice(0, index).some((other) => keptFor(other).port === port)
			return port !== undefined && !twice && (await canListen(port, '127.0.0.1')) ? port : undefined
		}),
	)
	const missing = ports.filter((port) => port === undefined).length
	const taken = new Set(Object.keys(kept).map((name) => keptFor(name).port))
	const fresh =
		missing === 0
			? []
			: (await pickFreePorts(missing + taken.size)).filter((port) => !taken.has(port))
	const addresses = names.map((name, index) => ({
		port: ports[index] ?? fresh.pop()!,
		secret: keptFor(name).secret ?? NodeCrypto.randomBytes(32).toString('hex'),
	}))
	NodeFS.mkdirSync(data, { recursive: true })
	const updated = { ...kept, ...Object.fromEntries(names.map((name, i) => [name, addresses[i]])) }
	NodeFS.writeFileSync(file, JSON.stringify(updated), { mode: 0o600 })
	return addresses
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
	const paths = resolveWorktreePaths()
	const [command, ...commandArgs] = process.argv.slice(2)
	if (command === 'fixture') {
		if (commandArgs.length > 1) throw new DevRunnerError('fixture takes one name or file.')
		return applyFixture(paths, commandArgs[0] ?? DEFAULT_FIXTURE)
	}
	if (command === 'down') return down(paths)
	if (command === 'status') {
		if (commandArgs.some((arg) => arg !== '--wait')) {
			throw new DevRunnerError('status takes only --wait.')
		}
		if (commandArgs.includes('--wait')) await waitUntilReady(paths)
		if (!(await printStatus(paths))) process.exitCode = 1
		return
	}
	if (command === 'stop' || command === 'rs') return sendProgramCommand(paths, command, commandArgs)
	if (command === 'build') return buildPrograms(paths, commandArgs)
	if (command === 'test') return testCrates(paths, commandArgs)
	if (command === 'gen-ts') return generateTypeScript(paths, commandArgs)
	if (command !== 'dev') {
		throw new DevRunnerError(
			`Unknown command ${command ?? ''}. Expected dev, fixture, status, stop, rs, down, build, test or gen-ts.`,
		)
	}
	const input = parseInput(commandArgs)
	const live = readRunner(paths)
	if (live) {
		throw new DevRunnerError(
			`Already running as PID ${live.pid}. Apply a fixture to it with vp run fixture <name>, or stop it with vp run down.`,
		)
	}
	logToFile(paths)
	// No apply outlives its runner, so a turn still held is a leftover.
	NodeFS.rmSync(fixtureLockPath(paths.data), { force: true })
	const { offset: startOffset, source } = resolveStartOffset({
		devInstance: process.env.DEV_INSTANCE,
		explicitOffset: process.env.PORT_OFFSET,
		primaryPath: paths.primary,
		worktreePath: paths.worktree,
	})
	const selectedOffset = await findFirstAvailableOffset({ startOffset })
	const ports = portsForOffset(selectedOffset)
	const branch = currentBranch(paths.worktree)
	const env = createRuntimeEnvironment({ baseEnv: readSharedEnv(paths), ports })
	const saved = readStackShape(paths.stack)
	// A checkout with no data yet gets the default fixture. The stack waits for the fixture.
	const fixture = input.held ? undefined : (input.fixture ?? (saved ? undefined : DEFAULT_FIXTURE))
	const secretPath = NodePath.join(paths.data, 'backend', 'dev-secret')
	NodeFS.mkdirSync(NodePath.dirname(secretPath), { recursive: true })
	if (!NodeFS.existsSync(secretPath))
		NodeFS.writeFileSync(secretPath, NodeCrypto.randomBytes(32).toString('hex'), { mode: 0o600 })
	env.LOCAL_DEV_SECRET = NodeFS.readFileSync(secretPath, 'utf8').trim()
	prepareLocalBackend(paths, env)
	// A fixture brings the backend's database with it.
	const started = input.held || fixture ? undefined : saved
	if (started) migrateLocalBackend(paths, env)
	const shared = createSharedSpecs({ env, paths, ports })
	printPlan({ branch, paths, ports, shape: started, source, specs: shared })

	const runtime: RuntimeFile = {
		pid: process.pid,
		started: processStarted(process.pid),
		starting: true,
		branch,
		dataDir: paths.data,
		ports,
		programs: {},
		requests: {},
		apps: [],
		cores: [],
		source,
		urls: {
			backend: `http://127.0.0.1:${ports.backend}`,
			app: `http://localhost:${ports.app}`,
		},
	}
	/** Once stopping, only `stop` writes the file, so nothing left queued records a live stack. */
	const save = () => {
		if (!stopping) writeRuntimeFile(paths, runtime)
	}
	/** Both programs come from one build, so its latest result stands for either. */
	const setBuildFailed = (label: string | undefined) => {
		if (runtime.buildFailed === label) return
		runtime.buildFailed = label
		save()
	}
	const setState = (label: string, state: ProgramState | undefined) => {
		if (runtime.programs[label] === state) return
		if (state) runtime.programs[label] = state
		else delete runtime.programs[label]
		save()
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
	/** `ready`, when given, is awaited after the build and before the program runs: false skips it. */
	const start = async (spec: ProcessSpec, ready?: Promise<boolean>) => {
		if (stopping) return
		const token = Symbol(spec.label)
		starts.set(spec.label, token)
		setState(spec.label, 'starting')
		let program: string | undefined
		if (spec.program) {
			setBuildFailed(undefined)
			const built = await ensureProgram(
				paths.worktree,
				spec,
				spec.program.build,
				builds,
				() => stopping,
			)
			if (stopping || starts.get(spec.label) !== token) return
			program = built && NodePath.join(built, spec.program.file)
			if (!program) {
				setBuildFailed(spec.label)
				setState(spec.label, 'failed')
				runnerLog('error', `${spec.label} did not build. Use rs ${spec.label} after fixing it.`)
				return
			}
		}
		if (ready && !(await ready)) return
		if (spec.label.startsWith('app:')) {
			// The App loads its page once, so the frontend has to be serving before it starts.
			while (!(await portIsOpen(ports.app))) {
				await new Promise((resolve) => setTimeout(resolve, 250))
				if (stopping || starts.get(spec.label) !== token) return
			}
		}
		// A stop or a newer start can arrive during any wait above, the last port check included.
		if (stopping || starts.get(spec.label) !== token) return
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
	const startProgram = (spec: ProcessSpec, ready?: Promise<boolean>) =>
		start(spec, ready).catch((error: unknown) => {
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
		if (stopping) return undefined
		if (!spec) {
			runnerLog(
				'warning',
				`Nothing here is named ${label}. Running: ${[...specsByLabel.keys()].join(', ')}`,
			)
			return undefined
		}
		await stopProgram(label)
		if (stopping) return undefined
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
		setBuildFailed(undefined)
		runnerLog('info', 'Backend, Cores and Apps stopped for a fixture.')
	}
	/** Starts the Apps and Cores `shape` names, replacing the ones that ran before. */
	const startStack = async (shape: StackShape) => {
		await halt()
		if (stopping) return
		for (const label of [...specsByLabel.keys()].filter(isStackLabel)) {
			specsByLabel.delete(label)
			setState(label, undefined)
		}
		const appAddresses = await appLocalAddresses(
			paths.data,
			shape.apps.map(({ name }) => name),
		)
		const corePorts =
			shape.cores === 0 ? [] : [ports.core, ...(await pickFreePorts(shape.cores - 1))]
		if (stopping) return
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
		save()
		const backend = specsByLabel.get('backend')
		if (backend) await startProgram(backend)
		if (stopping) return
		// An App signs in once as it starts and a Core asks for its pairing code as it starts, so the
		// backend has to answer before either runs; their build goes on meanwhile. Not waited for
		// here: commands run one at a time, and `rs backend` is how a backend that will not start
		// is brought back. One stopped on purpose meanwhile stays down.
		const replaced = () => stopping || runtime.stackId !== shape.id
		const backendAnswers = (async () => {
			while (backend && !replaced() && !(await portIsOpen(ports.backend))) {
				await new Promise((resolve) => setTimeout(resolve, 100))
			}
			return !replaced()
		})()
		for (const spec of stack) void startProgram(spec, backendAnswers)
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
			save()
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
	// Only now: the command input empties the command file, which would erase a command sent sooner.
	writeRuntimeFile(paths, runtime)

	process.once('SIGINT', () => void stop(130))
	process.once('SIGTERM', () => void stop(143))
	process.once('SIGHUP', () => void stop(129))

	const begun = performance.now()
	/** Stops the stack, saying why it can never become ready. */
	const fail = (reason: string) => {
		if (stopping) return
		runnerLog('error', reason)
		void stop(1)
	}
	/** Prints where to reach the stack once it is ready. A fixture prints it when it is applied. */
	const announceReady = async () => {
		let shown: string | undefined
		for (;;) {
			if (stopping) return
			const failed =
				runtime.buildFailed ??
				Object.keys(runtime.programs).find((label) => runtime.programs[label] === 'failed')
			if (failed) return fail(`${failed} did not start, so the stack cannot become ready.`)
			try {
				if (await runtimeIsReady(runtime)) break
			} catch (error) {
				if (!(error instanceof PageError)) throw error
				// The frontend picks up a fix by itself.
				if (error.message !== shown) runnerLog('error', (shown = error.message))
			}
			await new Promise((resolve) => setTimeout(resolve, 500))
		}
		recordTiming(paths.primary, {
			kind: 'dev',
			worktree: paths.worktree,
			result: 'ok',
			ms: { total: Math.round(performance.now() - begun) },
		})
		await printStatus(paths)
	}
	/** The first start is done: readiness is asked of what runs now, and others may change it. */
	const startDone = () => {
		runtime.starting = undefined
		save()
	}

	try {
		for (const spec of shared) if (spec.label !== 'backend') void startProgram(spec)
		// A fixture waits nine minutes for a program, which a cold build outlasts, so build first.
		const built = await ensureProgram(
			paths.worktree,
			{ env, label: 'build' },
			programsBuild(paths, env),
			builds,
			() => stopping,
		)
		if (stopping) {
			// Stopped during the build: start nothing.
		} else if (!built) {
			fail('The App and Core did not build. Fix the error above and start dev again.')
		} else if (started) {
			void startStack(started)
				.then(() => {
					startDone()
					return announceReady()
				})
				.catch((error: unknown) => fail(error instanceof Error ? error.message : String(error)))
		} else {
			runnerLog('info', 'Backend, Cores and Apps wait for a fixture.')
			if (!fixture) startDone()
			else if ((await applyFirstFixture(paths, fixture, builds)) !== 0) {
				fail(`The ${fixture} fixture did not apply, so the stack cannot become ready.`)
			} else if (!stopping) {
				startDone()
				await printStatus(paths)
			}
		}
		await finished
	} finally {
		await stop(process.exitCode || 1)
	}
}

/**
 * Applies the runner's first fixture as `vp run fixture` would, in a child the runner stops when
 * it stops. Resolves with its exit code.
 */
function applyFirstFixture(
	paths: WorktreePaths,
	fixture: string,
	children: Set<NodeChildProcess.ChildProcess>,
): Promise<number> {
	const child = NodeChildProcess.spawn(
		process.execPath,
		[NodeURL.fileURLToPath(import.meta.url), 'fixture', fixture],
		{
			cwd: paths.worktree,
			env: { ...process.env, [FIRST_APPLY]: String(process.pid) },
			stdio: ['ignore', 'pipe', 'pipe'],
			windowsHide: true,
		},
	)
	// Through this process's own output, so the dev log has it too.
	child.stdout.on('data', (chunk: Buffer) => process.stdout.write(chunk))
	child.stderr.on('data', (chunk: Buffer) => process.stderr.write(chunk))
	children.add(child)
	return new Promise((resolve) => {
		child.once('error', () => resolve(1))
		child.once('close', (code) => {
			children.delete(child)
			resolve(code ?? 1)
		})
	})
}

/** `dev [fixture] [--hide-windows]`, plus `--held` for a command that seeds the stack itself. */
export function parseInput(args: ReadonlyArray<string>): RunnerInput {
	const unknown = args.find((arg) => arg.startsWith('--') && !RUNNER_FLAGS.includes(arg))
	if (unknown) throw new DevRunnerError(`Unknown option ${unknown}.`)
	const fixtures = args.filter((arg) => !arg.startsWith('--'))
	if (fixtures.length > 1) throw new DevRunnerError('dev takes one fixture name or file.')
	const held = args.includes('--held')
	if (held && fixtures.length > 0) throw new DevRunnerError('A held stack takes no fixture.')
	return { fixture: fixtures[0], held, hideWindows: args.includes('--hide-windows') }
}

/** Copies everything the runner and its programs print to `.data/dev.log`, for other commands to point at. */
function logToFile(paths: WorktreePaths): void {
	NodeFS.mkdirSync(paths.data, { recursive: true })
	let log: number | undefined = NodeFS.openSync(devLogPath(paths), 'w')
	for (const stream of [process.stdout, process.stderr]) {
		const write = stream.write
		stream.write = function (this: NodeJS.WriteStream, chunk: string | Uint8Array) {
			// A throw here would end the runner before it stops its programs, so the copy is dropped.
			try {
				if (log !== undefined) {
					NodeFS.writeSync(log, typeof chunk === 'string' ? Buffer.from(chunk) : chunk)
				}
			} catch {
				log = undefined
			}
			return Reflect.apply(write, this, arguments)
		} as typeof stream.write
	}
}

export function resolveWorktreePaths(): WorktreePaths {
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
		// Cores read server profiles from this checkout, so a change to them needs no push.
		AMBERITE_SERVER_META_URL: NodePath.join(paths.worktree, 'meta') + NodePath.sep,
		...process.env,
	}
}

/**
 * `build [--test <crate>]...`: compiles the App and Core for this checkout's current Rust without
 * starting anything, so a later `dev` finds them ready. `dev` runs it before it starts the Cores and Apps.
 * With `--test`, the crates are tested after it, failed or not.
 */
async function buildPrograms(paths: WorktreePaths, args: ReadonlyArray<string>): Promise<void> {
	const tests = args.filter((_, index) => index % 2 === 1)
	if (args.some((arg, index) => index % 2 === 0 && arg !== '--test') || args.length % 2 === 1) {
		throw new DevRunnerError('build takes only `--test <crate>` pairs.')
	}
	await buildProgramsOnce(paths)
	if (tests.length > 0) await testCrates(paths, tests)
}

async function buildProgramsOnce(paths: WorktreePaths): Promise<void> {
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
	const spec = specs.find((candidate) => candidate.program)!
	if (!(await ensureProgram(paths.worktree, spec, spec.program!.build, new Set(), () => false))) {
		process.exitCode = 1
	}
}

/** `test <crate>...`: one `cargo test` of the crates in this checkout's build output. */
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

/**
 * `gen-ts [--check]`: writes the TypeScript the Rust crates generate, by running only their
 * `ts_is_up_to_date` tests, so it does not wait on the rest passing. Later crates import the
 * earlier ones' files, hence the order. `--check` only compares.
 */
async function generateTypeScript(
	paths: WorktreePaths,
	args: ReadonlyArray<string>,
): Promise<void> {
	if (args.some((arg) => arg !== '--check')) throw new DevRunnerError('gen-ts takes only --check.')
	const check = args.includes('--check')
	const { WRITE_TS: _, ...shell } = process.env
	const env = { ...shell, SQLX_OFFLINE: 'true', ...(check ? {} : { WRITE_TS: '1' }) }
	for (const crate of ['contracts', 'engine', 'amberite-app']) {
		const build = testBuild(paths, readSharedEnv(paths), [crate])
		if (!(await runTests(paths, env, [crate], build, ['ts_is_up_to_date']))) {
			process.exitCode = 1
			if (!check) return
		}
	}
}

export function testBuild(
	paths: WorktreePaths,
	env: NodeJS.ProcessEnv,
	crates: ReadonlyArray<string>,
): TestBuild {
	const tested = [...localCrates(paths.worktree).values()].filter((crate) =>
		crates.includes(crate.name),
	)
	const { cargoArgs: _, programs: __, ...shared } = programsBuild(paths, env)
	const targets = tested.flatMap((crate) => crate.testTargets)
	return {
		...shared,
		// The programs' packages and features, so every crate the tests do not test is the one the
		// programs compiled. Target flags then pick only the tested crates' tests. Their own default
		// features come back on, except the App's, which the programs leave off.
		cargoArgs: [
			'test',
			'--locked',
			...PROGRAM_PACKAGES,
			...crates.flatMap((crate) => (PROGRAM_PACKAGES.includes(crate) ? [] : ['--package', crate])),
			'--no-default-features',
			...tested.flatMap((crate) =>
				crate.features.includes('default') && crate.name !== 'amberite-app'
					? ['--features', `${crate.name}/default`]
					: [],
			),
			// Their tests check their generated TypeScript, as in CI, where the App's feature turns
			// it on for every crate. Only the tested crates take it, so the rest stay as compiled.
			...tested.flatMap((crate) =>
				crate.features.includes('export-ts') ? ['--features', `${crate.name}/export-ts`] : [],
			),
			// Cargo takes `--lib` once, for every selected package.
			...(targets.includes('--lib') ? ['--lib'] : []),
			...targets.filter((target) => target !== '--lib'),
		],
		inputs: [
			...new Set([
				...crates.flatMap((crate) => workspaceInputs(paths, crate, { tests: true })),
				...crates.flatMap((crate) =>
					crate === 'copal' || crate === 'amberite-app' ? programInputs(paths, crate) : [],
				),
			]),
		],
	}
}

/** Whether the crates' tests compiled and passed. `filter` names the tests to run, all when empty. */
async function runTests(
	paths: WorktreePaths,
	env: NodeJS.ProcessEnv,
	crates: ReadonlyArray<string>,
	build: TestBuild,
	filter: ReadonlyArray<string> = [],
): Promise<boolean> {
	const spec = { env, label: `test ${crates.join(' ')}` }
	const started = performance.now()
	const key = programKey(paths.worktree, build)
	// The tests see this environment too, so a pass counts for it alone.
	const passKey = (programs: string) =>
		NodeCrypto.createHash('sha256')
			.update(programs)
			.update(JSON.stringify(Object.entries(env).sort(([a], [b]) => (a < b ? -1 : 1))))
			.digest('hex')
			.slice(0, 20)
	const passedFile = NodePath.join(build.targetDir, PASSED_TESTS)
	const passed = (): ReadonlyArray<string> => {
		try {
			const record: unknown = JSON.parse(NodeFS.readFileSync(passedFile, 'utf8'))
			return Array.isArray(record) ? record.filter((key) => typeof key === 'string') : []
		} catch {
			return []
		}
	}
	// A filtered run passes for its tests alone, so it neither counts as a pass nor takes one.
	if (filter.length === 0 && passed().includes(passKey(key))) {
		runnerLog('info', `${spec.label} passed already with these inputs (${passKey(key)})`)
		recordTiming(paths.primary, {
			kind: 'test',
			worktree: paths.worktree,
			crates,
			result: 'ok',
			passedBefore: true,
			ms: { total: Math.round(performance.now() - started) },
		})
		return true
	}
	let compiled: number | undefined
	let timings: CargoTimings | undefined
	let result: 'ok' | 'failed' = 'failed'
	try {
		const built = await runCargo(spec, build, new Set(), [
			...build.cargoArgs,
			'--no-run',
			'--timings',
		])
		timings = takeCargoTimings(build.targetDir)
		if (!built) return false
		compiled = performance.now()
		const run = filter.length > 0 ? [...build.cargoArgs, '--', ...filter] : build.cargoArgs
		if (await runCargo(spec, build, new Set(), run)) result = 'ok'
		// An input edited during the run may not be what was tested.
		if (result === 'ok' && filter.length === 0 && programKey(paths.worktree, build) === key) {
			try {
				writeRecord(passedFile, [passKey(key), ...passed()].slice(0, PASSED_TEST_RUNS))
			} catch (error) {
				runnerLog(
					'warning',
					`${spec.label} passed, but the pass was not recorded: ${String(error)}`,
				)
			}
		}
		return result === 'ok'
	} finally {
		const finished = performance.now()
		recordTiming(paths.primary, {
			kind: 'test',
			worktree: paths.worktree,
			crates,
			result,
			...compiledDetails(paths.worktree, timings ? [timings] : []),
			ms: {
				compile: Math.round((compiled ?? finished) - started),
				...(compiled === undefined ? {} : { test: Math.round(finished - compiled) }),
				total: Math.round(finished - started),
			},
		})
	}
}

/** The programs every stack runs beside its Cores and Apps. */
export function createSharedSpecs(input: {
	readonly env: NodeJS.ProcessEnv
	readonly paths: WorktreePaths
	readonly ports: DevPorts
}): ReadonlyArray<ProcessSpec> {
	return [
		{
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
		{
			args: ['dev', '--port', String(input.ports.app), '--strictPort'],
			cwd: NodePath.join(input.paths.worktree, 'apps', 'app-frontend'),
			env: input.env,
			label: 'app-frontend',
		},
	]
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
	let programs: ProgramBuild | undefined
	const program = (name: 'copal' | 'amberite-app') => ({
		build: (programs ??= programsBuild(input.paths, input.env)),
		file: programFile(name),
	})
	const coreDir = NodePath.join(input.paths.worktree, 'apps', 'core')
	const cores = input.corePorts.slice(0, input.shape.cores).map((port, index) => ({
		args: ['run'],
		program: program('copal'),
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
			program: program('amberite-app'),
			cwd: NodePath.join(input.paths.worktree, 'apps', 'app'),
			env: {
				...input.env,
				RUST_LOG: 'engine=warn,amberite_app=warn,webview=off',
				AMBERITE_CONFIG_DIR: dataDir,
				WEBVIEW2_USER_DATA_FOLDER: NodePath.join(dataDir, 'webview2'),
			},
			label: `app:${index + 1}`,
		}
	})
	return [...cores, ...apps]
}

const PROGRAM_PACKAGES = ['--package', 'amberite-app', '--package', 'copal']

/**
 * The App and Core, in one Cargo run. Built apart, Cargo picks dependency features for each on its
 * own and compiles about 250 crates twice, the engine and contracts among them.
 */
function programsBuild(paths: WorktreePaths, env: NodeJS.ProcessEnv): ProgramBuild {
	const appDir = NodePath.join(paths.worktree, 'apps', 'app')
	const engine = engineBuildSettings(paths, env)
	return {
		// What `tauri dev` ran. Without `custom-protocol` the App loads its frontend from devUrl.
		// Core has no default features.
		// `--examples` builds none, but turns on what dev-dependencies add to features, as a test
		// run does, so tests compile no second copy of anything the programs use.
		cargoArgs: ['build', ...PROGRAM_PACKAGES, '--no-default-features', '--bins', '--examples'],
		cwd: paths.worktree,
		env: {
			...engine.env,
			TAURI_CONFIG: JSON.stringify(
				JSON.parse(NodeFS.readFileSync(NodePath.join(appDir, 'tauri-local.conf.json'), 'utf8')),
			),
		},
		ignoredInputs: engine.ignoredInputs,
		inputs: [
			...new Set([...programInputs(paths, 'amberite-app'), ...programInputs(paths, 'copal')]),
		],
		programs: [programFile('amberite-app'), programFile('copal')],
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
	packageName: 'copal' | 'amberite-app',
): ReadonlyArray<string> {
	const inputs = workspaceInputs(paths, packageName)
	// The App's version comes from the frontend.
	return packageName === 'amberite-app' ? [...inputs, 'apps/app-frontend/package.json'] : inputs
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
	/** The features it declares. */
	readonly features: ReadonlyArray<string>
	readonly folder: string
	readonly name: string
	/** Cargo target flags that select its tests: its library, programs and integration tests. */
	readonly testTargets: ReadonlyArray<string>
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
				features: Readonly<Record<string, unknown>>
				manifest_path: string
				name: string
				targets: ReadonlyArray<{ kind: ReadonlyArray<string>; name: string }>
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
				features: Object.keys(crate.features),
				folder,
				name: crate.name,
				testTargets: crate.targets.flatMap((target) =>
					target.kind.includes('lib')
						? ['--lib']
						: target.kind.includes('bin')
							? ['--bin', target.name]
							: target.kind.includes('test')
								? ['--test', target.name]
								: [],
				),
			})
		}
	}
	localCratesResults.set(worktree, crates)
	return crates
}

/**
 * Each checkout compiles into its own `target/dev`, so builds never wait on each other or replace
 * each other's output; `seedBuildOutput` starts it with another checkout's third-party crates.
 * Finished programs go to one folder every checkout shares.
 */
function workspaceBuildDirs(paths: WorktreePaths): Pick<ProgramBuild, 'cacheDir' | 'targetDir'> {
	return {
		cacheDir: NodePath.join(paths.primary, 'target', 'dev-bin'),
		targetDir: NodePath.join(paths.worktree, 'target', 'dev'),
	}
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

/** Builds in progress in this runner by key, so the App and Core wait on one Cargo run. */
const programBuilds = new Map<string, Promise<string | undefined>>()

/**
 * The folder holding this build's programs for its current inputs, built first when there is
 * none. `undefined` when the build failed or the runner is stopping.
 */
function ensureProgram(
	worktree: string,
	spec: Pick<ProcessSpec, 'env' | 'label'>,
	build: ProgramBuild,
	builds: Set<NodeChildProcess.ChildProcess>,
	stopped: () => boolean,
): Promise<string | undefined> {
	const key = programKey(worktree, build)
	const running = programBuilds.get(key)
	if (running) return running
	const built = buildProgram(worktree, spec, build, builds, stopped, key).finally(() =>
		programBuilds.delete(key),
	)
	programBuilds.set(key, built)
	return built
}

async function buildProgram(
	worktree: string,
	spec: Pick<ProcessSpec, 'env' | 'label'>,
	build: ProgramBuild,
	builds: Set<NodeChildProcess.ChildProcess>,
	stopped: () => boolean,
	initialKey: string,
): Promise<string | undefined> {
	const cache = build.cacheDir
	const cached = (key: string) => NodePath.join(cache, key)
	const complete = (key: string) =>
		build.programs.every((program) => NodeFS.existsSync(NodePath.join(cached(key), program)))
	let key = initialKey
	if (complete(key)) {
		runnerLog('info', `${spec.label} using ${cached(key)}`)
	} else {
		const started = performance.now()
		const timings: CargoTimings[] = []
		let result: 'ok' | 'failed' = 'failed'
		try {
			while (!complete(key)) {
				if (stopped()) return undefined
				runnerLog('info', `${spec.label} building ${build.programs.join(', ')} (${key})`)
				const built = await runCargo(spec, build, builds, [...build.cargoArgs, '--timings'])
				const report = takeCargoTimings(build.targetDir)
				if (report) timings.push(report)
				if (!built) return undefined
				const builtKey = programKey(worktree, build)
				// An input edited during the run makes the output a mix, so it compiles again.
				if (builtKey === key) saveProgram(build, cached(key))
				key = builtKey
			}
			result = 'ok'
		} finally {
			const total = performance.now() - started
			// The shared program folder is <primary>/target/dev-bin.
			recordTiming(NodePath.resolve(cache, '..', '..'), {
				kind: 'build',
				worktree,
				program: build.programs.join(' '),
				key,
				result,
				...compiledDetails(worktree, timings),
				ms: { total: Math.round(total) },
			})
		}
	}
	const used = new Date()
	try {
		NodeFS.utimesSync(cached(key), used, used)
	} catch {
		// Pruned by another checkout this instant: starting fails, and the next start builds it again.
	}
	// Other checkouts prune this folder too.
	const keys = NodeFS.readdirSync(cache)
		.flatMap((entry) => {
			try {
				return [{ entry, at: NodeFS.statSync(NodePath.join(cache, entry)).mtimeMs }]
			} catch {
				return []
			}
		})
		.sort((left, right) => right.at - left.at)
	for (const { entry } of keys.slice(CACHED_PROGRAMS).filter(({ entry }) => entry !== key)) {
		try {
			NodeFS.rmSync(NodePath.join(cache, entry), { recursive: true })
		} catch {
			// Still running in another checkout.
		}
	}
	return cached(key)
}

/** What a build or test run's timing line says about what compiled, from Cargo's `--timings`. */
function compiledDetails(worktree: string, timings: ReadonlyArray<CargoTimings>) {
	const names = new Set([...localCrates(worktree).values()].map((crate) => crate.name))
	return { branch: currentBranch(worktree), ...summarizeCargoTimings(timings, names) }
}

/**
 * Copies the built programs into `folder`. Another checkout with the same inputs may have saved
 * them first and be running them, and Windows cannot replace a running program, so a program
 * already there is kept.
 */
function saveProgram(build: ProgramBuild, folder: string): void {
	NodeFS.mkdirSync(folder, { recursive: true })
	for (const program of build.programs) {
		const target = NodePath.join(folder, program)
		if (NodeFS.existsSync(target)) continue
		const partial = `${target}.${process.pid}.partial`
		NodeFS.copyFileSync(NodePath.join(build.targetDir, 'debug', program), partial)
		try {
			NodeFS.renameSync(partial, target)
		} catch (error) {
			NodeFS.rmSync(partial, { force: true })
			if (!NodeFS.existsSync(target)) throw error
		}
	}
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
	if (!NodeFS.existsSync(NodePath.join(build.targetDir, 'debug'))) {
		const ours = new Set([...localCrates(build.cwd).values()].map((crate) => crate.name))
		await seedBuildOutput(build.cwd, build.targetDir, ours, spec.label)
	}
	NodeFS.mkdirSync(build.targetDir, { recursive: true })
	cargoRunning(build.targetDir)
	// The marker first: a copy that read the old `CARGO_STARTED` then sees the new one.
	const running = NodePath.join(
		build.targetDir,
		`${CARGO_RUNNING}.${process.pid}.${NodeCrypto.randomUUID()}`,
	)
	NodeFS.writeFileSync(running, '')
	try {
		NodeFS.writeFileSync(NodePath.join(build.targetDir, CARGO_STARTED), NodeCrypto.randomUUID())
		const child = NodeChildProcess.spawn('cargo', args, {
			cwd: build.cwd,
			detached: process.platform !== 'win32',
			env: cargoEnv({ ...spec.env, ...build.env, CARGO_TARGET_DIR: build.targetDir }),
			stdio: ['ignore', 'pipe', 'pipe'],
			windowsHide: true,
		})
		builds.add(child)
		try {
			// Cargo can outlive this process when it is killed, and keeps writing.
			NodeFS.writeFileSync(running, String(child.pid ?? ''))
		} catch {
			// The marker still names this process, which waits for Cargo below.
		}
		// Compiles started by any checkout run behind whatever the person is doing. Windows gives
		// the compiler processes Cargo starts the same priority.
		try {
			if (child.pid) NodeOS.setPriority(child.pid, NodeOS.constants.priority.PRIORITY_BELOW_NORMAL)
		} catch {
			// Already finished.
		}
		pipeOutput(child.stdout, spec.label, process.stdout)
		pipeOutput(child.stderr, spec.label, process.stderr)
		const code = await new Promise<number>((resolve) => {
			child.once('error', (error) => {
				runnerLog('error', `Could not start cargo: ${error.message}`)
				resolve(1)
			})
			child.once('close', (exitCode) => resolve(exitCode ?? 1))
		})
		builds.delete(child)
		return code === 0
	} finally {
		try {
			NodeFS.rmSync(running, { force: true })
		} catch {
			// Held open for a moment, as Windows does: `cargoRunning` removes it later.
		}
	}
}

/**
 * Starts a new build output from the third-party crates of the checkout that ran Cargo last, so
 * only this repo's crates compile. On a ReFS Dev Drive each copy is a block clone that takes no
 * disk space until one side changes. A source that runs Cargo during the copy may be half written,
 * so that copy is dropped and the next source tried. With none, Cargo compiles everything.
 */
export async function seedBuildOutput(
	worktree: string,
	targetDir: string,
	/** This repo's crates, which compile from `worktree` and are never taken from another. */
	ours: ReadonlySet<string>,
	label: string,
): Promise<void> {
	// Unknown when the manifests do not read, and their output elsewhere is from other files.
	if (ours.size === 0) return
	// tauri's build script lists the files in its output folder by absolute path, so it runs again
	// for this folder instead of pointing into the source.
	const compiledHere = new Set([...ours, 'tauri'])
	const sources = git(['worktree', 'list', '--porcelain'], worktree)
		.split(/\r?\n/)
		.filter((line) => line.startsWith('worktree '))
		.map((line) => NodePath.join(line.slice('worktree '.length), 'target', 'dev'))
		.filter((source) => !samePath(source, targetDir))
		.flatMap((source) => {
			try {
				return [{ source, at: NodeFS.statSync(NodePath.join(source, CARGO_STARTED)).mtimeMs }]
			} catch {
				return []
			}
		})
		.sort((left, right) => right.at - left.at)
	const debug = NodePath.join(targetDir, 'debug')
	for (const { source } of sources) {
		const seeding = NodePath.join(targetDir, `seeding.${process.pid}.${NodeCrypto.randomUUID()}`)
		const before = idleRun(source)
		if (!before || !NodeFS.existsSync(NodePath.join(source, 'debug', '.fingerprint'))) continue
		const started = performance.now()
		try {
			await copyBuildOutput(NodePath.join(source, 'debug'), seeding, compiledHere)
			if (idleRun(source) !== before) {
				runnerLog('info', `${label} dropped a copy of ${source}, which ran Cargo meanwhile`)
				continue
			}
			NodeFS.renameSync(seeding, debug)
			runnerLog(
				'info',
				`${label} started its build output from ${source} in ${Math.round((performance.now() - started) / 1000)}s`,
			)
			return
		} catch (error) {
			// Another runner in this checkout seeded it first.
			if (NodeFS.existsSync(debug)) return
			runnerLog('warning', `${label} could not copy ${source}: ${String(error)}`)
		} finally {
			try {
				NodeFS.rmSync(seeding, { force: true, recursive: true })
			} catch {
				// Held open for a moment, as Windows does: `cargoRunning` removes it later.
			}
		}
	}
}

/** The latest Cargo run in a build output, while none runs there. */
function idleRun(targetDir: string): string | undefined {
	try {
		// Read before the markers, the reverse of the order `runCargo` writes them.
		const run = NodeFS.readFileSync(NodePath.join(targetDir, CARGO_STARTED), 'utf8')
		return cargoRunning(targetDir) ? undefined : run
	} catch {
		return undefined
	}
}

/**
 * Whether a Cargo run is in progress in a build output, by its runner's or Cargo's process. Removes
 * what killed runners left: their markers, which a reused process ID would keep alive, and copies.
 */
function cargoRunning(targetDir: string): boolean {
	let running = false
	for (const entry of NodeFS.readdirSync(targetDir)) {
		const [name, pid] = entry.split('.')
		if (name !== CARGO_RUNNING && name !== 'seeding') continue
		const file = NodePath.join(targetDir, entry)
		const pids = [Number(pid)]
		try {
			if (name === CARGO_RUNNING) pids.push(Number(NodeFS.readFileSync(file, 'utf8')))
		} catch {
			// Removed this instant.
		}
		if (pids.some((alive) => alive > 0 && processIsRunning(alive))) {
			running ||= name === CARGO_RUNNING
			continue
		}
		try {
			NodeFS.rmSync(file, { force: true, recursive: true })
		} catch {
			// Held open for a moment, as Windows does: a later run removes it.
		}
	}
	return running
}

/**
 * Copies the third-party part of `from` (a build output's `debug`), several files at a time,
 * leaving out the crates in `skipped` and files Cargo is writing or removing.
 */
async function copyBuildOutput(
	from: string,
	to: string,
	skipped: ReadonlySet<string>,
): Promise<void> {
	const files: Array<readonly [string, string]> = []
	const walk = (source: string, target: string, top: boolean) => {
		NodeFS.mkdirSync(target, { recursive: true })
		for (const entry of NodeFS.readdirSync(source, { withFileTypes: true })) {
			// The compiler's own scratch files and folders.
			if (entry.name.startsWith('.tmp') || entry.name.startsWith('rmeta')) continue
			// `build` and `.fingerprint` hold a `<crate>-<hash>` folder per crate.
			if (top && skipped.has(/^(.+)-[0-9a-f]{16}$/.exec(entry.name)?.[1] ?? '')) continue
			const pair = [NodePath.join(source, entry.name), NodePath.join(target, entry.name)] as const
			if (entry.isDirectory()) walk(...pair, false)
			else if (entry.isFile()) files.push(pair)
		}
	}
	for (const folder of SEEDED_FOLDERS) {
		if (!NodeFS.existsSync(NodePath.join(from, folder))) continue
		walk(NodePath.join(from, folder), NodePath.join(to, folder), folder !== 'deps')
	}
	let next = 0
	let failed = false
	const copy = async () => {
		while (!failed && next < files.length) {
			const [source, target] = files[next++]!
			try {
				// Windows keeps the file's times, which Cargo compares; elsewhere they are copied over.
				await NodeFS.promises.copyFile(source, target, NodeFS.constants.COPYFILE_FICLONE)
				if (process.platform !== 'win32') {
					const { atime, mtime } = await NodeFS.promises.stat(source)
					await NodeFS.promises.utimes(target, atime, mtime)
				}
			} catch (error) {
				if (isRecord(error) && error.code === 'ENOENT') continue
				failed = true
				throw error
			}
		}
	}
	// Every copy stops before the caller removes the folder.
	const copies = await Promise.allSettled(Array.from({ length: SEED_COPIES }, copy))
	const rejected = copies.find((copy) => copy.status === 'rejected')
	if (rejected) throw rejected.reason
}

/**
 * What Cargo runs with. `RUSTC_BOOTSTRAP` changes every crate's fingerprint, so one shell that sets
 * it would compile everything again, and the next shell without it again.
 */
function cargoEnv(env: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
	const { RUSTC_BOOTSTRAP: _, ...rest } = env
	return rest
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
	rewrite: (line: string) => string = (line) => line,
): void {
	if (!stream) return
	const lines = NodeReadline.createInterface({ input: stream })
	let previousLine = ''
	const seenRepeatedNoise = new Set<string>()
	lines.on('line', (raw) => {
		const line = rewrite(raw)
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
 * Commands come from stdin and from `.data/dev-commands`: another terminal, or an agent whose
 * background task holds the runner, cannot type into its stdin, so appending a line to that file
 * is the way to send it `rs core:1`, `stop backend`, or `quit`. The file is emptied when the
 * runner starts. `halt` and `stack` are how a fixture swaps the data: stop what holds it, then
 * start the stack file it wrote.
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
	const read = () => {
		let buffer: Buffer
		// The file can vanish or be replaced at any point; skip this read and take it on the next.
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
		} catch {
			return
		}
		// A command is appended in one write, so a read never finds half of one.
		offset += buffer.length
		for (const line of buffer.toString('utf8').split(/\r?\n/)) handle(line)
	}
	// The watcher answers a command at once; the poll catches anything it misses.
	const poll = setInterval(read, 500)
	poll.unref()
	let watcher: NodeFS.FSWatcher | undefined
	try {
		watcher = NodeFS.watch(input.commandFile, read)
		watcher.on('error', () => watcher?.close())
		watcher.unref()
	} catch {
		// The poll alone still reads every command.
	}

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
			watcher?.close()
			commands.close()
		},
	}
}

function isNoisyProgressLine(line: string): boolean {
	return (
		/^\s*Finished\s+/.test(line) ||
		/^\s*(?:Building|Checking|Fetch)\s+\[[= >-]+\]/.test(line) ||
		// The timing log keeps what the report said, then removes it.
		/^\s*Timing report saved to /.test(line) ||
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
	// Asking wrangler takes about 4 seconds even with nothing to apply. The stamp sits in the
	// storage folder, so it goes wherever that folder's data goes.
	const migrations = NodePath.join(paths.worktree, 'apps', 'backend', 'migrations')
	const stampPath = NodePath.join(paths.data, 'backend', 'v3', 'migrations.stamp')
	const stamp = NodeCrypto.createHash('sha256')
	for (const file of NodeFS.readdirSync(migrations).sort()) {
		stamp
			.update(file)
			.update('\0')
			.update(NodeFS.readFileSync(NodePath.join(migrations, file)))
	}
	const applied = stamp.digest('hex')
	if (NodeFS.existsSync(stampPath) && NodeFS.readFileSync(stampPath, 'utf8') === applied) return
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
	NodeFS.writeFileSync(stampPath, applied)
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

/**
 * The live runtime file, once its PID is checked to be the runner that wrote it: a runner killed
 * outright leaves its PID behind, and Windows hands PIDs out again, to other runners too. The
 * check is slow, so ask it once.
 */
function readRunner(paths: WorktreePaths): RuntimeFile | undefined {
	const runtime = readLiveRuntime(paths)
	return runtime?.pid && runtime.started && processStarted(runtime.pid) === runtime.started
		? runtime
		: undefined
}

function processIsRunning(pid: number): boolean {
	try {
		process.kill(pid, 0)
		return true
	} catch (error) {
		// Running, as another user or elevated.
		return isRecord(error) && error.code === 'EPERM'
	}
}

/** When the process with `pid` was created, as the OS reports it, or `undefined` when none runs. */
function processStarted(pid: number): string | undefined {
	const result =
		process.platform === 'win32'
			? NodeChildProcess.spawnSync(
					'powershell.exe',
					[
						'-NoProfile',
						'-Command',
						`(Get-CimInstance Win32_Process -Filter "ProcessId=${pid}").CreationDate.ToFileTimeUtc()`,
					],
					{ encoding: 'utf8', windowsHide: true },
				)
			: NodeChildProcess.spawnSync('ps', ['-o', 'lstart=', '-p', String(pid)], { encoding: 'utf8' })
	return (result.status === 0 && result.stdout.trim()) || undefined
}

/** The ports the stack's programs listen on, by program label. */
function programPorts(runtime: RuntimeFile): ReadonlyMap<string, number> {
	return new Map<string, number>([
		['backend', runtime.ports.backend],
		['app-frontend', runtime.ports.app],
		...runtime.cores.map((core) => [`core:${core.index}`, Number(new URL(core.url).port)] as const),
	])
}

/** Whether everything that is meant to run answers. A program stopped on purpose is left out. */
async function runtimeIsReady(runtime: RuntimeFile): Promise<boolean> {
	const expected = (label: string) => runtime.programs[label] !== 'stopped'
	const ports = [...programPorts(runtime)].filter(([label]) => expected(label))
	// Read before the first wait: the runner asks this of its own record, which a fixture replaces.
	const apps = runtime.apps.filter((app) => expected(`app:${app.index}`))
	const open = await Promise.all(ports.map(([, port]) => portIsOpen(port)))
	if (!open.every(Boolean)) return false
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
 * keeps that wait inside the command that waits for the stack.
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
 * The App's page does not compile, so it will never load: whoever waits for the stack stops and shows why. The
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
	const runtime = readRunner(paths)
	if (!runtime) {
		runnerLog('info', NOT_RUNNING)
		return false
	}
	let ready = false
	let pageError: string | undefined
	try {
		ready = !runtime.starting && (await runtimeIsReady(runtime))
	} catch (error) {
		if (!(error instanceof PageError)) throw error
		pageError = error.message
	}
	const state = pageError ? 'page does not compile' : ready ? 'ready' : 'starting'
	runnerLog('info', `dev PID ${runtime.pid} · ${state}`)
	if (pageError) runnerLog('error', pageError)
	/** Says so when a program is not simply running. */
	const note = (label: string) => {
		const program = runtime.programs[label]
		return program && program !== 'running' ? ` · ${program}` : ''
	}
	runnerLog('info', `backend ${runtime.urls.backend}${note('backend')}`)
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

/** `status --wait`: waits until the stack that is starting is ready, or says why it is not. */
async function waitUntilReady(paths: WorktreePaths): Promise<void> {
	// A `dev` started a moment ago has not written its runtime file yet, and the file it replaces
	// can name a PID that is now another process's.
	const deadline = Date.now() + 30_000
	while (!readRunner(paths)) {
		if (Date.now() > deadline) throw new DevRunnerError(NOT_RUNNING)
		await new Promise((resolve) => setTimeout(resolve, 500))
	}
	const stack = connectStack(paths)
	const until = Date.now() + READY_WAIT_MS
	try {
		await untilStarted(stack.runner.runtime, until)
		await stack.ready(until)
	} catch (error) {
		if (!(error instanceof StillStarting)) throw error
		runnerLog('warning', `${error.message} Run vp run status --wait again to keep waiting.`)
	}
}

/**
 * Waits until no other fixture is being applied to this checkout's stack, then holds the turn
 * until the returned function is called: two applies at once would replace each other's data.
 */
export async function fixtureTurn(data: string): Promise<() => void> {
	const lock = fixtureLockPath(data)
	const holder = () => {
		try {
			return NodeFS.readFileSync(lock, 'utf8')
		} catch {
			return undefined
		}
	}
	const mine = String(process.pid)
	let told = false
	for (;;) {
		try {
			NodeFS.writeFileSync(lock, mine, { flag: 'wx' })
			// `dev` and `down` clear a turn left by a killed apply, and another may hold it by then.
			return () => {
				if (holder() === mine) NodeFS.rmSync(lock, { force: true })
			}
		} catch (error) {
			if (!(error instanceof Error && 'code' in error && error.code === 'EEXIST')) throw error
		}
		const seen = holder()
		// Empty while its writer is still writing it. Read again before removing it: another waiter
		// may already have taken the turn over.
		if (seen && !processIsRunning(Number(seen))) {
			if (holder() === seen) NodeFS.rmSync(lock, { force: true })
			continue
		}
		if (seen && !told) {
			runnerLog('info', `Another fixture is being applied (PID ${seen}). This one waits for it.`)
			told = true
		}
		await new Promise((resolve) => setTimeout(resolve, 500))
	}
}

function fixtureLockPath(data: string): string {
	return NodePath.join(data, 'fixture.lock')
}

/**
 * Waits until the stack's first start is done, failing at `deadline`. Until then the runner
 * starts its own stack, which would replace any data put in before it.
 */
async function untilStarted(
	runtime: () => RuntimeFile | undefined,
	deadline = Date.now() + READY_WAIT_MS,
): Promise<void> {
	let told = false
	for (;;) {
		// The runner rewrites its file as it works, so one read can come back empty.
		const current = runtime()
		if (current && !current.starting) return
		if (current && !told) {
			runnerLog('info', 'Waiting for the stack to finish starting.')
			told = true
		}
		if (Date.now() > deadline) throw new StillStarting('The stack is still starting.')
		await new Promise((resolve) => setTimeout(resolve, 500))
	}
}

/** Sends the running dev runner one line, the way `rs` and `stop` reach it. */
function sendRunnerCommand(paths: WorktreePaths, command: string): void {
	NodeFS.appendFileSync(NodePath.join(paths.data, 'dev-commands'), `${command}\n`)
}

/**
 * The running stack, or one started for this command and stopped by `release`, so a command that
 * needs a stack leaves none behind. A stack started here waits for the command's fixture.
 */
export async function holdStack(
	paths: WorktreePaths,
): Promise<{ readonly release: () => Promise<void> }> {
	if (readRunner(paths)) {
		await untilStarted(connectStack(paths).runner.runtime)
		return { release: async () => {} }
	}
	const child = NodeChildProcess.spawn(
		process.execPath,
		[NodeURL.fileURLToPath(import.meta.url), 'dev', '--held', '--hide-windows'],
		{ cwd: paths.worktree, stdio: 'ignore', windowsHide: true },
	)
	let running = true
	const exited = new Promise<void>((resolve) =>
		child.once('close', () => {
			running = false
			resolve()
		}),
	)
	for (;;) {
		const runtime = readLiveRuntime(paths)
		if (runtime && runtime.pid === child.pid && !runtime.starting) break
		if (!running) throw new DevRunnerError(`The dev stack stopped. See ${devLogPath(paths)}`)
		await new Promise((resolve) => setTimeout(resolve, 200))
	}
	runnerLog('info', `Started a stack for this run. It stops when the run ends.`)
	return {
		release: async () => {
			// Once it has exited, the command file is another runner's.
			if (running) sendRunnerCommand(paths, 'quit')
			await exited
		},
	}
}

/**
 * The running stack, which a fixture or a scenario fills. `runner` is what the fixture package's
 * helpers talk to it through.
 */
export function connectStack(paths: WorktreePaths) {
	const live = readRunner(paths)
	if (!live?.pid) throw new DevRunnerError(NOT_RUNNING)
	const pid = live.pid
	/** The runner this command talks to, or an error once it can no longer finish the job. */
	const runtime = (): RuntimeFile | undefined => {
		if (!processIsRunning(pid)) {
			throw new DevRunnerError(`The dev stack stopped. See ${devLogPath(paths)}`)
		}
		const current = readLiveRuntime(paths)
		if (current?.buildFailed) {
			throw new DevRunnerError(`${current.buildFailed} did not build. See ${devLogPath(paths)}`)
		}
		return current
	}
	return {
		live,
		runner: {
			data: paths.data,
			primary: paths.primary,
			runtime,
			send: (command: string) => sendRunnerCommand(paths, command),
			worktree: paths.worktree,
		},
		/** Resolves once everything that is meant to run answers. */
		async ready(deadline = Date.now() + READY_WAIT_MS): Promise<RuntimeFile> {
			for (;;) {
				// The runner rewrites its file as it works, so one read can come back empty.
				const current = runtime()
				if (current && (await runtimeIsReady(current))) return current
				if (Date.now() > deadline) throw new StillStarting('The stack is not ready yet.')
				await new Promise((resolve) => setTimeout(resolve, 500))
			}
		},
		/** Resolves once the frontend serves the App's page, which `ready` asks only of a stack with Apps. */
		async pageLoads(): Promise<void> {
			const deadline = Date.now() + READY_WAIT_MS
			for (;;) {
				const current = runtime()
				if (current?.programs['app-frontend'] === 'failed') {
					throw new DevRunnerError(`The App frontend stays down. See ${devLogPath(paths)}`)
				}
				if (current && (await appPageLoads(current.urls.app))) return
				if (Date.now() > deadline) throw new DevRunnerError('The App page did not load.')
				await new Promise((resolve) => setTimeout(resolve, 500))
			}
		},
	}
}

/**
 * `fixture <name>`: replaces the running stack's data with what the fixture builds, waits until
 * it is ready, and prints where to reach it.
 */
async function applyFixture(paths: WorktreePaths, name: string): Promise<void> {
	const stack = connectStack(paths)
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
	const first = process.env[FIRST_APPLY] === String(stack.live.pid)
	if (!first) await untilStarted(stack.runner.runtime)
	const release = await fixtureTurn(paths.data)
	const waitStarted = performance.now()
	const record = (result: 'ok' | 'failed') =>
		recordTiming(paths.primary, {
			kind: 'fixture',
			worktree: paths.worktree,
			result,
			fixture: NodePath.basename(file),
			ms: { total: Math.round(performance.now() - waitStarted) },
		})
	try {
		await fixtures.runFixture({ ...stack.runner, file })
		await stack.ready()
	} catch (error) {
		record('failed')
		runnerLog('error', error instanceof Error ? error.message : String(error))
		// A runner whose first fixture fails stops its stack.
		if (!first) {
			runnerLog('info', 'The stack stays up as far as the fixture got. Applying again starts over.')
		}
		process.exitCode = 1
		return
	} finally {
		release()
	}
	record('ok')
	// The runner prints this for its first fixture, once it has taken it as applied.
	if (!first) await printStatus(paths)
	if (stack.live.apps.length > 0) {
		runnerLog('info', 'Tabs keep their URLs. Reload an open one to see the new data.')
	}
}

/** `stop <name>` and `rs <name>`: asks the runner, then waits until the program is there. */
async function sendProgramCommand(
	paths: WorktreePaths,
	command: 'stop' | 'rs',
	args: ReadonlyArray<string>,
): Promise<void> {
	const [name] = args
	const before = readRunner(paths)
	if (!before) throw new DevRunnerError(NOT_RUNNING)
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
	const deadline = Date.now() + READY_WAIT_MS
	while (Date.now() < deadline) {
		if (!processIsRunning(before.pid!)) throw new DevRunnerError('The stack stopped.')
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

/** Stops the dev runner recorded in the runtime file, and everything it started. */
async function down(paths: WorktreePaths): Promise<void> {
	const runtime = readLiveRuntime(paths)
	if (!runtime?.pid || !readRunner(paths)) {
		// Left by a runner that was killed, and its PID now belongs to something else.
		if (runtime) writeRuntimeFile(paths, stoppedRuntime(runtime))
		// No apply outlives its runner.
		NodeFS.rmSync(fixtureLockPath(paths.data), { force: true })
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
	// An apply killed with its runner leaves its turn behind.
	NodeFS.rmSync(fixtureLockPath(paths.data), { force: true })
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
