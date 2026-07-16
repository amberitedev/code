#!/usr/bin/env node
import { spawn } from 'node:child_process'
import {
	chmodSync,
	closeSync,
	copyFileSync,
	existsSync,
	mkdirSync,
	openSync,
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
} from './dev-shared.mjs'

const action = process.argv[2] ?? 'start'
const jsonOutput = process.argv.includes('--json')
const positional = process.argv.slice(3).filter((value) => value !== '--' && value !== '--json')
const worktree = findWorktreeRoot()
const root = globalDevRoot(worktree)
const appRoot = join(root, 'apps')
const registryPath = join(appRoot, 'registry.json')
const socketPath =
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

async function runClient() {
	const request = requestForCommand()
	await ensureController()
	const response = await sendRequest(request)
	if (!response.ok) throw new Error(response.error)
	printResult(action, response.result)
}

function requestForCommand() {
	if (action === 'start') {
		const username = positional[0]
		if (!username) throw new Error('Usage: pnpm app:dev -- <username>')
		return { action, worktree, username: validateDevUsername(username) }
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
	try {
		const response = await sendRequest({ action: 'ping' }, 300)
		if (response.ok) return
	} catch {}

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
			socket.end(`${JSON.stringify({ ok: true, result })}\n`)
		} catch (error) {
			socket.end(
				`${JSON.stringify({ ok: false, error: error instanceof Error ? error.message : String(error) })}\n`,
			)
		}
	}

	async function handleRequest(request) {
		pruneRegistry()
		if (request.action === 'ping') return { pid: process.pid }
		if (request.action === 'start') return await startApp(request.worktree, request.username)
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

	async function startApp(targetWorktree, username) {
		validateDevUsername(username)
		const convex = requireConvex(targetWorktree)
		const ids = new Set(Object.keys(registry.apps))
		const id = makeShortId(ids)
		const branch = gitValue(targetWorktree, ['branch', '--show-current']) || 'detached'
		const host = registry.hosts[targetWorktree]
		const vitePort = host?.vitePort ?? (await availablePort(1420))
		const controlPort = await availablePort(17_000)
		const dataDir = join(appRoot, 'data', id)
		const logPath = join(appRoot, 'logs', `${id}.log`)
		mkdirSync(dataDir, { recursive: true })
		mkdirSync(join(appRoot, 'logs'), { recursive: true })
		const devConfig = {
			appId: id,
			username,
			branch,
			title: `${branch} · ${id} · ${username}`,
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
			registry.hosts[targetWorktree] = { launcherPid: child.pid, vitePort, anchorId: id }
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
			message: `Started app ${id} as ${username} from ${targetWorktree}.`,
		}
	}

	async function switchAccount(appId, username) {
		const app = requireApp(appId)
		validateDevUsername(username)
		await control(app, { action: 'account', username }, 12_000)
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
		killProcessTree(app.launcherPid ?? app.pid)
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
		}
		writeRegistry()
		return { message: `Restarted app ${app.id}.`, ...publicApp(app) }
	}

	function devConfigForApp(app) {
		const convex = requireConvex(app.worktree)
		return {
			appId: app.id,
			username: app.username,
			branch: app.branch,
			title: `${app.branch} · ${app.id} · ${app.username}`,
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
		killProcessTree(app.launcherPid ?? app.pid)
		delete registry.apps[app.id]
		if (app.kind === 'host') delete registry.hosts[app.worktree]
		if (
			app.kind === 'secondary' &&
			!Object.values(registry.apps).some((candidate) => candidate.worktree === app.worktree) &&
			registry.hosts[app.worktree]?.anchorId === null
		) {
			killProcessTree(registry.hosts[app.worktree].launcherPid)
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
		if (host) killProcessTree(host.launcherPid)
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
		for (const host of Object.values(registry.hosts)) killProcessTree(host.launcherPid)
		registry = emptyRegistry()
		writeRegistry()
		setTimeout(() => server.close(() => process.exit(0)), 100)
		return { message: `Stopped ${count} app(s).` }
	}

	function pruneRegistry() {
		for (const [worktreePath, host] of Object.entries(registry.hosts)) {
			if (isProcessRunning(host.launcherPid)) continue
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
			throw new Error('Local Convex is selected but not running. Start pnpm convex:dev:local first.')
		}
		const state = JSON.parse(readFileSync(runtimePath, 'utf8'))
		if (!isProcessRunning(state.pid)) {
			throw new Error('Local Convex is selected but not running. Start pnpm convex:dev:local first.')
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
	const { command, args } = commandForPnpm([
		'--filter',
		'@modrinth/app',
		'exec',
		'tauri',
		'dev',
		'--config',
		JSON.stringify(tauriConfig),
		'--',
		'--',
		'--amberite-dev-config',
		JSON.stringify(devConfig),
	])
	return spawnLogged(command, args, targetWorktree, logPath)
}

function spawnSecondary(targetWorktree, devConfig, logPath) {
	const source = join(
		targetWorktree,
		'target',
		'debug',
		process.platform === 'win32' ? 'theseus_gui.exe' : 'theseus_gui',
	)
	if (!existsSync(source)) throw new Error(`The dev executable is not ready at ${source}.`)
	const binDir = join(appRoot, 'bin')
	mkdirSync(binDir, { recursive: true })
	const destination = join(
		binDir,
		process.platform === 'win32' ? `${devConfig.appId}.exe` : devConfig.appId,
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

function spawnLogged(command, args, cwd, logPath) {
	const log = openSync(logPath, 'a')
	const child = spawn(command, args, {
		cwd,
		stdio: ['ignore', log, log],
		detached: true,
		windowsHide: false,
		env: { ...process.env, COREPACK_ENABLE_DOWNLOAD_PROMPT: '0' },
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
	return {
		build: {
			devUrl: `http://localhost:${vitePort}`,
			beforeDevCommand: `corepack pnpm --filter @modrinth/app-frontend dev -- --port ${vitePort} --strictPort`,
		},
		app: {
			security: {
				csp: { 'connect-src': `${connectSrc} ${localSources.join(' ')}` },
			},
		},
	}
}

function control(app, request, timeoutMs = 5_000) {
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
		targetWorktree,
		'target',
		'debug',
		process.platform === 'win32' ? 'theseus_gui.exe' : 'theseus_gui',
	)
	const startedAt = Date.now()
	while (Date.now() - startedAt < timeoutMs) {
		if (existsSync(executable)) return
		await delay(250)
	}
	throw new Error(`Timed out waiting for the dev executable at ${executable}.`)
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
		health: app.ready ? 'running' : isProcessRunning(app.pid) ? 'starting' : 'stopped',
		username: app.username,
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
