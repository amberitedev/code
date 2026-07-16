#!/usr/bin/env node
import { spawn, spawnSync } from 'node:child_process'
import {
	existsSync,
	lstatSync,
	mkdirSync,
	readFileSync,
	readlinkSync,
	realpathSync,
	rmSync,
	writeFileSync,
} from 'node:fs'
import net from 'node:net'
import { dirname, join, resolve, sep } from 'node:path'

import {
	commandForPnpm,
	findWorktreeRoot,
	gitValue,
	isProcessRunning,
	killProcessTree,
	readEnvFile,
} from './dev-shared.mjs'

const action = process.argv[2] ?? 'local'
const positional = process.argv.slice(3).filter((value) => value !== '--')
const baseline = positional[0] ?? 'accounts'
const worktree = findWorktreeRoot()
const localRoot = join(worktree, '.convex')
const localConfigPath = join(localRoot, 'local', 'default', 'config.json')
const statePath = join(localRoot, 'amberite-runtime.json')
const baselinePath = join(localRoot, 'amberite-baseline.json')

try {
	if (action === 'local') await startLocal(baseline)
	else if (action === 'reset') resetLocal(baseline)
	else if (action === 'cloud') await selectCloud()
	else if (action === 'seed-cloud') seedCloudAccounts()
	else if (action === 'status') printStatus()
	else if (action === 'stop') await stopLocal()
	else throw new Error(`Unknown Convex development action "${action}".`)
} catch (error) {
	console.error(error instanceof Error ? error.message : String(error))
	process.exitCode = 1
}

async function startLocal(selectedBaseline) {
	if (!['accounts', 'group'].includes(selectedBaseline)) {
		throw new Error('Local Convex baseline must be "accounts" or "group".')
	}
	const current = readState()
	if (current && isProcessRunning(current.pid)) {
		throw new Error(`Local Convex is already running for this worktree (PID ${current.pid}).`)
	}

	removeLegacySharedDeployment()
	selectLocalDeployment()
	const savedBaseline = readBaseline()
	if (savedBaseline && positional[0] && savedBaseline !== selectedBaseline) {
		throw new Error(
			`This worktree currently uses the "${savedBaseline}" baseline. ` +
				`Run pnpm convex:dev:reset -- ${selectedBaseline} to replace its data.`,
		)
	}
	selectedBaseline = savedBaseline ?? selectedBaseline
	const urls = readConvexUrls()
	mkdirSync(localRoot, { recursive: true })

	const { command, args } = commandForPnpm(['exec', 'convex', 'dev', '--tail-logs', 'disable'])
	const child = spawn(command, args, {
		cwd: worktree,
		stdio: 'inherit',
		detached: process.platform !== 'win32',
		env: { ...process.env, COREPACK_ENABLE_DOWNLOAD_PROMPT: '0' },
	})
	if (!child.pid) throw new Error('Convex local development did not start.')
	writeState({
		pid: child.pid,
		worktree,
		branch: gitValue(worktree, ['branch', '--show-current']) || 'detached',
		baseline: selectedBaseline,
		startedAt: new Date().toISOString(),
		...urls,
	})

	try {
		await configureLocal(urls.convexUrl, selectedBaseline, !savedBaseline)
	} catch (error) {
		killProcessTree(child.pid)
		rmSync(statePath, { force: true })
		throw error
	}
	console.log(`Local Convex is ready with the "${selectedBaseline}" baseline.`)

	const forwardSignal = (signal) => {
		if (isProcessRunning(child.pid)) child.kill(signal)
	}
	process.once('SIGINT', forwardSignal)
	process.once('SIGTERM', forwardSignal)
	const exitCode = await new Promise((resolveExit) => {
		child.once('exit', (code, signal) => resolveExit(signal ? 1 : (code ?? 1)))
	})
	if (readState()?.pid === child.pid) rmSync(statePath, { force: true })
	process.exitCode = exitCode
}

async function configureLocal(convexUrl, selectedBaseline, initialize) {
	await waitForUrl(convexUrl, 60_000)
	runConvex(['env', 'set', 'AMBERITE_DEV_MODE', 'true', '--deployment', 'local'])
	if (!initialize) return
	await applyBaseline(selectedBaseline)
	writeBaseline(selectedBaseline)
}

async function applyBaseline(selectedBaseline) {
	const args = JSON.stringify({ baseline: selectedBaseline })
	const deadline = Date.now() + 60_000
	let lastError = ''
	while (Date.now() < deadline) {
		const result = runConvex(['run', 'dev:applyBaseline', args, '--deployment', 'local'], 'pipe', false)
		if (result.status === 0) return
		lastError = result.stderr?.trim() || result.stdout?.trim() || 'unknown Convex error'
		await delay(1_000)
	}
	throw new Error(`Could not apply the local Convex baseline: ${lastError}`)
}

function resetLocal(selectedBaseline) {
	if (!['accounts', 'group'].includes(selectedBaseline)) {
		throw new Error('Local Convex baseline must be "accounts" or "group".')
	}
	const state = readState()
	if (!state || !isProcessRunning(state.pid)) {
		throw new Error('Start this worktree\'s local Convex deployment before resetting it.')
	}
	runConvex([
		'run',
		'dev:applyBaseline',
		JSON.stringify({ baseline: selectedBaseline }),
		'--deployment',
		'local',
	])
	writeBaseline(selectedBaseline)
	console.log(`Local Convex was reset to the "${selectedBaseline}" baseline.`)
}

function selectLocalDeployment() {
	const selected = runConvex(['deployment', 'select', 'local'], 'pipe', false)
	if (selected.status === 0) return
	runConvex(['deployment', 'create', 'local', '--select'])
}

async function selectCloud() {
	await stopLocal(false)
	runConvex(['deployment', 'select', 'dev'])
	console.log('This worktree now uses the cloud development deployment.')
}

function seedCloudAccounts() {
	runConvex(['env', 'set', 'AMBERITE_DEV_MODE', 'true', '--deployment', 'dev'])
	runConvex(['run', 'dev:ensureAccounts', '{}', '--deployment', 'dev'])
	console.log('The cloud development accounts are ready: owner, friend, other.')
}

async function stopLocal(print = true) {
	const state = readState()
	if (!state || !isProcessRunning(state.pid)) {
		rmSync(statePath, { force: true })
		if (print) console.log('Local Convex is not running for this worktree.')
		return
	}
	killProcessTree(state.pid)
	const deadline = Date.now() + 5_000
	while (isProcessRunning(state.pid) && Date.now() < deadline) await delay(50)
	if (isProcessRunning(state.pid)) throw new Error('Local Convex did not stop within five seconds.')
	rmSync(statePath, { force: true })
	if (print) console.log('Stopped this worktree\'s local Convex deployment.')
}

function printStatus() {
	const env = readEnvFile(join(worktree, '.env.local'))
	const state = readState()
	console.log(
		JSON.stringify(
			{
				selection: env.CONVEX_DEPLOYMENT?.startsWith('local:') ? 'local' : 'cloud',
				localRunning: Boolean(state && isProcessRunning(state.pid)),
				...(state && isProcessRunning(state.pid) ? state : {}),
			},
			null,
			2,
		),
	)
}

function removeLegacySharedDeployment() {
	if (!existsSync(localRoot) || !lstatSync(localRoot).isSymbolicLink()) return
	const target = resolve(dirname(localRoot), readlinkSync(localRoot))
	const realTarget = existsSync(target) ? realpathSync(target) : target
	if (!realTarget.includes(`${sep}amberite-dev${sep}`)) {
		throw new Error(`${localRoot} is a user-managed symlink and cannot be replaced automatically.`)
	}
	rmSync(localRoot, { force: true })
}

function readConvexUrls() {
	if (!existsSync(localConfigPath)) {
		throw new Error(`Local Convex configuration is missing from ${localConfigPath}.`)
	}
	const config = JSON.parse(readFileSync(localConfigPath, 'utf8'))
	const cloudPort = Number(config?.ports?.cloud)
	const sitePort = Number(config?.ports?.site)
	if (!validPort(cloudPort) || !validPort(sitePort)) {
		throw new Error(`Local Convex ports are invalid in ${localConfigPath}.`)
	}
	return {
		convexUrl: `http://127.0.0.1:${cloudPort}`,
		convexSiteUrl: `http://127.0.0.1:${sitePort}`,
	}
}

function runConvex(args, stdio = 'inherit', fail = true) {
	const command = commandForPnpm(['exec', 'convex', ...args])
	const result = spawnSync(command.command, command.args, {
		cwd: worktree,
		stdio,
		encoding: stdio === 'pipe' ? 'utf8' : undefined,
		windowsHide: true,
		env: { ...process.env, COREPACK_ENABLE_DOWNLOAD_PROMPT: '0' },
	})
	if (fail && result.status !== 0) throw new Error(`Convex command failed: convex ${args.join(' ')}`)
	return result
}

function waitForUrl(rawUrl, timeoutMs) {
	const url = new URL(rawUrl)
	const port = Number(url.port || (url.protocol === 'https:' ? 443 : 80))
	const startedAt = Date.now()
	return new Promise((resolveWait, reject) => {
		const attempt = () => {
			const socket = net.createConnection({ host: url.hostname, port })
			socket.setTimeout(500)
			socket.once('connect', () => {
				socket.destroy()
				resolveWait()
			})
			const retry = () => {
				socket.destroy()
				if (Date.now() - startedAt >= timeoutMs) reject(new Error('Local Convex did not start.'))
				else setTimeout(attempt, 250)
			}
			socket.once('error', retry)
			socket.once('timeout', retry)
		}
		attempt()
	})
}

function readState() {
	if (!existsSync(statePath)) return null
	try {
		return JSON.parse(readFileSync(statePath, 'utf8'))
	} catch {
		return null
	}
}

function writeState(state) {
	writeFileSync(statePath, `${JSON.stringify(state, null, 2)}\n`, { mode: 0o600 })
}

function readBaseline() {
	if (!existsSync(baselinePath)) return null
	try {
		const value = JSON.parse(readFileSync(baselinePath, 'utf8'))
		return value.baseline === 'accounts' || value.baseline === 'group' ? value.baseline : null
	} catch {
		return null
	}
}

function writeBaseline(value) {
	writeFileSync(baselinePath, `${JSON.stringify({ baseline: value }, null, 2)}\n`, {
		mode: 0o600,
	})
}

function delay(ms) {
	return new Promise((resolveDelay) => setTimeout(resolveDelay, ms))
}

function validPort(value) {
	return Number.isInteger(value) && value >= 1 && value <= 65_535
}
