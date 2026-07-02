#!/usr/bin/env node
import { spawn, spawnSync } from 'node:child_process'
import { copyFileSync, existsSync, statSync } from 'node:fs'
import net from 'node:net'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const devmDataRoot = process.env.LOCALAPPDATA
	? join(process.env.LOCALAPPDATA, 'Amberite', 'devm')
	: join(root, '.devm')
const debugExe = join(root, 'target', 'debug', process.platform === 'win32' ? 'theseus_gui.exe' : 'theseus_gui')
const defaultPlayerCount = 2
const maxPlayerCount = 9
const primaryAuthEnv = {
	AMBERITE_DEV_MODE: 'true',
	AMBERITE_DEV_AUTH_SCOPE: 'owner',
}
let secondaryProfiles = []

const children = new Set()
const expectedExits = new Set()
let shuttingDown = false
let primaryChild = null
let secondaryWatcher = null
let primaryRetryTimer = null

process.stdout.on('error', () => {})
process.stderr.on('error', () => {})

function parsePlayerCount() {
	const raw = process.argv[2] ?? process.env.AMBERITE_DEVM_PLAYERS ?? String(defaultPlayerCount)
	const value = raw.startsWith('--players=') ? raw.slice('--players='.length) : raw
	const playerCount = Number(value)
	if (!Number.isInteger(playerCount) || playerCount < defaultPlayerCount || playerCount > maxPlayerCount) {
		throw new Error(`Player count must be an integer from ${defaultPlayerCount} to ${maxPlayerCount}.`)
	}
	return playerCount
}

function commandLabel(playerCount) {
	if (process.env.npm_lifecycle_event) return process.env.npm_lifecycle_event
	return playerCount === defaultPlayerCount ? 'app:devm' : `app:devm -- ${playerCount}`
}

function secondaryDebugExe(number) {
	return join(
		root,
		'target',
		'debug',
		process.platform === 'win32' ? `theseus_gui-amber-${number}.exe` : `theseus_gui-amber-${number}`,
	)
}

function secondaryAuthScope(number) {
	return number === 2 ? 'friend' : `friend-${number}`
}

function secondaryPersonaId(number) {
	return `theogib${number}`
}

function buildSecondaryProfiles(playerCount) {
	return Array.from({ length: playerCount - 1 }, (_, index) => {
		const number = index + 2
		return {
			number,
			name: `amber-${number}`,
			displayName: `Amberite ${number}`,
			configDir: join(devmDataRoot, `amber-${number}`),
			debugExe: secondaryDebugExe(number),
			authEnv: {
				AMBERITE_DEV_MODE: 'true',
				AMBERITE_DEV_AUTH_SCOPE: secondaryAuthScope(number),
				AMBERITE_DEV_PERSONA_ID: secondaryPersonaId(number),
			},
			child: null,
			exeMtime: null,
			startTime: null,
			retryTimer: null,
			restartPending: false,
			waitTimer: null,
			waitingLogged: false,
		}
	})
}

function secondaryProfileSummary() {
	return secondaryProfiles
		.map((profile) => `${profile.displayName} uses the ${profile.authEnv.AMBERITE_DEV_PERSONA_ID} dev persona`)
		.join('; ')
}

function prefixOutput(stream, prefix, chunk) {
	const text = chunk.toString()
	for (const line of text.split(/\r?\n/)) {
		if (line.length > 0 && !stream.destroyed) {
			stream.write(`[${prefix}] ${line}\n`)
		}
	}
}

function pnpmCommand(args) {
	if (process.platform === 'win32') {
		return {
			command: 'cmd.exe',
			args: ['/d', '/s', '/c', ['corepack', 'pnpm', ...args].join(' ')],
		}
	}

	return {
		command: 'corepack',
		args: ['pnpm', ...args],
	}
}

function killChild(child) {
	if (!child || child.killed) return
	if (process.platform === 'win32') {
		spawnSync('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], {
			windowsHide: true,
		})
	} else {
		child.kill()
	}
}

function restartChild(child) {
	if (child) expectedExits.add(child)
	killChild(child)
}

function isPortOpenOnHost(port, host) {
	return new Promise((resolve) => {
		const socket = new net.Socket()
		socket.setTimeout(1000)
		socket.once('connect', () => {
			socket.destroy()
			resolve(true)
		})
		socket.once('error', () => resolve(false))
		socket.once('timeout', () => {
			socket.destroy()
			resolve(false)
		})
		socket.connect(port, host)
	})
}

async function isPortOpen(port) {
	return await isPortOpenOnHost(port, 'localhost')
		|| await isPortOpenOnHost(port, '127.0.0.1')
		|| await isPortOpenOnHost(port, '::1')
}

function startPrimaryDev() {
	const { command, args } = pnpmCommand([
		'--filter',
		'@modrinth/app',
		'dev',
	])
	const child = spawn(
		command,
		args,
		{
			stdio: ['inherit', 'pipe', 'pipe'],
			env: {
				...process.env,
				COREPACK_ENABLE_DOWNLOAD_PROMPT: '0',
				...primaryAuthEnv,
				AMBERITE_DISABLE_SINGLE_INSTANCE: '1',
				AMBERITE_DEVM_NAME: 'Amberite 1',
				RUST_LOG: process.env.RUST_LOG ?? 'info',
			},
		},
	)

	primaryChild = child
	children.add(child)
	child.stdout.on('data', (chunk) => prefixOutput(process.stdout, 'amber-1', chunk))
	child.stderr.on('data', (chunk) => prefixOutput(process.stderr, 'amber-1', chunk))
	child.on('exit', (code, signal) => {
		children.delete(child)
		if (primaryChild === child) primaryChild = null
		const reason = signal ? `signal ${signal}` : `code ${code}`
		console.log(`[amber-1] exited with ${reason}`)
		if (!shuttingDown && (signal || code !== 0)) {
			console.log('[amber-1] restarting after unexpected exit')
			schedulePrimaryRetry()
		}
	})

	console.log('[amber-1] starting app dev task')
}

function schedulePrimaryRetry() {
	if (primaryRetryTimer) return
	primaryRetryTimer = setTimeout(() => {
		primaryRetryTimer = null
		if (!shuttingDown && !primaryChild) startPrimaryDev()
	}, 2000)
}

function findWindowsPortOwner(port) {
	if (process.platform !== 'win32') return null
	const result = spawnSync(
		'powershell.exe',
		[
			'-NoLogo',
			'-NoProfile',
			'-NonInteractive',
			'-Command',
			`$c = Get-NetTCPConnection -LocalPort ${port} -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1; if ($c) { $p = Get-Process -Id $c.OwningProcess -ErrorAction SilentlyContinue; if ($p) { "$($p.Id) $($p.ProcessName)" } else { "$($c.OwningProcess)" } }`,
		],
		{ encoding: 'utf8', windowsHide: true, timeout: 5000 },
	)
	return result.stdout.trim() || null
}

function prepareSecondaryExe(profile) {
	if (!existsSync(debugExe)) return false
	try {
		copyFileSync(debugExe, profile.debugExe)
		return true
	} catch (error) {
		console.log(`[${profile.name}] waiting for debug exe copy: ${error.message}`)
		return false
	}
}

function startSecondaryFromExe(profile) {
	if (!existsSync(debugExe)) return false
	const sourceMtime = statSync(debugExe).mtimeMs
	if (!prepareSecondaryExe(profile)) return false
	profile.exeMtime = sourceMtime
	profile.startTime = Date.now()

	const child = spawn(
		profile.debugExe,
		[],
		{
			cwd: join(root, 'apps', 'app'),
			stdio: ['inherit', 'pipe', 'pipe'],
			env: {
				...process.env,
				...profile.authEnv,
				AMBERITE_DISABLE_SINGLE_INSTANCE: '1',
				AMBERITE_DEVM_NAME: profile.displayName,
				THESEUS_CONFIG_DIR: profile.configDir,
				WEBVIEW2_USER_DATA_FOLDER: join(profile.configDir, 'webview2'),
				RUST_LOG: process.env.RUST_LOG ?? 'info',
			},
		},
	)

	profile.child = child
	children.add(child)
	child.stdout.on('data', (chunk) => prefixOutput(process.stdout, profile.name, chunk))
	child.stderr.on('data', (chunk) => prefixOutput(process.stderr, profile.name, chunk))
	child.on('exit', (code, signal) => {
		children.delete(child)
		if (profile.child === child) profile.child = null
		const reason = signal ? `signal ${signal}` : `code ${code}`
		console.log(`[${profile.name}] exited with ${reason}`)
		if (expectedExits.delete(child)) {
			if (profile.restartPending && !shuttingDown) {
				profile.restartPending = false
				scheduleSecondaryRetry(profile)
			}
			return
		}
		if (!shuttingDown && (signal || code !== 0)) {
			const lifetime = profile.startTime ? Date.now() - profile.startTime : 0
			if (lifetime < 10_000) {
				console.log(`[${profile.name}] exited quickly; will retry after the dev build is ready`)
				scheduleSecondaryRetry(profile)
				return
			}
			console.log(`[${profile.name}] restarting after unexpected exit`)
			scheduleSecondaryRetry(profile)
		}
	})

	console.log(`[${profile.name}] starting from ${profile.debugExe} with app data at ${profile.configDir}`)
	return true
}

function startSecondaryWhenReady(profile) {
	if (profile.waitTimer) return
	profile.waitTimer = setInterval(async () => {
		if (shuttingDown) {
			clearInterval(profile.waitTimer)
			profile.waitTimer = null
			return
		}

		if (!profile.waitingLogged) {
			console.log(`[${profile.name}] waiting for ${debugExe} and Vite on localhost:1420`)
			profile.waitingLogged = true
		}

		if (existsSync(debugExe) && await isPortOpen(1420) && !profile.child && startSecondaryFromExe(profile)) {
			clearInterval(profile.waitTimer)
			profile.waitTimer = null
			startSecondaryExeWatcher()
		}
	}, 1000)
}

function scheduleSecondaryRetry(profile) {
	if (profile.retryTimer) return
	profile.retryTimer = setTimeout(() => {
		profile.retryTimer = null
		if (shuttingDown || profile.child) return
		if (existsSync(debugExe)) {
			if (!startSecondaryFromExe(profile)) scheduleSecondaryRetry(profile)
		} else {
			startSecondaryWhenReady(profile)
		}
	}, 2000)
}

function startSecondaryExeWatcher() {
	if (secondaryWatcher) return
	secondaryWatcher = setInterval(() => {
		if (shuttingDown || !existsSync(debugExe)) return

		const mtime = statSync(debugExe).mtimeMs
		for (const profile of secondaryProfiles) {
			if (profile.exeMtime !== null && mtime !== profile.exeMtime) {
				if (profile.restartPending) continue
				console.log(`[${profile.name}] debug exe changed; restarting app`)
				if (profile.child) {
					profile.restartPending = true
					restartChild(profile.child)
					continue
				}
				startSecondaryFromExe(profile)
			}
		}
	}, 1500)
}

function shutdown(exitCode = 0) {
	if (shuttingDown) return
	shuttingDown = true

	for (const child of children) {
		killChild(child)
	}
	if (secondaryWatcher) clearInterval(secondaryWatcher)
	if (primaryRetryTimer) clearTimeout(primaryRetryTimer)
	for (const profile of secondaryProfiles) {
		if (profile.retryTimer) clearTimeout(profile.retryTimer)
		if (profile.waitTimer) clearInterval(profile.waitTimer)
		profile.restartPending = false
	}

	setTimeout(() => process.exit(exitCode), 500).unref()
}

async function main() {
	const playerCount = parsePlayerCount()
	secondaryProfiles = buildSecondaryProfiles(playerCount)

	process.on('SIGINT', () => shutdown(0))
	process.on('SIGTERM', () => shutdown(0))
	process.on('SIGHUP', () => shutdown(0))

	console.log(`Starting ${playerCount}-player Amberite dev.`)
	console.log(`All ${playerCount} apps load the shared Vite dev server, so frontend changes hot reload in each window.`)
	console.log('Amberite 1 runs the normal app dev task; additional players run from copied debug exes with separate app data.')
	console.log(`Amberite 1 uses the canonical Minecraft account; ${secondaryProfileSummary()}.`)

	if (await isPortOpen(1420)) {
		const owner = findWindowsPortOwner(1420)
		throw new Error(
			`Port 1420 is already in use${owner ? ` by ${owner}` : ''}. Stop the existing app/frontend dev process before running ${commandLabel(playerCount)}.`,
		)
	}

	startPrimaryDev()
	for (const profile of secondaryProfiles) {
		startSecondaryWhenReady(profile)
	}
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	main().catch((error) => {
		console.error(error.stack ?? error.message)
		shutdown(1)
	})
}
