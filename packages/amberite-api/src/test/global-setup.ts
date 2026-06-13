/**
 * Global vitest setup: ensures an Copal process is reachable at
 * http://localhost:16662 before tests run.
 *
 * - If Core is already running, setup is a no-op (does not kill it in teardown).
 * - If Core is not running, spawns `cargo run` in apps/core/ and waits up to
 *   two minutes for /health to respond. Kills the spawned process in teardown.
 *
 * AMBERITE_DEV defaults to true in debug builds, so no JWT or pairing is needed.
 */

import { dirname, join } from 'node:path'
import { spawn, type ChildProcess } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const HEALTH_URL = 'http://localhost:16662/health'
const POLL_INTERVAL_MS = 500
const STARTUP_TIMEOUT_MS = 120_000

let spawnedProcess: ChildProcess | null = null

async function isHealthy(): Promise<boolean> {
	try {
		const res = await fetch(HEALTH_URL)
		return res.ok
	} catch {
		return false
	}
}

async function waitForHealth(): Promise<void> {
	const deadline = Date.now() + STARTUP_TIMEOUT_MS
	while (Date.now() < deadline) {
		if (await isHealthy()) return
		await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS))
	}
	throw new Error(
		`Core did not become healthy within ${STARTUP_TIMEOUT_MS}ms. ` +
			'Make sure apps/core/ compiles, or start Core manually before running tests.',
	)
}

export async function setup(): Promise<void> {
	if (await isHealthy()) {
		console.log('[global-setup] Core already running — skipping spawn')
		return
	}

	const __dirname = dirname(fileURLToPath(import.meta.url))
	const coreDir = join(__dirname, '../../../../apps/core')

	console.log('[global-setup] Core not detected — spawning via cargo run...')
	spawnedProcess = spawn('cargo', ['run'], {
		cwd: coreDir,
		stdio: 'pipe',
		shell: process.platform === 'win32',
	})

	spawnedProcess.on('error', (err) => {
		console.error('[global-setup] Failed to spawn Core process:', err.message)
	})

	await waitForHealth()
	console.log('[global-setup] Core is healthy')
}

export async function teardown(): Promise<void> {
	if (spawnedProcess) {
		console.log('[global-setup] Killing spawned Core process')
		spawnedProcess.kill()
		spawnedProcess = null
	}
}
