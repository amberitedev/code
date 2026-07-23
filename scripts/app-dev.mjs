#!/usr/bin/env node
import { spawn, spawnSync } from 'node:child_process'
import {
	chmodSync,
	closeSync,
	copyFileSync,
	existsSync,
	fstatSync,
	mkdirSync,
	openSync,
	readSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs'
import net from 'node:net'
import { basename, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
	commandForPnpm,
	findWorktreeRoot,
	gitValue,
	globalDevRoot,
	isProcessRunning,
	killProcessTree,
	makeShortId,
	readEnvFile,
	validateDevUsername,
	worktreeKey,
} from './dev-shared.mjs'

const action = process.argv[2] ?? 'start'
const controllerProtocolVersion = 2
const jsonOutput = process.argv.includes('--json')
const positional = process.argv.slice(3).filter((value) => value !== '--' && value !== '--json')
let worktree
let appRoot
let registryPath
let socketPath

if (action === 'windows-host' && process.platform === 'win32') {
	await runWindowsHost()
} else if (action === 'windows-control' && process.platform === 'win32') {
	await runWindowsControl()
} else {
	worktree = findWorktreeRoot()
	const root = globalDevRoot(worktree)
	appRoot = join(root, 'apps')
	registryPath = join(appRoot, 'registry.json')
	socketPath =
		process.platform === 'win32'
			? `\\\\.\\pipe\\amberite-${basename(root)}`
			: join(appRoot, 'controller.sock')

	mkdirSync(appRoot, { recursive: true })

	try {
		if (action === 'serve') await serve()
		else await runClient()
	} catch (error) {
		console.error(error instanceof Error ? error.message : String(error))
		process.exitCode = 1
	}
}

async function runClient() {
	const request = requestForCommand()
	await ensureController()
	const response = await sendRequest(request, request.action === 'restart' ? 120_000 : 15_000)
	if (!response.ok) throw new Error(response.error)
	printResult(action, response.result)
	if (action === 'start' || action === 'start-auth') await attachToApp(response.result)
}

async function attachToApp(app) {
	let offset = 0
	let stopping = false

	function flushLogs() {
		if (!existsSync(app.logPath)) return
		const log = openSync(app.logPath, 'r')
		try {
			const size = fstatSync(log).size
			if (size < offset) offset = 0
			if (size === offset) return
			const output = Buffer.alloc(size - offset)
			readSync(log, output, 0, output.length, offset)
			offset = size
			process.stdout.write(output)
		} finally {
			closeSync(log)
		}
	}

	console.log(`Attached to app ${app.id}. Press Ctrl+C to stop it.`)
	flushLogs()
	await new Promise((resolve) => {
		const logTimer = setInterval(flushLogs, 100)
		const statusTimer = setInterval(async () => {
			try {
				const response = await sendRequest({ action: 'list' }, 1_000)
				if (response.ok && !response.result.some((candidate) => candidate.id === app.id)) {
					finish()
				}
			} catch {}
		}, 1_000)

		function finish() {
			clearInterval(logTimer)
			clearInterval(statusTimer)
			flushLogs()
			resolve()
		}

		async function stop() {
			if (stopping) return
			stopping = true
			try {
				const response = await sendRequest({ action: 'stop', appId: app.id })
				if (response.ok) console.log(`\n${response.result.message}`)
			} catch (error) {
				console.error(`\nFailed to stop app ${app.id}: ${error}`)
			}
			finish()
		}

		process.once('SIGINT', stop)
		process.once('SIGTERM', stop)
	})
}

function requestForCommand() {
	if (action === 'start' || action === 'start-auth') {
		if (action === 'start-auth' && positional.length > 0) {
			throw new Error('Usage: pnpm app:dev:auth')
		}
		const username = positional[0] ?? 'owner'
		return {
			action: 'start',
			worktree,
			authMode: action === 'start-auth' ? 'real' : 'dev',
			username: action === 'start-auth' ? null : validateDevUsername(username),
		}
	}
	if (action === 'list' || action === 'stop-all') return { action }
	if (action === 'stop-worktree') return { action, worktree }
	if (action === 'account') {
		if (positional.length !== 2) {
			throw new Error('Usage: pnpm app:dev:account -- <app-id> <username>')
		}
		return { action, appId: positional[0], username: validateDevUsername(positional[1]) }
	}
	if (['reload', 'restart', 'stop', 'focus', 'logs'].includes(action)) {
		if (positional.length !== 1) throw new Error(`Usage: pnpm app:dev:${action} -- <app-id>`)
		return { action, appId: positional[0] }
	}
	throw new Error(`Unknown app:dev action "${action}".`)
}

function printResult(command, result) {
	if (jsonOutput || command === 'list') {
		console.log(JSON.stringify(result, null, 2))
		return
	}
	if (command === 'logs') {
		console.log(result.contents)
		return
	}
	console.log(result.message)
}

async function ensureController() {
	let response
	try {
		response = await sendRequest({ action: 'ping' }, 300)
	} catch {}
	if (response?.ok && response.result.protocolVersion === controllerProtocolVersion) return
	if (response?.ok) await replaceOutdatedController(response.result.pid)

	const outputPath = join(appRoot, 'controller.log')
	const output = openSync(outputPath, 'a')
	const child = spawn(process.execPath, [fileURLToPath(import.meta.url), 'serve'], {
		cwd: worktree,
		stdio: ['ignore', output, output],
		detached: true,
		windowsHide: true,
	})
	child.unref()
	closeSync(output)
	const startedAt = Date.now()
	while (Date.now() - startedAt < 5_000) {
		await delay(50)
		try {
			const response = await sendRequest({ action: 'ping' }, 300)
			if (response.ok) return
		} catch {}
	}
	throw new Error(`The app dev controller did not start. See ${outputPath}.`)
}

async function replaceOutdatedController(pid) {
	const response = await sendRequest({ action: 'list' }, 1_000)
	if (!response.ok) throw new Error(response.error)
	if (response.result.length > 0) {
		throw new Error(
			'The app dev controller is outdated. Stop its running apps with "pnpm app:dev:stop-all", then retry.',
		)
	}
	process.kill(pid)
	const startedAt = Date.now()
	while (Date.now() - startedAt < 5_000) {
		await delay(50)
		try {
			await sendRequest({ action: 'ping' }, 100)
		} catch {
			return
		}
	}
	throw new Error('The outdated app dev controller did not stop.')
}

function sendRequest(request, timeoutMs = 15_000) {
	return new Promise((resolveRequest, reject) => {
		const socket = net.createConnection(socketPath)
		let buffer = ''
		const timeout = setTimeout(() => {
			socket.destroy()
			reject(new Error('The app dev controller timed out.'))
		}, timeoutMs)
		socket.setEncoding('utf8')
		socket.once('connect', () => socket.write(`${JSON.stringify(request)}\n`))
		socket.on('data', (chunk) => {
			buffer += chunk
			const newline = buffer.indexOf('\n')
			if (newline === -1) return
			clearTimeout(timeout)
			socket.end()
			try {
				resolveRequest(JSON.parse(buffer.slice(0, newline)))
			} catch {
				reject(new Error('The app dev controller returned invalid data.'))
			}
		})
		socket.once('error', (error) => {
			clearTimeout(timeout)
			reject(error)
		})
	})
}

async function serve() {
	if (process.platform !== 'win32') rmSync(socketPath, { force: true })
	let registry = readRegistry()
	const children = new Map()
	let operationQueue = Promise.resolve()
	const server = net.createServer((socket) => {
		socket.on('error', () => undefined)
		socket.setEncoding('utf8')
		let buffer = ''
		socket.on('data', (chunk) => {
			buffer += chunk
			const newline = buffer.indexOf('\n')
			if (newline === -1) return
			const raw = buffer.slice(0, newline)
			buffer = buffer.slice(newline + 1)
			operationQueue = operationQueue.then(
				() => handleRaw(raw, socket),
				() => handleRaw(raw, socket),
			)
		})
	})

	async function handleRaw(raw, socket) {
		try {
			const request = JSON.parse(raw)
			const result = await handleRequest(request)
			if (!socket.destroyed) socket.end(`${JSON.stringify({ ok: true, result })}\n`)
		} catch (error) {
			if (!socket.destroyed) {
				socket.end(
					`${JSON.stringify({ ok: false, error: error instanceof Error ? error.message : String(error) })}\n`,
				)
			}
		}
	}

	async function handleRequest(request) {
		pruneRegistry()
		if (request.action === 'ping') {
			return { pid: process.pid, protocolVersion: controllerProtocolVersion }
		}
		if (request.action === 'start') {
			return await startApp(request.worktree, request.username, request.authMode)
		}
		if (request.action === 'list') return await publicApps()
		if (request.action === 'account') return await switchAccount(request.appId, request.username)
		if (request.action === 'reload') {
			const app = requireApp(request.appId)
			await control(app, { action: 'reload' })
			return { message: `Reloaded app ${app.id}.` }
		}
		if (request.action === 'restart') return await restartApp(request.appId)
		if (request.action === 'focus') {
			const app = requireApp(request.appId)
			await control(app, { action: 'focus' })
			return { message: `Focused app ${app.id}.` }
		}
		if (request.action === 'stop') return await stopApp(request.appId)
		if (request.action === 'stop-worktree') return await stopWorktree(request.worktree)
		if (request.action === 'stop-all') return await stopAll()
		if (request.action === 'logs') {
			const app = requireApp(request.appId)
			return {
				path: app.logPath,
				contents: existsSync(app.logPath) ? readFileSync(app.logPath, 'utf8') : '',
			}
		}
		throw new Error(`Unknown controller action "${request.action}".`)
	}

	async function startApp(targetWorktree, username, authMode = 'dev') {
		if (authMode === 'dev') validateDevUsername(username)
		if (authMode === 'real') {
			const existing = Object.values(registry.apps).find(
				(app) => app.worktree === targetWorktree && app.authMode === 'real',
			)
			if (existing) {
				throw new Error(
					`Auth app ${existing.id} is already running for this worktree. Stop it before starting another.`,
				)
			}
		}
		const convex = requireConvex(targetWorktree)
		const ids = new Set(Object.keys(registry.apps))
		const id = makeShortId(ids)
		const branch = gitValue(targetWorktree, ['branch', '--show-current']) || 'detached'
		const host = registry.hosts[targetWorktree]
		const vitePort = host?.vitePort ?? (await availablePort(1420))
		const controlPort = await availablePort(17_000)
		const dataDir = appDataDir(authMode === 'real' ? `auth-${worktreeKey(targetWorktree)}` : id)
		const logPath = join(appRoot, 'logs', `${id}.log`)
		mkdirSync(join(appRoot, 'logs'), { recursive: true })
		const identity = authMode === 'real' ? 'auth' : username
		const devConfig = {
			appId: id,
			username,
			authMode,
			branch,
			title: `${branch} · ${id} · ${identity}`,
			dataDir,
			convexUrl: convex.convexUrl,
			convexSiteUrl: convex.convexSiteUrl,
			controlPort,
		}

		let child
		let kind
		if (!host) {
			child = spawnHost(targetWorktree, vitePort, devConfig, logPath)
			kind = 'host'
			registry.hosts[targetWorktree] = {
				launcherPid: child.pid,
				windowsPidPath: child.windowsPidPath,
				vitePort,
				anchorId: id,
			}
		} else {
			const anchor = host.anchorId ? registry.apps[host.anchorId] : null
			if (anchor) await waitForControl(anchor, 60_000)
			else {
				await Promise.all([
					waitForPort(vitePort, 60_000),
					waitForExecutable(targetWorktree, 60_000),
				])
			}
			child = spawnSecondary(targetWorktree, devConfig, logPath)
			kind = 'secondary'
		}
		children.set(id, child)
		registry.apps[id] = {
			id,
			kind,
			pid: child.pid,
			launcherPid: kind === 'host' ? child.pid : undefined,
			controlPort,
			vitePort,
			worktree: targetWorktree,
			branch,
			username,
			authMode,
			dataDir,
			logPath,
			startedAt: new Date().toISOString(),
			ready: false,
		}
		child.once('exit', () => {
			children.delete(id)
			const current = registry.apps[id]
			if (current?.pid === child.pid && current.kind === 'secondary' && !current.restarting) {
				delete registry.apps[id]
				writeRegistry()
			}
		})
		writeRegistry()
		return {
			...publicApp(registry.apps[id]),
			logPath,
			message:
				authMode === 'real'
					? `Started auth app ${id} from ${targetWorktree}.`
					: `Started app ${id} as ${username} from ${targetWorktree}.`,
		}
	}

	async function switchAccount(appId, username) {
		const app = requireApp(appId)
		if (app.authMode === 'real') {
			throw new Error(`App ${app.id} uses real authentication; switch accounts in the app.`)
		}
		validateDevUsername(username)
		await control(app, { action: 'account', username }, 30_000)
		app.username = username
		writeRegistry()
		return { message: `App ${app.id} is now signed in as ${username}.`, ...publicApp(app) }
	}

	async function restartApp(appId) {
		const app = requireApp(appId)
		const otherApps = Object.values(registry.apps).filter(
			(candidate) => candidate.worktree === app.worktree && candidate.id !== app.id,
		)
		if (app.kind === 'host' && otherApps.length > 0) {
			await control(app, { action: 'reload' })
			return {
				message: `Reloaded app ${app.id}; its native watcher stays alive for the other apps.`,
			}
		}

		app.restarting = true
		await control(app, { action: 'close' }).catch(() => undefined)
		await delay(200)
		if (app.kind === 'host') killHost(registry.hosts[app.worktree])
		else killProcessTree(app.pid)
		await waitForAvailablePort(app.controlPort, 5_000)
		if (app.kind === 'host') await waitForAvailablePort(app.vitePort, 5_000)
		const config = devConfigForApp(app)
		const child =
			app.kind === 'host'
				? spawnHost(app.worktree, app.vitePort, config, app.logPath)
				: spawnSecondary(app.worktree, config, app.logPath)
		children.set(app.id, child)
		app.pid = child.pid
		app.ready = false
		delete app.restarting
		registry.apps[app.id] = app
		if (app.kind === 'host') {
			app.launcherPid = child.pid
			registry.hosts[app.worktree].launcherPid = child.pid
			registry.hosts[app.worktree].windowsPidPath = child.windowsPidPath
		}
		writeRegistry()
		return { message: `Restarted app ${app.id}.`, ...publicApp(app) }
	}

	function devConfigForApp(app) {
		const convex = requireConvex(app.worktree)
		return {
			appId: app.id,
			username: app.username,
			authMode: app.authMode ?? 'dev',
			branch: app.branch,
			title: `${app.branch} · ${app.id} · ${app.authMode === 'real' ? 'auth' : app.username}`,
			dataDir: app.dataDir,
			convexUrl: convex.convexUrl,
			convexSiteUrl: convex.convexSiteUrl,
			controlPort: app.controlPort,
		}
	}

	async function stopApp(appId) {
		const app = requireApp(appId)
		const otherApps = Object.values(registry.apps).filter(
			(candidate) => candidate.worktree === app.worktree && candidate.id !== app.id,
		)
		if (app.kind === 'host' && otherApps.length > 0) {
			await control(app, { action: 'hide' }).catch(() => undefined)
			delete registry.apps[app.id]
			registry.hosts[app.worktree].anchorId = null
			writeRegistry()
			return { message: `Stopped app ${app.id}; its hidden dev host remains for the other apps.` }
		}
		await control(app, { action: 'close' }).catch(() => undefined)
		await delay(200)
		if (app.kind === 'host') killHost(registry.hosts[app.worktree])
		else killProcessTree(app.pid)
		delete registry.apps[app.id]
		if (app.kind === 'host') delete registry.hosts[app.worktree]
		if (
			app.kind === 'secondary' &&
			!Object.values(registry.apps).some((candidate) => candidate.worktree === app.worktree) &&
			registry.hosts[app.worktree]?.anchorId === null
		) {
			killHost(registry.hosts[app.worktree])
			delete registry.hosts[app.worktree]
		}
		writeRegistry()
		return { message: `Stopped app ${app.id}.` }
	}

	async function stopWorktree(targetWorktree) {
		const ids = Object.values(registry.apps)
			.filter((app) => app.worktree === targetWorktree)
			.map((app) => app.id)
		for (const id of ids) {
			const app = registry.apps[id]
			if (!app) continue
			await control(app, { action: 'close' }).catch(() => undefined)
			if (app.kind === 'secondary') killProcessTree(app.pid)
			delete registry.apps[id]
		}
		const host = registry.hosts[targetWorktree]
		if (host) killHost(host)
		delete registry.hosts[targetWorktree]
		writeRegistry()
		return { message: `Stopped ${ids.length} app(s) from ${targetWorktree}.` }
	}

	async function stopAll() {
		const count = Object.keys(registry.apps).length
		for (const app of Object.values(registry.apps)) {
			await control(app, { action: 'close' }).catch(() => undefined)
			if (app.kind === 'secondary') killProcessTree(app.pid)
		}
		for (const host of Object.values(registry.hosts)) killHost(host)
		registry = emptyRegistry()
		writeRegistry()
		setTimeout(() => server.close(() => process.exit(0)), 100)
		return { message: `Stopped ${count} app(s).` }
	}

	function pruneRegistry() {
		for (const [worktreePath, host] of Object.entries(registry.hosts)) {
			if (isProcessRunning(host.launcherPid)) continue
			killHost(host)
			delete registry.hosts[worktreePath]
			for (const app of Object.values(registry.apps)) {
				if (app.worktree === worktreePath && app.kind === 'host') delete registry.apps[app.id]
			}
		}
		for (const app of Object.values(registry.apps)) {
			if (app.kind === 'secondary' && !isProcessRunning(app.pid)) delete registry.apps[app.id]
		}
		writeRegistry()
	}

	async function publicApps() {
		for (const app of Object.values(registry.apps)) {
			app.ready = false
			try {
				const response = await control(app, { action: 'status' }, 300)
				app.pid = response.pid
				app.ready = true
			} catch {}
		}
		writeRegistry()
		return Object.values(registry.apps)
			.map(publicApp)
			.sort((a, b) => a.startedAt.localeCompare(b.startedAt))
	}

	function requireApp(id) {
		const app = registry.apps[id]
		if (!app) throw new Error(`No running app has ID ${id}. Run pnpm app:dev:list.`)
		return app
	}

	function requireConvex(targetWorktree) {
		const env = readEnvFile(join(targetWorktree, '.env.local'))
		if (!env.CONVEX_DEPLOYMENT?.startsWith('local:')) {
			const convexUrl = env.CONVEX_URL
			const convexSiteUrl = env.CONVEX_SITE_URL
			if (!convexUrl || !convexSiteUrl) {
				throw new Error(`Convex URLs are missing from ${join(targetWorktree, '.env.local')}.`)
			}
			return { convexUrl, convexSiteUrl }
		}

		const runtimePath = join(targetWorktree, '.convex', 'amberite-runtime.json')
		if (!existsSync(runtimePath)) {
			throw new Error(
				'Local Convex is selected but not running. Start pnpm convex:dev:local first.',
			)
		}
		const state = JSON.parse(readFileSync(runtimePath, 'utf8'))
		if (!isProcessRunning(state.pid)) {
			throw new Error(
				'Local Convex is selected but not running. Start pnpm convex:dev:local first.',
			)
		}
		if (!state.convexUrl || !state.convexSiteUrl) {
			throw new Error(`Local Convex runtime URLs are missing from ${runtimePath}.`)
		}
		return { convexUrl: state.convexUrl, convexSiteUrl: state.convexSiteUrl }
	}

	function writeRegistry() {
		writeFileSync(registryPath, `${JSON.stringify(registry, null, 2)}\n`, { mode: 0o600 })
	}

	await new Promise((resolveServer, reject) => {
		server.once('error', reject)
		server.listen(socketPath, () => {
			if (process.platform !== 'win32') chmodSync(socketPath, 0o600)
			resolveServer()
		})
	})
}

function spawnHost(targetWorktree, vitePort, devConfig, logPath) {
	const tauriConfig = createTauriDevConfig(targetWorktree, vitePort, devConfig)
	const tauriArgs = [
		'dev',
		'--config',
		JSON.stringify(tauriConfig),
		'--',
		'--',
		'--amberite-dev-config',
		JSON.stringify(devConfig),
	]
	if (usesWindowsDesktop() && process.platform !== 'win32') {
		ensureWindowsTauriCli()
		const windowsPidPath = join(appRoot, 'pids', `${devConfig.appId}.pid`)
		mkdirSync(join(appRoot, 'pids'), { recursive: true })
		const paths = windowsPaths(targetWorktree, windowsPidPath)
		const cargoTarget = windowsCargoTargetDir(targetWorktree)
		const child = spawnLogged(
			'node.exe',
			[paths.script, 'windows-host', paths.worktree, paths.pid, ...tauriArgs],
			targetWorktree,
			logPath,
			{
				CARGO_TARGET_DIR: cargoTarget.windows,
				WSLENV: [...(process.env.WSLENV?.split(':') ?? []), 'CARGO_TARGET_DIR'].join(':'),
			},
		)
		child.windowsPidPath = windowsPidPath
		return child
	}
	const extraEnv =
		process.platform === 'win32'
			? { CARGO_TARGET_DIR: windowsCargoTargetDir(targetWorktree).windows }
			: {}
	const { command, args } = commandForPnpm([
		'--filter',
		'@modrinth/app',
		'exec',
		'tauri',
		...tauriArgs,
	])
	return spawnLogged(command, args, targetWorktree, logPath, extraEnv)
}

function spawnSecondary(targetWorktree, devConfig, logPath) {
	const source = join(
		appTargetDir(targetWorktree),
		'debug',
		usesWindowsDesktop() ? 'theseus_gui.exe' : 'theseus_gui',
	)
	if (!existsSync(source)) throw new Error(`The dev executable is not ready at ${source}.`)
	const binDir = join(appRoot, 'bin')
	mkdirSync(binDir, { recursive: true })
	const destination = join(
		binDir,
		usesWindowsDesktop() ? `${devConfig.appId}.exe` : devConfig.appId,
	)
	copyFileSync(source, destination)
	chmodSync(destination, 0o755)
	return spawnLogged(
		destination,
		['--amberite-dev-config', JSON.stringify(devConfig)],
		join(targetWorktree, 'apps', 'app'),
		logPath,
	)
}

function spawnLogged(command, args, cwd, logPath, extraEnv = {}) {
	const log = openSync(logPath, 'a')
	const child = spawn(command, args, {
		cwd,
		stdio: ['ignore', log, log],
		detached: true,
		windowsHide: false,
		env: { ...process.env, ...extraEnv, COREPACK_ENABLE_DOWNLOAD_PROMPT: '0' },
	})
	closeSync(log)
	return child
}

function createTauriDevConfig(targetWorktree, vitePort, devConfig) {
	const tauri = JSON.parse(
		readFileSync(join(targetWorktree, 'apps', 'app', 'tauri.conf.json'), 'utf8'),
	)
	const connectSrc = tauri.app.security.csp['connect-src']
	const convexUrl = new URL(devConfig.convexUrl)
	const convexSiteUrl = new URL(devConfig.convexSiteUrl)
	const localSources = [
		`http://localhost:${vitePort}`,
		`http://127.0.0.1:${vitePort}`,
		`ws://localhost:${vitePort}`,
		`${convexUrl.protocol}//${convexUrl.host}`,
		`${convexUrl.protocol === 'https:' ? 'wss:' : 'ws:'}//${convexUrl.host}`,
		`${convexSiteUrl.protocol}//${convexSiteUrl.host}`,
	]
	const wslWorktree = /\s/.test(targetWorktree) ? JSON.stringify(targetWorktree) : targetWorktree
	return {
		build: {
			devUrl: `http://localhost:${vitePort}`,
			beforeDevCommand: usesWindowsDesktop() && process.platform !== 'win32'
				? `wsl.exe --cd ${wslWorktree} corepack pnpm --filter @modrinth/app-frontend dev -- --port ${vitePort} --strictPort`
				: `corepack pnpm --filter @modrinth/app-frontend dev -- --port ${vitePort} --strictPort`,
		},
		app: {
			security: {
				csp: { 'connect-src': `${connectSrc} ${localSources.join(' ')}` },
			},
		},
	}
}

function windowsPaths(targetWorktree, pidPath) {
	const distroRoot = spawnSync('wslpath', ['-w', '/'], { encoding: 'utf8' })
		.stdout.trim()
		.replace(/[\\/]+$/, '')
	const windowsScript = spawnSync('wslpath', ['-w', fileURLToPath(import.meta.url)], {
		encoding: 'utf8',
	}).stdout.trim()
	const windowsWorktree = spawnSync('wslpath', ['-w', targetWorktree], {
		encoding: 'utf8',
	}).stdout.trim()
	const windowsPidPath = spawnSync('wslpath', ['-w', pidPath], {
		encoding: 'utf8',
	}).stdout.trim()
	if (!distroRoot || !windowsScript || !windowsWorktree || !windowsPidPath) {
		throw new Error('Could not translate the WSL app-dev paths for Windows.')
	}

	const drive = ensureWindowsDrive(distroRoot)
	const toDrivePath = (value) => `${drive}${value.slice(distroRoot.length)}`
	return {
		script: toDrivePath(windowsScript),
		worktree: toDrivePath(windowsWorktree),
		pid: toDrivePath(windowsPidPath),
	}
}

function ensureWindowsDrive(distroRoot) {
	const networkRoot = distroRoot.replace(/^\\\\wsl\.localhost\\/i, '\\\\wsl$\\')
	for (const letter of ['W', 'V', 'U', 'T']) {
		const drive = `${letter}:`
		const current = spawnSync('cmd.exe', ['/d', '/s', '/c', `net use ${drive}`], {
			encoding: 'utf8',
			windowsHide: true,
		})
		if (current.status === 0) {
			if (
				current.stdout.toLowerCase().includes(distroRoot.toLowerCase()) ||
				current.stdout.toLowerCase().includes(networkRoot.toLowerCase())
			) {
				return drive
			}
			continue
		}
		const mapped = spawnSync('net.exe', ['use', drive, networkRoot, '/persistent:no'], {
			encoding: 'utf8',
			windowsHide: true,
		})
		if (mapped.status === 0) return drive
	}
	throw new Error(`Could not map ${distroRoot} to an available Windows drive.`)
}

function usesWindowsDesktop() {
	if (process.platform === 'win32') return true
	return (
		process.platform === 'linux' &&
		readFileSync('/proc/version', 'utf8').toLowerCase().includes('microsoft')
	)
}

function ensureWindowsTauriCli() {
	const localAppData = windowsLocalAppData()
	const windowsTauriScript = `${localAppData}\\amberite-tauri-cli\\node_modules\\@tauri-apps\\cli\\tauri.js`
	const version = spawnSync('node.exe', [windowsTauriScript, '--version'], {
		windowsHide: true,
	})
	if (version.status === 0) return

	const linuxLocalAppData = spawnSync('wslpath', ['-u', localAppData], {
		encoding: 'utf8',
	}).stdout.trim()
	const linuxCliRoot = join(linuxLocalAppData, 'amberite-tauri-cli')
	mkdirSync(linuxCliRoot, { recursive: true })
	if (!existsSync(join(linuxCliRoot, 'package.json'))) {
		spawnSync('cmd.exe', ['/d', '/s', '/c', 'corepack pnpm init --bare'], {
			cwd: linuxCliRoot,
			stdio: 'inherit',
			windowsHide: true,
		})
	}
	const install = spawnSync(
		'cmd.exe',
		[
			'/d',
			'/s',
			'/c',
			'corepack pnpm add @tauri-apps/cli@2.5.0 && corepack pnpm install --config.node-linker=hoisted --force',
		],
		{ cwd: linuxCliRoot, stdio: 'inherit', windowsHide: true },
	)
	const installedVersion = spawnSync('node.exe', [windowsTauriScript, '--version'], {
		windowsHide: true,
	})
	if (install.status !== 0 || installedVersion.status !== 0) {
		throw new Error('Could not install the Windows Tauri CLI bridge.')
	}
}

async function runWindowsHost() {
	const targetWorktree = process.argv[3]
	const pidPath = process.argv[4]
	if (!targetWorktree) throw new Error('The Windows app host is missing its worktree path.')
	if (!pidPath) throw new Error('The Windows app host is missing its PID path.')
	const tauriScript = join(
		process.env.LOCALAPPDATA,
		'amberite-tauri-cli',
		'node_modules',
		'@tauri-apps',
		'cli',
		'tauri.js',
	)
	process.chdir(targetWorktree)
	writeFileSync(pidPath, `${process.pid}\n`)
	const child = spawn(process.execPath, [tauriScript, ...process.argv.slice(5)], {
		stdio: 'inherit',
		env: process.env,
		windowsHide: false,
	})
	const exitCode = await new Promise((resolveExit) => {
		child.once('error', () => resolveExit(1))
		child.once('exit', (code) => resolveExit(code ?? 1))
	})
	rmSync(pidPath, { force: true })
	process.exitCode = exitCode
}

async function runWindowsControl() {
	const port = Number.parseInt(process.argv[3], 10)
	const request = process.argv[4]
	if (!Number.isInteger(port) || port <= 0 || !request) {
		throw new Error('The Windows app control request is invalid.')
	}
	const response = await new Promise((resolveControl, reject) => {
		const socket = net.createConnection({ host: '127.0.0.1', port })
		let buffer = ''
		socket.setEncoding('utf8')
		socket.once('connect', () => socket.write(`${request}\n`))
		socket.on('data', (chunk) => {
			buffer += chunk
			const newline = buffer.indexOf('\n')
			if (newline === -1) return
			socket.end()
			resolveControl(buffer.slice(0, newline))
		})
		socket.once('error', reject)
	})
	process.stdout.write(response)
}

function killHost(host) {
	if (host?.windowsPidPath && existsSync(host.windowsPidPath)) {
		const windowsPid = Number.parseInt(readFileSync(host.windowsPidPath, 'utf8').trim(), 10)
		if (Number.isInteger(windowsPid) && windowsPid > 0) {
			spawnSync('taskkill.exe', ['/PID', String(windowsPid), '/T', '/F'], {
				windowsHide: true,
			})
		}
		rmSync(host.windowsPidPath, { force: true })
	}
	killProcessTree(host?.launcherPid)
}

function control(app, request, timeoutMs = 5_000) {
	if (usesWindowsDesktop() && process.platform !== 'win32') {
		return controlThroughWindows(app, request, timeoutMs)
	}
	return new Promise((resolveControl, reject) => {
		const socket = net.createConnection({ host: '127.0.0.1', port: app.controlPort })
		let buffer = ''
		const timeout = setTimeout(() => {
			socket.destroy()
			reject(new Error(`App ${app.id} did not respond.`))
		}, timeoutMs)
		socket.setEncoding('utf8')
		socket.once('connect', () => socket.write(`${JSON.stringify(request)}\n`))
		socket.on('data', (chunk) => {
			buffer += chunk
			const newline = buffer.indexOf('\n')
			if (newline === -1) return
			clearTimeout(timeout)
			socket.end()
			const response = JSON.parse(buffer.slice(0, newline))
			if (response.ok) resolveControl(response)
			else reject(new Error(response.error || `App ${app.id} rejected the command.`))
		})
		socket.once('error', (error) => {
			clearTimeout(timeout)
			reject(new Error(`App ${app.id} is not ready: ${error.message}`))
		})
	})
}

function controlThroughWindows(app, request, timeoutMs) {
	return new Promise((resolveControl, reject) => {
		const paths = windowsPaths(app.worktree, join(appRoot, 'windows-control.pid'))
		const child = spawn(
			'node.exe',
			[paths.script, 'windows-control', String(app.controlPort), JSON.stringify(request)],
			{ stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true },
		)
		let stdout = ''
		let stderr = ''
		const timeout = setTimeout(() => {
			child.kill()
			reject(new Error(`App ${app.id} did not respond.`))
		}, timeoutMs)
		child.stdout.setEncoding('utf8')
		child.stderr.setEncoding('utf8')
		child.stdout.on('data', (chunk) => (stdout += chunk))
		child.stderr.on('data', (chunk) => (stderr += chunk))
		child.once('error', (error) => {
			clearTimeout(timeout)
			reject(error)
		})
		child.once('exit', (code) => {
			clearTimeout(timeout)
			if (code !== 0) {
				reject(new Error(stderr.trim() || `App ${app.id} is not ready.`))
				return
			}
			try {
				const response = JSON.parse(stdout.trim())
				if (response.ok) resolveControl(response)
				else reject(new Error(response.error || `App ${app.id} rejected the command.`))
			} catch {
				reject(new Error(`App ${app.id} returned an invalid control response.`))
			}
		})
	})
}

async function availablePort(start) {
	for (let port = start; port < start + 1_000; port += 1) {
		if (await portAvailable(port)) return port
	}
	throw new Error(`No available port found from ${start}.`)
}

function portAvailable(port) {
	return new Promise((resolvePort) => {
		const server = net.createServer()
		server.unref()
		server.once('error', () => resolvePort(false))
		server.listen({ host: '127.0.0.1', port }, () => server.close(() => resolvePort(true)))
	})
}

async function waitForPort(port, timeoutMs) {
	const startedAt = Date.now()
	while (Date.now() - startedAt < timeoutMs) {
		if (!(await portAvailable(port))) return
		await delay(250)
	}
	throw new Error(`Timed out waiting for Vite on port ${port}.`)
}

async function waitForAvailablePort(port, timeoutMs) {
	const startedAt = Date.now()
	while (Date.now() - startedAt < timeoutMs) {
		if (await portAvailable(port)) return
		await delay(50)
	}
	throw new Error(`Timed out waiting to reuse port ${port}.`)
}

async function waitForExecutable(targetWorktree, timeoutMs) {
	const executable = join(
		appTargetDir(targetWorktree),
		'debug',
		usesWindowsDesktop() ? 'theseus_gui.exe' : 'theseus_gui',
	)
	const startedAt = Date.now()
	while (Date.now() - startedAt < timeoutMs) {
		if (existsSync(executable)) return
		await delay(250)
	}
	throw new Error(`Timed out waiting for the dev executable at ${executable}.`)
}

function appTargetDir(targetWorktree) {
	if (usesWindowsDesktop() && process.platform !== 'win32') {
		return windowsCargoTargetDir(targetWorktree).linux
	}
	return join(targetWorktree, 'target', usesWindowsDesktop() ? 'app-dev-windows' : '')
}

function windowsCargoTargetDir(targetWorktree) {
	const windows = `${windowsLocalAppData()}\\amberite-dev\\cargo-targets\\${worktreeKey(targetWorktree)}`
	if (process.platform === 'win32') return { windows, linux: windows }
	const linux = spawnSync('wslpath', ['-u', windows], { encoding: 'utf8' }).stdout.trim()
	if (!linux) throw new Error('Could not translate the Windows Cargo target directory.')
	return { windows, linux }
}

function windowsLocalAppData() {
	const value = spawnSync('cmd.exe', ['/d', '/s', '/c', 'echo %LOCALAPPDATA%'], {
		encoding: 'utf8',
		windowsHide: true,
	}).stdout.trim()
	if (!value) throw new Error('Windows did not report its local application data directory.')
	return value
}

function appDataDir(appId) {
	if (!usesWindowsDesktop() || process.platform === 'win32') {
		const dataDir = join(appRoot, 'data', appId)
		mkdirSync(dataDir, { recursive: true })
		return dataDir
	}

	const repository = basename(join(appRoot, '..'))
	const windows = `${windowsLocalAppData()}\\amberite-dev\\${repository}\\apps\\data\\${appId}`
	const linux = spawnSync('wslpath', ['-u', windows], { encoding: 'utf8' }).stdout.trim()
	if (!linux) throw new Error('Could not translate the Windows app data directory.')
	mkdirSync(linux, { recursive: true })
	return windows
}

async function waitForControl(app, timeoutMs) {
	const startedAt = Date.now()
	while (Date.now() - startedAt < timeoutMs) {
		try {
			await control(app, { action: 'status' }, 300)
			return
		} catch {}
		await delay(250)
	}
	throw new Error(`Timed out waiting for app ${app.id} to finish its native dev build.`)
}

function publicApp(app) {
	return {
		id: app.id,
		pid: app.pid,
		health: app.ready
			? 'running'
			: isProcessRunning(app.launcherPid ?? app.pid)
				? 'starting'
				: 'stopped',
		username: app.username,
		authMode: app.authMode ?? 'dev',
		branch: app.branch,
		worktree: app.worktree,
		vitePort: app.vitePort,
		nativeWatcherOwner: app.kind === 'host',
		startedAt: app.startedAt,
	}
}

function readRegistry() {
	if (!existsSync(registryPath)) return emptyRegistry()
	try {
		const value = JSON.parse(readFileSync(registryPath, 'utf8'))
		return value?.apps && value?.hosts ? value : emptyRegistry()
	} catch {
		return emptyRegistry()
	}
}

function emptyRegistry() {
	return { apps: {}, hosts: {} }
}

function delay(ms) {
	return new Promise((resolveDelay) => setTimeout(resolveDelay, ms))
}
