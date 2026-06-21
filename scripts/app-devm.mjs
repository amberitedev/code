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
const secondDebugExe = join(
	root,
	'target',
	'debug',
	process.platform === 'win32' ? 'theseus_gui-amber-2.exe' : 'theseus_gui-amber-2',
)
const secondProfile = {
	name: 'amber-2',
	configDir: join(devmDataRoot, 'amber-2'),
}
const primaryAuthEnv = {
	AMBERITE_DEV_MODE: 'true',
	AMBERITE_DEV_AUTH_SCOPE: 'owner',
}
const secondAuthEnv = {
	AMBERITE_DEV_MODE: 'true',
	AMBERITE_DEV_AUTH_SCOPE: 'friend',
	AMBERITE_DEV_PERSONA_ID: 'theogib2',
}

const children = new Set()
const expectedExits = new Set()
let shuttingDown = false
let secondChild = null
let secondExeMtime = null
let secondWatcher = null
let secondStartTime = null
let secondRetryTimer = null
let secondRestartPending = false
let waitingLogged = false

process.stdout.on('error', () => {})
process.stderr.on('error', () => {})

function prefixOutput(stream, prefix, chunk) {
	const text = chunk.toString()
	for (const line of text.split(/\r?\n/)) {
		if (line.length > 0 && !stream.destroyed) {
			stream.write(`[${prefix}] ${line}\n`)
		}
	}
}

function pnpmCommand() {
	const command = process.platform === 'win32' && process.env.APPDATA
		? process.execPath
		: 'pnpm'
	const pnpmArgs = process.platform === 'win32' && process.env.APPDATA
		? [join(process.env.APPDATA, 'npm', 'node_modules', 'pnpm', 'bin', 'pnpm.cjs')]
		: []

	return { command, pnpmArgs }
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
	const { command, pnpmArgs } = pnpmCommand()
	const child = spawn(
		command,
		[
			...pnpmArgs,
			'app:dev',
		],
		{
			stdio: ['inherit', 'pipe', 'pipe'],
			env: {
				...process.env,
				...primaryAuthEnv,
				AMBERITE_DISABLE_SINGLE_INSTANCE: '1',
				AMBERITE_DEVM_NAME: 'Amberite 1',
				RUST_LOG: process.env.RUST_LOG ?? 'info',
			},
		},
	)

	children.add(child)
	child.stdout.on('data', (chunk) => prefixOutput(process.stdout, 'amber-1', chunk))
	child.stderr.on('data', (chunk) => prefixOutput(process.stderr, 'amber-1', chunk))
	child.on('exit', (code, signal) => {
		children.delete(child)
		const reason = signal ? `signal ${signal}` : `code ${code}`
		console.log(`[amber-1] exited with ${reason}`)
		if (!shuttingDown && children.size > 0) shutdown(1)
	})

	console.log('[amber-1] starting normal pnpm app:dev')
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

function prepareSecondExe() {
	if (!existsSync(debugExe)) return false
	try {
		copyFileSync(debugExe, secondDebugExe)
		return true
	} catch (error) {
		console.log(`[${secondProfile.name}] waiting for debug exe copy: ${error.message}`)
		return false
	}
}

function startSecondFromExe() {
	if (!existsSync(debugExe)) return false
	const sourceMtime = statSync(debugExe).mtimeMs
	if (!prepareSecondExe()) return false
	secondExeMtime = sourceMtime
	secondStartTime = Date.now()

	const child = spawn(
		secondDebugExe,
		[],
		{
			cwd: join(root, 'apps', 'app'),
			stdio: ['inherit', 'pipe', 'pipe'],
			env: {
				...process.env,
				...secondAuthEnv,
				AMBERITE_DISABLE_SINGLE_INSTANCE: '1',
				AMBERITE_DEVM_NAME: 'Amberite 2',
				THESEUS_CONFIG_DIR: secondProfile.configDir,
				WEBVIEW2_USER_DATA_FOLDER: join(secondProfile.configDir, 'webview2'),
				RUST_LOG: process.env.RUST_LOG ?? 'info',
			},
		},
	)

	secondChild = child
	children.add(child)
	child.stdout.on('data', (chunk) => prefixOutput(process.stdout, secondProfile.name, chunk))
	child.stderr.on('data', (chunk) => prefixOutput(process.stderr, secondProfile.name, chunk))
	child.on('exit', (code, signal) => {
		children.delete(child)
		if (secondChild === child) secondChild = null
		const reason = signal ? `signal ${signal}` : `code ${code}`
		console.log(`[${secondProfile.name}] exited with ${reason}`)
		if (expectedExits.delete(child)) {
			if (secondRestartPending && !shuttingDown) {
				secondRestartPending = false
				scheduleSecondRetry()
			}
			return
		}
		if (!shuttingDown && children.size > 0) {
			const lifetime = secondStartTime ? Date.now() - secondStartTime : 0
			if (lifetime < 10_000) {
				console.log(`[${secondProfile.name}] exited quickly; will retry after the dev build is ready`)
				scheduleSecondRetry()
				return
			}
			shutdown(1)
		}
	})

	console.log(`[${secondProfile.name}] starting from ${secondDebugExe} with app data at ${secondProfile.configDir}`)
	return true
}

function startSecondWhenReady() {
	const wait = setInterval(async () => {
		if (shuttingDown) {
			clearInterval(wait)
			return
		}

		if (!waitingLogged) {
			console.log(`[${secondProfile.name}] waiting for ${debugExe} and Vite on localhost:1420`)
			waitingLogged = true
		}

		if (existsSync(debugExe) && await isPortOpen(1420) && !secondChild && startSecondFromExe()) {
			clearInterval(wait)
			startSecondExeWatcher()
		}
	}, 1000)
}

function scheduleSecondRetry() {
	if (secondRetryTimer) return
	secondRetryTimer = setTimeout(() => {
		secondRetryTimer = null
		if (!shuttingDown && !secondChild && existsSync(debugExe)) {
			startSecondFromExe()
		}
	}, 2000)
}

function startSecondExeWatcher() {
	secondWatcher = setInterval(() => {
		if (shuttingDown || !existsSync(debugExe)) return

		const mtime = statSync(debugExe).mtimeMs
		if (secondExeMtime !== null && mtime !== secondExeMtime) {
			if (secondRestartPending) return
			console.log(`[${secondProfile.name}] debug exe changed; restarting second app`)
			if (secondChild) {
				secondRestartPending = true
				restartChild(secondChild)
				return
			}
			startSecondFromExe()
		}
	}, 1500)
}

function shutdown(exitCode = 0) {
	if (shuttingDown) return
	shuttingDown = true

	for (const child of children) {
		killChild(child)
	}
	if (secondWatcher) clearInterval(secondWatcher)
	if (secondRetryTimer) clearTimeout(secondRetryTimer)
	secondRestartPending = false

	setTimeout(() => process.exit(exitCode), 500).unref()
}

async function main() {
	process.on('SIGINT', () => shutdown(0))
	process.on('SIGTERM', () => shutdown(0))
	process.on('SIGHUP', () => shutdown(0))

	console.log('Starting multiplayer Amberite dev.')
	console.log('Amberite 1 runs normal pnpm app:dev; Amberite 2 runs from a copied debug exe with separate app data.')
	console.log('Amberite 1 uses the canonical Minecraft account; Amberite 2 uses the Theogib2 dev persona.')

	if (await isPortOpen(1420)) {
		const owner = findWindowsPortOwner(1420)
		throw new Error(
			`Port 1420 is already in use${owner ? ` by ${owner}` : ''}. Stop the existing app/frontend dev process before running app:devm.`,
		)
	}

	startPrimaryDev()
	startSecondWhenReady()
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	main().catch((error) => {
		console.error(error.stack ?? error.message)
		shutdown(1)
	})
}
