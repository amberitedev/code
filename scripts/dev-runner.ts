#!/usr/bin/env node

import * as NodeChildProcess from 'node:child_process'
import * as NodeCrypto from 'node:crypto'
import * as NodeFS from 'node:fs'
import * as NodeNet from 'node:net'
import * as NodePath from 'node:path'
import * as NodeReadline from 'node:readline'
import * as NodeURL from 'node:url'
import * as NodeUtil from 'node:util'

const BASE_PORTS = {
	app: 1420,
	convexCloud: 3210,
	convexSite: 3211,
	core: 16662,
} as const
const MAX_HASH_OFFSET = 3000
const MAX_PORT = 65_535
const PORT_PROBE_HOSTS = ['127.0.0.1', '::1'] as const
const FETCH_BAD_PORTS = new Set([
	0, 1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95, 101, 102,
	103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143, 161, 179, 389, 427, 465,
	512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563, 587, 601, 636, 989, 990, 993,
	995, 1719, 1720, 1723, 2049, 3659, 4045, 4190, 5060, 5061, 6000, 6566, 6665, 6666, 6667, 6668,
	6669, 6679, 6697, 10080,
])

export const DEV_MODES = ['dev', 'dev:app', 'dev:core', 'dev:convex'] as const
export type DevMode = (typeof DEV_MODES)[number]
export type ConvexMode = 'cloud' | 'local'
export type DevPorts = {
	readonly app: number
	readonly convexCloud: number
	readonly convexSite: number
	readonly core: number
}

type PortName = keyof DevPorts
type PortAvailabilityCheck = (port: number, hosts: ReadonlyArray<string>) => Promise<boolean>
export type WorktreePaths = {
	readonly convexData: string
	readonly coreData: string
	readonly data: string
	readonly primary: string
	readonly runtime: string
	readonly scenariosData: string
	readonly worktree: string
}
type RunnerInput = {
	readonly dryRun: boolean
	readonly mode: DevMode
	readonly scenarios: ReadonlyArray<number>
}
export type GitDiffSummary = {
	readonly additions: number
	readonly deletions: number
	readonly files: number
}
export type ProcessSpec = {
	readonly args: ReadonlyArray<string>
	readonly cwd: string
	readonly env: NodeJS.ProcessEnv
	readonly label: string
}
type RunningProcess = {
	readonly child: NodeChildProcess.ChildProcess
	readonly done: Promise<number>
	readonly label: string
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
		convexCloud: BASE_PORTS.convexCloud + offset,
		convexSite: BASE_PORTS.convexSite + offset,
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
	readonly checkPort?: PortAvailabilityCheck
	readonly convexMode?: ConvexMode
	readonly mode: DevMode
	readonly startOffset: number
}): Promise<number> {
	const requiredPorts = requiredPortNames(input.mode, input.convexMode ?? 'local')
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
	readonly convexMode: ConvexMode
	readonly localDeployment?: string
	readonly paths: WorktreePaths
	readonly ports: DevPorts
}): NodeJS.ProcessEnv {
	const env: NodeJS.ProcessEnv = { ...input.baseEnv }
	delete env.HOST
	delete env.PORT

	if (input.convexMode === 'local') {
		delete env.CONVEX_SELF_HOSTED_ADMIN_KEY
		delete env.CONVEX_SELF_HOSTED_URL

		const localDeployment = input.localDeployment ?? readLocalConvexDeployment(input.paths)
		const convexUrl = `http://127.0.0.1:${input.ports.convexCloud}`
		const convexSiteUrl = `http://127.0.0.1:${input.ports.convexSite}`
		return {
			...env,
			AMBERITE_DATA_DIR: input.paths.data,
			AMBERITE_DEV_MODE: 'true',
			AMBERITE_LOCAL_CORE_DATA_DIR: input.paths.coreData,
			CONVEX_DEPLOYMENT: `local:${localDeployment}`,
			CONVEX_SITE_URL: convexSiteUrl,
			CONVEX_URL: convexUrl,
			VITE_CONVEX_SITE_URL: convexSiteUrl,
			VITE_CONVEX_URL: convexUrl,
			VITE_CORE_URL: `http://127.0.0.1:${input.ports.core}`,
		}
	}

	const convexDeployment = requireCloudDevDeployment(env)
	const convexUrl = requireEnvironmentValue(env, 'CONVEX_URL')
	const convexSiteUrl = requireEnvironmentValue(env, 'CONVEX_SITE_URL')
	return {
		...env,
		AMBERITE_DATA_DIR: input.paths.data,
		AMBERITE_DEV_MODE: 'true',
		AMBERITE_LOCAL_CORE_DATA_DIR: input.paths.coreData,
		CONVEX_DEPLOYMENT: convexDeployment,
		CONVEX_SITE_URL: convexSiteUrl,
		CONVEX_URL: convexUrl,
		VITE_CONVEX_SITE_URL: convexSiteUrl,
		VITE_CONVEX_URL: convexUrl,
		VITE_CORE_URL: `http://127.0.0.1:${input.ports.core}`,
	}
}

export function processLabelsForMode(mode: DevMode): ReadonlyArray<string> {
	switch (mode) {
		case 'dev':
			return ['convex', 'core', 'app-frontend']
		case 'dev:app':
			return ['app-frontend']
		case 'dev:core':
			return ['core']
		case 'dev:convex':
			return ['convex']
	}
}

export function parseGitNumstat(output: string): GitDiffSummary {
	let additions = 0
	let deletions = 0
	let files = 0
	for (const line of output.split(/\r?\n/)) {
		if (!line.trim()) continue
		const [added, deleted] = line.split('\t', 3)
		if (added === undefined || deleted === undefined) continue
		files += 1
		if (/^\d+$/.test(added)) additions += Number(added)
		if (/^\d+$/.test(deleted)) deletions += Number(deleted)
	}
	return { additions, deletions, files }
}

export function shouldPromptForCloudConvexPush(
	convexMode: ConvexMode,
	mode: DevMode,
	diff: GitDiffSummary,
): boolean {
	return convexMode === 'cloud' && processLabelsForMode(mode).includes('convex') && diff.files > 0
}

export function formatGitDiffSummary(diff: GitDiffSummary, color = true): string {
	const additions = `+${diff.additions}`
	const deletions = `-${diff.deletions}`
	const formattedAdditions = color ? `\u001b[32m${additions}\u001b[0m` : additions
	const formattedDeletions = color ? `\u001b[31m${deletions}\u001b[0m` : deletions
	return `${formattedAdditions} ${formattedDeletions} in ${diff.files} ${diff.files === 1 ? 'file' : 'files'}`
}

async function main(): Promise<void> {
	const paths = resolveWorktreePaths()
	const convexMode = resolveConvexMode(paths)
	const input = parseInput(
		process.argv.slice(2),
		readDefaultScenarios(NodePath.join(paths.worktree, 'dev.json')),
	)
	const { offset: startOffset, source } = resolveStartOffset({
		devInstance: process.env.AMBERITE_DEV_INSTANCE,
		explicitOffset: process.env.AMBERITE_PORT_OFFSET,
		primaryPath: paths.primary,
		worktreePath: paths.worktree,
	})
	const selectedOffset = await findFirstAvailableOffset({
		convexMode,
		mode: input.mode,
		startOffset,
	})
	const ports = portsForOffset(selectedOffset)
	const branch = currentBranch(paths.worktree)
	const sharedEnv = {
		...readEnv(NodePath.join(paths.primary, '.env.local')),
		...readEnv(NodePath.join(paths.primary, 'apps', 'core', '.env.local')),
		...readEnv(NodePath.join(paths.worktree, '.env.local')),
		...process.env,
	}
	const env = createRuntimeEnvironment({ baseEnv: sharedEnv, convexMode, paths, ports })
	const specs = createProcessSpecs({
		branch,
		convexMode,
		env,
		mode: input.mode,
		paths,
		ports,
		scenarios: input.scenarios,
	})
	const convexDiff =
		convexMode === 'cloud' && processLabelsForMode(input.mode).includes('convex')
			? readConvexGitDiff(paths.worktree)
			: null

	printPlan({
		branch,
		convexMode,
		env,
		mode: input.mode,
		paths,
		ports,
		scenarios: input.scenarios,
		source,
		specs,
		convexDiff,
	})
	if (input.dryRun) return
	if (convexDiff && shouldPromptForCloudConvexPush(convexMode, input.mode, convexDiff)) {
		await confirmCloudConvexPush(convexDiff)
	}

	ensureDataLayout(paths, input.scenarios)
	writeRuntimeFile({
		branch,
		convexMode,
		env,
		mode: input.mode,
		paths,
		ports,
		scenarios: input.scenarios,
		source,
	})
	const processes = specs.map((spec) => spawnProcess(paths.worktree, spec))
	let stopping = false

	const stop = async (exitCode: number) => {
		if (stopping) return
		stopping = true
		await Promise.all(processes.map(stopProcess))
		process.exitCode = exitCode
	}

	process.once('SIGINT', () => void stop(130))
	process.once('SIGTERM', () => void stop(143))

	try {
		const firstExit = Promise.race(
			processes.map(async (running) => ({ code: await running.done, label: running.label })),
		)
		if (convexMode === 'local' && processLabelsForMode(input.mode).includes('convex')) {
			await Promise.race([
				prepareConvex({ env, paths, ports, processes, scenarios: input.scenarios }),
				firstExit.then((exited) => {
					throw new DevRunnerError(`${exited.label} exited with code ${exited.code}.`)
				}),
			])
		}

		const exited = await firstExit
		if (!stopping) {
			throw new DevRunnerError(`${exited.label} exited with code ${exited.code}.`)
		}
	} finally {
		await stop(process.exitCode || 1)
	}
}

export function resolveConvexMode(paths: Pick<WorktreePaths, 'primary' | 'worktree'>): ConvexMode {
	return samePath(paths.primary, paths.worktree) ? 'cloud' : 'local'
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

	const scenarioArgs = args.filter((arg) => arg !== modeValue && arg !== '--dry-run')
	const startsApps = modeValue === 'dev' || modeValue === 'dev:app'
	if (!startsApps && scenarioArgs.length > 0) {
		throw new DevRunnerError(`${modeValue} does not start App scenarios.`)
	}
	const scenarios = startsApps
		? normalizeScenarios(scenarioArgs.length > 0 ? scenarioArgs : defaultScenarios)
		: []

	return { dryRun: args.includes('--dry-run'), mode: modeValue as DevMode, scenarios }
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
		convexData: NodePath.join(data, 'convex'),
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
	ensureConvexDataLink(paths)
	NodeFS.mkdirSync(paths.coreData, { recursive: true })
	for (const scenario of new Set([1, 2, 3, 4, ...scenarios])) {
		NodeFS.mkdirSync(scenarioDataPath(paths, scenario), { recursive: true })
	}
}

function ensureConvexDataLink(paths: WorktreePaths): void {
	const linkPath = NodePath.join(paths.worktree, '.convex')
	if (NodeFS.existsSync(linkPath)) {
		const stat = NodeFS.lstatSync(linkPath)
		if (stat.isSymbolicLink()) {
			const actual = NodeFS.realpathSync(linkPath)
			NodeFS.mkdirSync(paths.convexData, { recursive: true })
			if (!samePath(actual, NodeFS.realpathSync(paths.convexData))) {
				throw new DevRunnerError(`.convex points to ${actual}, not ${paths.convexData}.`)
			}
			return
		}

		if (NodeFS.existsSync(paths.convexData)) {
			throw new DevRunnerError(
				'Both .convex and .data/convex exist. Move the wanted state into .data/convex before starting development.',
			)
		}
		NodeFS.renameSync(linkPath, paths.convexData)
	} else {
		NodeFS.mkdirSync(paths.convexData, { recursive: true })
	}

	NodeFS.symlinkSync(paths.convexData, linkPath, process.platform === 'win32' ? 'junction' : 'dir')
}

export function createProcessSpecs(input: {
	readonly branch: string
	readonly convexMode: ConvexMode
	readonly env: NodeJS.ProcessEnv
	readonly mode: DevMode
	readonly paths: WorktreePaths
	readonly ports: DevPorts
	readonly scenarios: ReadonlyArray<number>
}): ReadonlyArray<ProcessSpec> {
	const labels = processLabelsForMode(input.mode)
	const convexUrl = requireEnvironmentValue(input.env, 'VITE_CONVEX_URL')
	const convexSiteUrl = requireEnvironmentValue(input.env, 'VITE_CONVEX_SITE_URL')
	const tauri = JSON.parse(
		NodeFS.readFileSync(
			NodePath.join(input.paths.worktree, 'apps', 'app', 'tauri.conf.json'),
			'utf8',
		),
	) as {
		readonly app: { readonly security: { readonly csp: Record<string, string> } }
	}
	const connectSrc = tauri.app.security.csp['connect-src']
	const convexOrigin = new URL(convexUrl).origin
	const convexSiteOrigin = new URL(convexSiteUrl).origin
	const tauriOverride = {
		app: {
			security: {
				csp: {
					'connect-src': `${connectSrc} http://localhost:${input.ports.app} ws://localhost:${input.ports.app} ${convexOrigin} ${convexOrigin.replace('http', 'ws')} ${convexSiteOrigin}`,
				},
			},
		},
		build: {
			beforeDevCommand: '',
			devUrl: `http://localhost:${input.ports.app}`,
		},
	}
	const coreEnv = {
		...input.env,
		ALLOWED_ORIGIN: `http://localhost:${input.ports.app}`,
		AMBERITE_BIND_HOST: '127.0.0.1',
		AMBERITE_PUBLIC_URL: `http://127.0.0.1:${input.ports.core}`,
		CORE_DATA_DIR: input.paths.coreData,
		PORT: String(input.ports.core),
	}
	const specs: Record<string, ProcessSpec> = {
		'app-frontend': {
			args: [
				'run',
				'--filter',
				'@modrinth/app-frontend',
				'dev',
				'--',
				'--port',
				String(input.ports.app),
				'--strictPort',
			],
			cwd: input.paths.worktree,
			env: input.env,
			label: 'app-frontend',
		},
		convex: {
			args:
				input.convexMode === 'cloud'
					? ['exec', 'convex', 'dev', '--tail-logs', 'disable']
					: [
							'exec',
							'convex',
							'dev',
							'--tail-logs',
							'disable',
							'--local-cloud-port',
							String(input.ports.convexCloud),
							'--local-site-port',
							String(input.ports.convexSite),
						],
			cwd: input.paths.worktree,
			env: input.env,
			label: 'convex',
		},
		core: {
			args: ['run', '--filter', '@amberite/core', 'dev'],
			cwd: input.paths.worktree,
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
				convexSiteUrl,
				convexUrl,
				env: input.env,
				paths: input.paths,
				scenario,
				tauriOverride,
			}),
		),
	]
}

function createAppProcessSpec(input: {
	readonly branch: string
	readonly convexSiteUrl: string
	readonly convexUrl: string
	readonly env: NodeJS.ProcessEnv
	readonly paths: WorktreePaths
	readonly scenario: number
	readonly tauriOverride: unknown
}): ProcessSpec {
	const dataDir = scenarioDataPath(input.paths, input.scenario)
	const namespace = `${input.branch}:scenario:${input.scenario}`
	const appDevConfig = {
		authMode: 'dev',
		branch: input.branch,
		convexSiteUrl: input.convexSiteUrl,
		convexUrl: input.convexUrl,
		credentialNamespace: namespace,
		dataDir,
		title: `Amberite ${input.scenario} — ${input.branch}`,
		username: scenarioUsername(input.scenario),
	}
	return {
		args: [
			'exec',
			'tauri',
			'dev',
			'--features',
			'browser-bridge',
			'--config',
			JSON.stringify(input.tauriOverride),
			'--',
			'--',
			'--amberite-dev-config',
			JSON.stringify(appDevConfig),
		],
		cwd: NodePath.join(input.paths.worktree, 'apps', 'app'),
		env: {
			...input.env,
			THESEUS_CONFIG_DIR: dataDir,
			WEBVIEW2_USER_DATA_FOLDER: NodePath.join(dataDir, 'webview2'),
		},
		label: `app:${input.scenario}`,
	}
}

function scenarioDataPath(paths: WorktreePaths, scenario: number): string {
	return NodePath.join(paths.scenariosData, String(scenario))
}

function spawnProcess(worktree: string, spec: ProcessSpec): RunningProcess {
	const vpPath = resolveVpPath(worktree)
	const child = NodeChildProcess.spawn(process.execPath, [vpPath, ...spec.args], {
		cwd: spec.cwd,
		detached: process.platform !== 'win32',
		env: spec.env,
		stdio: ['inherit', 'pipe', 'pipe'],
		windowsHide: true,
	})
	pipeOutput(child.stdout, spec.label, process.stdout)
	pipeOutput(child.stderr, spec.label, process.stderr)

	const done = new Promise<number>((resolve, reject) => {
		child.once('error', (error) =>
			reject(new DevRunnerError(`Could not start ${spec.label}: ${error.message}`)),
		)
		child.once('exit', (code, signal) => {
			if (code !== null) resolve(code)
			else resolve(signal ? 1 : 0)
		})
	})

	return { child, done, label: spec.label }
}

async function stopProcess(running: RunningProcess): Promise<void> {
	const pid = running.child.pid
	if (!pid || running.child.exitCode !== null) return

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
		running.child.kill('SIGTERM')
	}
}

async function prepareConvex(input: {
	readonly env: NodeJS.ProcessEnv
	readonly paths: WorktreePaths
	readonly ports: DevPorts
	readonly processes: ReadonlyArray<RunningProcess>
	readonly scenarios: ReadonlyArray<number>
}): Promise<void> {
	const convexProcess = input.processes.find((process) => process.label === 'convex')
	if (!convexProcess) return

	await waitForUrl(`http://127.0.0.1:${input.ports.convexCloud}`, convexProcess)
	ensureConvexDevMode(input.paths.worktree, input.env)
	ensureConvexAuthKeys(input.paths.worktree, input.env)
	await ensureConvexScenarios(input.paths.worktree, input.env, input.scenarios)
}

function ensureConvexDevMode(worktree: string, env: NodeJS.ProcessEnv): void {
	const result = runVp(
		worktree,
		['exec', 'convex', 'env', 'get', 'AMBERITE_DEV_MODE', '--deployment', 'local'],
		env,
	)
	if (result.status === 0 && result.stdout.trim() === 'true') return
	runVpOrThrow(
		worktree,
		['exec', 'convex', 'env', 'set', 'AMBERITE_DEV_MODE', 'true', '--deployment', 'local'],
		env,
	)
}

async function ensureConvexScenarios(
	worktree: string,
	env: NodeJS.ProcessEnv,
	scenarios: ReadonlyArray<number>,
): Promise<void> {
	if (scenarios.length === 0) return
	const args = [
		'exec',
		'convex',
		'run',
		'dev:ensureScenarios',
		JSON.stringify({ scenarios }),
		'--deployment',
		'local',
	]
	const deadline = Date.now() + 60_000
	let failure = ''
	while (Date.now() < deadline) {
		const result = runVp(worktree, args, env)
		if (result.status === 0) return
		failure = result.stderr.trim() || result.stdout.trim()
		await new Promise((resolve) => setTimeout(resolve, 250))
	}
	throw new DevRunnerError(failure || 'Convex dev scenarios could not be prepared.')
}

function ensureConvexAuthKeys(worktree: string, env: NodeJS.ProcessEnv): void {
	const privateResult = runVp(
		worktree,
		['exec', 'convex', 'env', 'get', 'JWT_PRIVATE_KEY', '--deployment', 'local'],
		env,
	)
	const jwksResult = runVp(
		worktree,
		['exec', 'convex', 'env', 'get', 'JWKS', '--deployment', 'local'],
		env,
	)
	if (
		privateResult.status === 0 &&
		jwksResult.status === 0 &&
		keysMatch(privateResult.stdout, jwksResult.stdout)
	)
		return

	const { privateKey, publicKey } = NodeCrypto.generateKeyPairSync('rsa', { modulusLength: 2048 })
	const privatePem = privateKey
		.export({ type: 'pkcs8', format: 'pem' })
		.trimEnd()
		.replace(/\n/g, ' ')
	const jwks = JSON.stringify({ keys: [{ use: 'sig', ...publicKey.export({ format: 'jwk' }) }] })
	runVpOrThrow(
		worktree,
		['exec', 'convex', 'env', 'set', 'JWT_PRIVATE_KEY', privatePem, '--deployment', 'local'],
		env,
	)
	runVpOrThrow(
		worktree,
		['exec', 'convex', 'env', 'set', 'JWKS', jwks, '--deployment', 'local'],
		env,
	)
}

function keysMatch(privateValue: string, jwksValue: string): boolean {
	try {
		const match = privateValue.match(
			/-----BEGIN PRIVATE KEY-----([\s\S]+?)-----END PRIVATE KEY-----/,
		)
		if (!match) return false
		const body = match[1]
			.replace(/\s/g, '')
			.match(/.{1,64}/g)
			?.join('\n')
		if (!body) return false
		const expected = NodeCrypto.createPublicKey(
			NodeCrypto.createPrivateKey(
				`-----BEGIN PRIVATE KEY-----\n${body}\n-----END PRIVATE KEY-----\n`,
			),
		).export({ format: 'jwk' })
		const parsed = JSON.parse(jwksValue) as {
			readonly keys?: ReadonlyArray<Record<string, unknown>>
		}
		return (
			parsed.keys?.some(
				(key) => key.kty === 'RSA' && key.n === expected.n && key.e === expected.e,
			) ?? false
		)
	} catch {
		return false
	}
}

function runVp(worktree: string, args: ReadonlyArray<string>, env: NodeJS.ProcessEnv) {
	return NodeChildProcess.spawnSync(process.execPath, [resolveVpPath(worktree), ...args], {
		cwd: worktree,
		encoding: 'utf8',
		env,
		windowsHide: true,
	})
}

function runVpOrThrow(worktree: string, args: ReadonlyArray<string>, env: NodeJS.ProcessEnv): void {
	const result = runVp(worktree, args, env)
	if (result.status === 0) return
	throw new DevRunnerError(
		result.stderr.trim() || result.stdout.trim() || 'A Vite+ command failed.',
	)
}

async function waitForUrl(url: string, running: RunningProcess): Promise<void> {
	const deadline = Date.now() + 60_000
	while (Date.now() < deadline) {
		if (running.child.exitCode !== null) {
			throw new DevRunnerError('Convex exited before its local backend became ready.')
		}
		try {
			const response = await fetch(url)
			if (response.status < 500) return
		} catch {}
		await new Promise((resolve) => setTimeout(resolve, 250))
	}
	throw new DevRunnerError(`Convex did not become ready at ${url} within 60 seconds.`)
}

function pipeOutput(
	stream: NodeJS.ReadableStream | null,
	label: string,
	destination: NodeJS.WritableStream,
): void {
	if (!stream) return
	const lines = NodeReadline.createInterface({ input: stream })
	lines.on('line', (line) => destination.write(`[${label}] ${line}\n`))
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

function requiredPortNames(mode: DevMode, convexMode: ConvexMode): ReadonlyArray<PortName> {
	switch (mode) {
		case 'dev':
			return convexMode === 'local' ? ['app', 'convexCloud', 'convexSite', 'core'] : ['app', 'core']
		case 'dev:app':
			return ['app']
		case 'dev:core':
			return ['core']
		case 'dev:convex':
			return convexMode === 'local' ? ['convexCloud', 'convexSite'] : []
	}
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

const CONVEX_GIT_PATHS = [':(glob)convex/**/*.ts', ':(glob)convex/**/*.js', 'convex.json'] as const

function readConvexGitDiff(worktree: string): GitDiffSummary {
	git(['rev-parse', '--verify', 'origin/main'], worktree)
	const tracked = git(
		[
			'diff',
			'--numstat',
			'--ignore-all-space',
			'--ignore-blank-lines',
			'origin/main',
			'--',
			...CONVEX_GIT_PATHS,
		],
		worktree,
	)
	const untracked = git(
		['ls-files', '--others', '--exclude-standard', '--', ...CONVEX_GIT_PATHS],
		worktree,
	)
	const untrackedDiffs = untracked
		.split(/\r?\n/)
		.filter(Boolean)
		.map((path) => gitUntrackedNumstat(worktree, path))
	return parseGitNumstat([tracked, ...untrackedDiffs].filter(Boolean).join('\n'))
}

function gitUntrackedNumstat(worktree: string, path: string): string {
	const emptyPath = process.platform === 'win32' ? 'NUL' : '/dev/null'
	const result = NodeChildProcess.spawnSync(
		'git',
		[
			'diff',
			'--no-index',
			'--numstat',
			'--ignore-all-space',
			'--ignore-blank-lines',
			'--',
			emptyPath,
			path,
		],
		{ cwd: worktree, encoding: 'utf8' },
	)
	if (result.status !== 0 && result.status !== 1) {
		throw new DevRunnerError(result.stderr.trim() || `git diff for ${path} failed.`)
	}
	return result.stdout.trim()
}

async function confirmCloudConvexPush(diff: GitDiffSummary): Promise<void> {
	if (!process.stdin.isTTY || !process.stdout.isTTY) {
		throw new DevRunnerError(
			`Convex cloud has ${formatGitDiffSummary(diff, false)}. Run vp run dev in an interactive terminal to approve the push.`,
		)
	}
	const prompt = NodeReadline.createInterface({ input: process.stdin, output: process.stdout })
	try {
		const answer = (
			await new Promise<string>((resolve) =>
				prompt.question('[dev-runner] Push these changes to Convex cloud? [y/N] ', resolve),
			)
		)
			.trim()
			.toLowerCase()
		if (answer !== 'y' && answer !== 'yes') {
			throw new DevRunnerError('Convex cloud push cancelled. Use vp run dev:app to skip Convex.')
		}
	} finally {
		prompt.close()
	}
}

function git(args: ReadonlyArray<string>, cwd: string): string {
	const result = NodeChildProcess.spawnSync('git', args, { cwd, encoding: 'utf8' })
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

function requireEnvironmentValue(env: NodeJS.ProcessEnv, name: string): string {
	const value = env[name]?.trim()
	if (value) return value
	throw new DevRunnerError(
		`${name} is required because the primary checkout uses the cloud Convex development deployment.`,
	)
}

function requireCloudDevDeployment(env: NodeJS.ProcessEnv): string {
	const deployment = requireEnvironmentValue(env, 'CONVEX_DEPLOYMENT')
	if (deployment.startsWith('dev:')) return deployment
	throw new DevRunnerError(
		`The primary checkout requires a cloud Convex development deployment; received ${deployment.split(':', 1)[0] || 'an invalid value'}.`,
	)
}

function readLocalConvexDeployment(paths: WorktreePaths): string {
	const candidates = [
		NodePath.join(paths.convexData, 'local', 'default', 'config.json'),
		NodePath.join(paths.worktree, '.convex', 'local', 'default', 'config.json'),
	]
	const path = candidates.find((candidate) => NodeFS.existsSync(candidate))
	if (!path) {
		throw new DevRunnerError(
			'Local Convex data is missing. Run the worktree setup before starting development.',
		)
	}

	let value: unknown
	try {
		value = JSON.parse(NodeFS.readFileSync(path, 'utf8'))
	} catch (error) {
		throw new DevRunnerError(`Could not read ${path}: ${String(error)}`)
	}
	if (
		!isRecord(value) ||
		typeof value.deploymentName !== 'string' ||
		!value.deploymentName.trim()
	) {
		throw new DevRunnerError(`${path} does not contain a local Convex deployment name.`)
	}
	return value.deploymentName.trim()
}

function resolveVpPath(worktree: string): string {
	const path = NodePath.join(worktree, 'node_modules', 'vite-plus', 'bin', 'vp')
	if (!NodeFS.existsSync(path)) {
		throw new DevRunnerError('Vite+ is not installed. Run vp install first.')
	}
	return path
}

function writeRuntimeFile(input: {
	readonly branch: string
	readonly convexMode: ConvexMode
	readonly env: NodeJS.ProcessEnv
	readonly mode: DevMode
	readonly paths: WorktreePaths
	readonly ports: DevPorts
	readonly scenarios: ReadonlyArray<number>
	readonly source: string
}): void {
	const content = {
		branch: input.branch,
		convexMode: input.convexMode,
		dataDir: input.paths.data,
		mode: input.mode,
		ports: input.ports,
		scenarios: input.scenarios.map((scenario) => ({
			dataDir: scenarioDataPath(input.paths, scenario),
			number: scenario,
			username: scenarioUsername(scenario),
		})),
		source: input.source,
		urls: {
			app: `http://localhost:${input.ports.app}`,
			convex: requireEnvironmentValue(input.env, 'VITE_CONVEX_URL'),
			convexSite: requireEnvironmentValue(input.env, 'VITE_CONVEX_SITE_URL'),
			core: `http://127.0.0.1:${input.ports.core}`,
		},
	}
	NodeFS.writeFileSync(input.paths.runtime, `${JSON.stringify(content, null, '\t')}\n`)
}

function printPlan(input: {
	readonly branch: string
	readonly convexMode: ConvexMode
	readonly env: NodeJS.ProcessEnv
	readonly mode: DevMode
	readonly paths: WorktreePaths
	readonly ports: DevPorts
	readonly scenarios: ReadonlyArray<number>
	readonly source: string
	readonly specs: ReadonlyArray<ProcessSpec>
	readonly convexDiff: GitDiffSummary | null
}): void {
	console.log(`[dev-runner] ${input.branch} · ${input.mode}`)
	console.log(`[dev-runner] data ${input.paths.data}`)
	console.log(`[dev-runner] ports from ${input.source}`)
	console.log(`[dev-runner] App http://localhost:${input.ports.app}`)
	console.log(`[dev-runner] Core http://127.0.0.1:${input.ports.core}`)
	console.log(
		`[dev-runner] Convex ${requireEnvironmentValue(input.env, 'VITE_CONVEX_URL')} (${input.convexMode})`,
	)
	if (input.convexDiff?.files) {
		console.log(`[dev-runner] Convex changes ${formatGitDiffSummary(input.convexDiff)}`)
	}
	if (input.scenarios.length > 0) {
		console.log(`[dev-runner] scenarios ${input.scenarios.join(', ')}`)
	}
	console.log(`[dev-runner] processes ${input.specs.map((spec) => spec.label).join(', ')}`)
}

const isMain = process.argv[1] && samePath(process.argv[1], NodeURL.fileURLToPath(import.meta.url))
if (isMain) {
	main().catch((error: unknown) => {
		console.error(error instanceof Error ? error.message : String(error))
		process.exitCode = 1
	})
}
