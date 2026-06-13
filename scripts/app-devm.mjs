#!/usr/bin/env node
import { spawn, spawnSync } from 'node:child_process'
import net from 'node:net'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')

const profiles = [
	{
		name: 'amber-a',
		identifier: 'amberite.dev.a',
		productName: 'Amberite Dev A',
		port: 1421,
		targetDir: join(root, 'apps', 'app', 'target', 'devm-a'),
	},
	{
		name: 'amber-b',
		identifier: 'amberite.dev.b',
		productName: 'Amberite Dev B',
		port: 1422,
		targetDir: join(root, 'apps', 'app', 'target', 'devm-b'),
	},
]

const children = new Set()
let shuttingDown = false

function configFor(profile) {
	return JSON.stringify({
		identifier: profile.identifier,
		productName: profile.productName,
		build: {
			devUrl: `http://localhost:${profile.port}`,
		},
	})
}

function prefixOutput(stream, prefix, chunk) {
	const text = chunk.toString()
	for (const line of text.split(/\r?\n/)) {
		if (line.length > 0) stream.write(`[${prefix}] ${line}\n`)
	}
}

function isPortOpen(port) {
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
		socket.connect(port, '127.0.0.1')
	})
}

async function findFreePort(startPort, reservedPorts) {
	let port = startPort
	while ((await isPortOpen(port)) || reservedPorts.has(port)) {
		port++
	}
	reservedPorts.add(port)
	return port
}

function startProfile(profile) {
	const command = process.platform === 'win32' && process.env.APPDATA
		? process.execPath
		: 'pnpm'
	const pnpmArgs = process.platform === 'win32' && process.env.APPDATA
		? [join(process.env.APPDATA, 'npm', 'node_modules', 'pnpm', 'bin', 'pnpm.cjs')]
		: []
	const child = spawn(
		command,
		[
			...pnpmArgs,
			'--filter',
			'@modrinth/app',
			'tauri',
			'dev',
			'--config',
			configFor(profile),
		],
		{
			stdio: ['inherit', 'pipe', 'pipe'],
			env: {
				...process.env,
				AMBERITE_APP_DEV_PORT: String(profile.port),
				CARGO_TARGET_DIR: profile.targetDir,
				RUST_LOG: process.env.RUST_LOG ?? 'info',
			},
		},
	)

	children.add(child)
	child.stdout.on('data', (chunk) => prefixOutput(process.stdout, profile.name, chunk))
	child.stderr.on('data', (chunk) => prefixOutput(process.stderr, profile.name, chunk))
	child.on('exit', (code, signal) => {
		children.delete(child)
		const reason = signal ? `signal ${signal}` : `code ${code}`
		console.log(`[${profile.name}] exited with ${reason}`)
		if (!shuttingDown && children.size > 0) shutdown(1)
	})

	console.log(
		`[${profile.name}] starting ${profile.productName} on http://localhost:${profile.port} with identifier ${profile.identifier}`,
	)
}

function shutdown(exitCode = 0) {
	if (shuttingDown) return
	shuttingDown = true

	for (const child of children) {
		if (child.killed) continue
		if (process.platform === 'win32') {
			spawnSync('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], {
				windowsHide: true,
			})
		} else {
			child.kill()
		}
	}

	setTimeout(() => process.exit(exitCode), 500).unref()
}

async function main() {
	process.on('SIGINT', () => shutdown(0))
	process.on('SIGTERM', () => shutdown(0))
	process.on('SIGHUP', () => shutdown(0))

	console.log('Starting multiplayer Amberite dev with two isolated app identifiers.')
	console.log('Each app keeps separate login/session/app data while using the same backend configuration.')

	const reservedPorts = new Set()
	for (const profile of profiles) {
		profile.port = await findFreePort(profile.port, reservedPorts)
		startProfile(profile)
	}
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	main().catch((error) => {
		console.error(error.stack ?? error.message)
		shutdown(1)
	})
}
