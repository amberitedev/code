#!/usr/bin/env node
import { spawn } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

import {
	commandForPnpm,
	findWorktreeRoot,
	isProcessRunning,
	readEnvFile,
} from './dev-shared.mjs'

const worktree = findWorktreeRoot()
const rootEnv = readEnvFile(join(worktree, '.env.local'))
const noAuth = process.argv.includes('--no-auth')

try {
	const convex = selectedConvexRuntime()
	console.log(`Starting Core with Convex ${convex.convexUrl} (${convex.convexSiteUrl}).`)
	const command = commandForPnpm([
		'exec',
		'cargo',
		'run',
		...(noAuth ? ['--', '--no-auth'] : []),
	])
	const child = spawn(command.command, command.args, {
		cwd: join(worktree, 'apps', 'core'),
		stdio: 'inherit',
		env: {
			...process.env,
			CONVEX_URL: convex.convexUrl,
			CONVEX_SITE_URL: convex.convexSiteUrl,
			COREPACK_ENABLE_DOWNLOAD_PROMPT: '0',
		},
	})
	child.once('exit', (code, signal) => {
		process.exitCode = signal ? 1 : (code ?? 1)
	})
} catch (error) {
	console.error(error instanceof Error ? error.message : String(error))
	process.exitCode = 1
}

function selectedConvexRuntime() {
	if (!rootEnv.CONVEX_DEPLOYMENT?.startsWith('local:')) {
		if (!rootEnv.CONVEX_URL || !rootEnv.CONVEX_SITE_URL) {
			throw new Error(`Convex URLs are missing from ${join(worktree, '.env.local')}.`)
		}
		return {
			convexUrl: rootEnv.CONVEX_URL,
			convexSiteUrl: rootEnv.CONVEX_SITE_URL,
		}
	}

	const runtimePath = join(worktree, '.convex', 'amberite-runtime.json')
	if (!existsSync(runtimePath)) {
		throw new Error('Local Convex is selected but not running. Start pnpm convex:dev:local first.')
	}
	const runtime = JSON.parse(readFileSync(runtimePath, 'utf8'))
	if (!isProcessRunning(runtime.pid) || !runtime.convexUrl || !runtime.convexSiteUrl) {
		throw new Error('Local Convex is selected but not running. Start pnpm convex:dev:local first.')
	}
	return runtime
}
