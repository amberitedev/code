import { spawn, type ChildProcess } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { lstat, mkdir, readFile, realpath, rm, writeFile } from 'node:fs/promises'
import { dirname, isAbsolute, join, relative, resolve } from 'node:path'
import { promisify } from 'node:util'
import { execFile } from 'node:child_process'

import { prepareAppLibEnv } from './env.ts'
import { HOST, PERSONAS, PORT } from './fixtures.ts'
import { prepareNativePersona } from './native-state.ts'
import { startProxyLabServer, type RunningProxyLabServer } from './server.ts'
import { createStateStore } from './state.ts'
import type { Persona } from './types.ts'

const execFileAsync = promisify(execFile)
const REPOSITORY_ROOT = resolve(import.meta.dirname, '../../..')
const DATA_DIRECTORY_NAME = 'modrinthclonedata'
const GIT_EXCLUDE_ENTRY = `/${DATA_DIRECTORY_NAME}/`

export type DevPlan = {
	persona: Persona['id']
	publicCatalog: boolean
	repositoryRoot: string
	dataDir: string
	serviceDataDir: string
	appDataDir: string
	webviewDataDir: string
	command: string
	args: string[]
	env: Record<string, string>
}

export function createDevPlan(args: string[], repositoryRoot = REPOSITORY_ROOT): DevPlan {
	const personaName = args.find((argument) => !argument.startsWith('--')) ?? 'owner'
	if (personaName !== 'owner' && personaName !== 'recipient') {
		throw new Error(`Unknown proxy-lab persona "${personaName}". Use "owner" or "recipient".`)
	}
	const knownFlags = new Set(['--dry-run', '--public-catalog'])
	const unknownFlag = args.find(
		(argument) => argument.startsWith('--') && !knownFlags.has(argument),
	)
	if (unknownFlag) throw new Error(`Unknown proxy-lab option "${unknownFlag}"`)
	const positional = args.filter((argument) => !argument.startsWith('--'))
	if (positional.length > 1) throw new Error('The dev command accepts one persona.')

	const dataDir = join(repositoryRoot, DATA_DIRECTORY_NAME)
	const appDataDir = join(dataDir, 'personas', personaName)
	const configPath = join(repositoryRoot, 'packages', 'proxy-lab', 'tauri.proxy-lab.conf.json')
	const corepack = corepackCommand()

	return {
		persona: personaName,
		publicCatalog: args.includes('--public-catalog'),
		repositoryRoot,
		dataDir,
		serviceDataDir: join(dataDir, 'service'),
		appDataDir,
		webviewDataDir: join(appDataDir, 'webview'),
		command: corepack.command,
		args: [
			...corepack.args,
			'pnpm',
			'--filter',
			'@modrinth/app',
			'exec',
			'tauri',
			'dev',
			'--features',
			'export-app-events',
			'--config',
			configPath,
		],
		env: {
			THESEUS_CONFIG_DIR: appDataDir,
			THESEUS_DB_BACKUP_DIR: join(appDataDir, 'backups', 'app-db'),
			WEBVIEW2_USER_DATA_FOLDER: join(appDataDir, 'webview'),
		},
	}
}

export async function runDev(args: string[]): Promise<void> {
	const plan = createDevPlan(args)
	if (args.includes('--dry-run')) {
		console.log(JSON.stringify(plan, null, 2))
		return
	}

	const releaseRunLock = await acquireRunLock(plan)
	let server: RunningProxyLabServer | undefined
	let app: ChildProcess | undefined
	let cleanupPromise: Promise<void> | undefined
	let stopRequested = false

	const cleanup = (): Promise<void> => {
		cleanupPromise ??= (async () => {
			stopRequested = true
			if (app?.pid && app.exitCode === null && app.signalCode === null) {
				await stopChildTree(app)
			}
			if (server) await server.close()
			if (app?.pid) await finishRunManifest(plan, app.pid)
			await releaseRunLock()
		})()
		return cleanupPromise
	}

	const handleSignal = () => void cleanup().catch(reportCleanupError)
	process.once('SIGINT', handleSignal)
	process.once('SIGTERM', handleSignal)

	try {
		await prepareDataDirectories(plan)
		await prepareAppLibEnv()
		server = await startProxyLabServer({
			host: HOST,
			port: PORT,
			dataDir: plan.serviceDataDir,
			publicCatalog: plan.publicCatalog,
		})
		app = spawn(plan.command, plan.args, {
			cwd: plan.repositoryRoot,
			detached: process.platform !== 'win32',
			env: { ...process.env, ...plan.env },
			stdio: 'inherit',
			windowsHide: false,
		})
		const appExit = waitForChild(app)
		if (!app.pid) throw new Error('The proxy-lab App process did not report a PID.')
		await writeRunManifest(plan, app.pid)

		console.log(`Proxy lab backend: ${server.origin} (PID ${process.pid})`)
		console.log(`Proxy lab App: ${plan.persona} (PID ${app.pid})`)
		console.log(`Persistent isolated data: ${plan.dataDir}`)
		console.log(`Public Modrinth catalog reads: ${plan.publicCatalog ? 'enabled' : 'disabled'}`)

		const result = await appExit
		if (!stopRequested && result.code !== 0) {
			throw new Error(
				`The proxy-lab App exited with ${result.signal ? `signal ${result.signal}` : `code ${result.code}`}.`,
			)
		}
	} finally {
		process.off('SIGINT', handleSignal)
		process.off('SIGTERM', handleSignal)
		await cleanup()
	}
}

async function prepareDataDirectories(plan: DevPlan): Promise<void> {
	for (const path of [
		plan.serviceDataDir,
		join(plan.dataDir, 'personas'),
		plan.appDataDir,
		plan.webviewDataDir,
	]) {
		await refuseLink(path)
	}
	await mkdir(plan.serviceDataDir, { recursive: true })
	await mkdir(plan.webviewDataDir, { recursive: true })
	await createStateStore(plan.serviceDataDir)
	await prepareNativePersona(plan.appDataDir, plan.persona, plan.repositoryRoot)

	const fixture = PERSONAS[plan.persona]
	await writeFile(
		join(plan.appDataDir, 'proxy-lab-persona.json'),
		`${JSON.stringify(
			{
				persona: fixture.id,
				user_id: fixture.user.id,
				username: fixture.user.username,
			},
			null,
			'\t',
		)}\n`,
		'utf8',
	)
}

async function acquireRunLock(plan: DevPlan): Promise<() => Promise<void>> {
	assertDataDirectory(plan.repositoryRoot, plan.dataDir)
	await refuseLink(plan.dataDir)
	await ensureLocalGitIgnore(plan.repositoryRoot)
	await mkdir(plan.dataDir, { recursive: true })
	await refuseLink(plan.dataDir)

	const lockDir = join(plan.dataDir, '.run-lock')
	const ownerPath = join(lockDir, 'owner.json')
	const token = randomUUID()
	for (let attempt = 0; attempt < 2; attempt++) {
		try {
			await mkdir(lockDir)
			await writeFile(
				ownerPath,
				`${JSON.stringify({ pid: process.pid, token, started_at: new Date().toISOString() }, null, '\t')}\n`,
				'utf8',
			)
			return async () => {
				const owner = await readLockOwner(ownerPath)
				if (owner?.token === token) await rm(lockDir, { recursive: true, force: true })
			}
		} catch (error) {
			if (!isAlreadyExists(error)) throw error
			const owner = await readLockOwner(ownerPath)
			if (!owner || isProcessRunning(owner.pid)) {
				throw new Error(
					`Proxy lab is already running${owner ? ` in process ${owner.pid}` : ''}. Stop it before starting another persona.`,
				)
			}
			await rm(lockDir, { recursive: true, force: true })
		}
	}
	throw new Error('Could not acquire the proxy-lab run lock.')
}

async function readLockOwner(path: string): Promise<{ pid: number; token: string } | null> {
	try {
		const value: unknown = JSON.parse(await readFile(path, 'utf8'))
		if (
			typeof value === 'object' &&
			value !== null &&
			'pid' in value &&
			typeof value.pid === 'number' &&
			'token' in value &&
			typeof value.token === 'string'
		) {
			return { pid: value.pid, token: value.token }
		}
		return null
	} catch (error) {
		if (isMissing(error) || error instanceof SyntaxError) return null
		throw error
	}
}

function isProcessRunning(pid: number): boolean {
	try {
		process.kill(pid, 0)
		return true
	} catch (error) {
		return !isProcessGone(error)
	}
}

function corepackCommand(): { command: string; args: string[] } {
	if (process.platform !== 'win32') return { command: 'corepack', args: [] }
	return {
		command: process.execPath,
		args: [join(dirname(process.execPath), 'node_modules', 'corepack', 'dist', 'corepack.js')],
	}
}

function assertDataDirectory(repositoryRoot: string, dataDir: string): void {
	const path = relative(repositoryRoot, dataDir)
	if (path !== DATA_DIRECTORY_NAME || path.startsWith('..') || isAbsolute(path)) {
		throw new Error(`Refusing to use unexpected proxy-lab data directory ${dataDir}`)
	}
}

async function refuseLink(path: string): Promise<void> {
	try {
		const info = await lstat(path)
		if (info.isSymbolicLink()) {
			throw new Error(`Refusing to use linked proxy-lab data directory ${path}`)
		}
		const resolved = await realpath(path)
		if (resolved !== resolve(path)) {
			throw new Error(`Refusing to use redirected proxy-lab data directory ${path}`)
		}
	} catch (error) {
		if (isMissing(error)) return
		throw error
	}
}

async function ensureLocalGitIgnore(repositoryRoot: string): Promise<void> {
	const { stdout } = await execFileAsync(
		'git',
		['-C', repositoryRoot, 'rev-parse', '--git-path', 'info/exclude'],
		{ encoding: 'utf8' },
	)
	const value = stdout.trim()
	const excludePath = isAbsolute(value) ? value : resolve(repositoryRoot, value)
	let existing = ''
	try {
		existing = await readFile(excludePath, 'utf8')
	} catch (error) {
		if (!isMissing(error)) throw error
	}
	if (existing.split(/\r?\n/).includes(GIT_EXCLUDE_ENTRY)) return
	await mkdir(dirname(excludePath), { recursive: true })
	const prefix = existing.length > 0 && !existing.endsWith('\n') ? '\n' : ''
	await writeFile(excludePath, `${existing}${prefix}${GIT_EXCLUDE_ENTRY}\n`, 'utf8')
}

async function writeRunManifest(plan: DevPlan, childPid: number): Promise<void> {
	await writeFile(
		join(plan.dataDir, 'run.json'),
		`${JSON.stringify(
			{
				started_at: new Date().toISOString(),
				persona: plan.persona,
				orchestrator_pid: process.pid,
				app_process_pid: childPid,
				backend_origin: `http://${HOST}:${PORT}`,
			},
			null,
			'\t',
		)}\n`,
		'utf8',
	)
}

async function finishRunManifest(plan: DevPlan, childPid: number | undefined): Promise<void> {
	const runPath = join(plan.dataDir, 'run.json')
	await rm(runPath, { force: true })
	await writeFile(
		join(plan.dataDir, 'last-run.json'),
		`${JSON.stringify(
			{
				stopped_at: new Date().toISOString(),
				persona: plan.persona,
				orchestrator_pid: process.pid,
				app_process_pid: childPid ?? null,
			},
			null,
			'\t',
		)}\n`,
		'utf8',
	)
}

function waitForChild(
	child: ChildProcess,
): Promise<{ code: number | null; signal: NodeJS.Signals | null }> {
	return new Promise((resolve, reject) => {
		child.once('error', reject)
		child.once('exit', (code, signal) => resolve({ code, signal }))
	})
}

async function stopChildTree(child: ChildProcess): Promise<void> {
	const pid = child.pid
	if (!pid) return
	if (process.platform === 'win32') {
		await new Promise<void>((resolve) => {
			const taskkill = spawn('taskkill.exe', ['/PID', String(pid), '/T', '/F'], {
				stdio: 'ignore',
				windowsHide: true,
			})
			taskkill.once('error', () => resolve())
			taskkill.once('exit', () => resolve())
		})
		return
	}
	try {
		process.kill(-pid, 'SIGTERM')
	} catch (error) {
		if (!isProcessGone(error)) throw error
	}
}

function reportCleanupError(error: unknown): void {
	console.error(error)
	process.exitCode = 1
}

function isMissing(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error && error.code === 'ENOENT'
}

function isAlreadyExists(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error && error.code === 'EEXIST'
}

function isProcessGone(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error && error.code === 'ESRCH'
}
