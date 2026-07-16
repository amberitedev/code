#!/usr/bin/env node
import { existsSync } from 'node:fs'
import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { pathToFileURL } from 'node:url'

export interface CoreSetupContext {
	baseUrl: string
	request<T>(method: string, target: string, body?: unknown, userId?: string): Promise<T>
}

export interface CoreSetup {
	description: string
	run(context: CoreSetupContext): Promise<void>
}

const coreDir = process.cwd()
const setupsDir = path.join(coreDir, 'scripts', 'setups')
const command = process.argv.slice(2).find((argument) => argument !== '--') ?? 'list'

async function main() {
	const setups = await availableSetups()
	if (command === 'list') {
		if (setups.length === 0) console.log('No Core setups are defined.')
		for (const name of setups) {
			const setup = await loadSetup(name)
			console.log(`${name}\t${setup.description}`)
		}
		return
	}
	if (!setups.includes(command)) {
		throw new Error(`Unknown Core setup "${command}". Run pnpm core:setup -- list.`)
	}

	const baseUrl = await resolveCoreUrl()
	await waitForCore(baseUrl)
	const setup = await loadSetup(command)
	await setup.run({ baseUrl, request: createRequest(baseUrl) })
	console.log(`Core setup "${command}" is ready at ${baseUrl}.`)
}

async function availableSetups() {
	return (await readdir(setupsDir))
		.filter((name) => /^[a-z0-9-]+\.ts$/.test(name))
		.map((name) => name.slice(0, -3))
		.sort()
}

async function loadSetup(name: string): Promise<CoreSetup> {
	const module = await import(pathToFileURL(path.join(setupsDir, `${name}.ts`)).href)
	if (typeof module.description !== 'string' || typeof module.run !== 'function') {
		throw new Error(`Core setup "${name}" must export description and run.`)
	}
	return { description: module.description, run: module.run }
}

async function resolveCoreUrl() {
	const runtimePath = path.join(coreDir, '.copal', 'dev-runtime.json')
	if (existsSync(runtimePath)) {
		const runtime = JSON.parse(await readFile(runtimePath, 'utf8'))
		if (typeof runtime.url === 'string' && runtime.url) return runtime.url.replace(/\/$/, '')
	}

	const env = await readEnv(path.join(coreDir, '.env.local'))
	const host = env.AMBERITE_BIND_HOST === '0.0.0.0' ? '127.0.0.1' : env.AMBERITE_BIND_HOST
	if (!host || !env.PORT) throw new Error('Core URL is unavailable. Start this worktree\'s Core first.')
	return `http://${host}:${env.PORT}`
}

async function readEnv(file: string) {
	const values: Record<string, string> = {}
	for (const raw of (await readFile(file, 'utf8')).split(/\r?\n/)) {
		const match = raw.trim().match(/^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$/)
		if (!match) continue
		values[match[1]] = match[2].trim().replace(/^['"]|['"]$/g, '')
	}
	return values
}

function createRequest(baseUrl: string) {
	return async function request<T>(
		method: string,
		target: string,
		body?: unknown,
		userId = 'dev-owner',
	): Promise<T> {
		const response = await fetch(`${baseUrl}${target}`, {
			method,
			headers: {
				authorization: `Bearer dev:${userId}`,
				...(body === undefined ? {} : { 'content-type': 'application/json' }),
			},
			body: body === undefined ? undefined : JSON.stringify(body),
		})
		const text = await response.text()
		if (!response.ok) throw new Error(`${method} ${target} -> ${response.status}: ${text}`)
		return (text ? JSON.parse(text) : undefined) as T
	}
}

async function waitForCore(baseUrl: string) {
	const deadline = Date.now() + 30_000
	while (Date.now() < deadline) {
		try {
			const response = await fetch(`${baseUrl}/health`)
			if (response.ok) return
		} catch {}
		await new Promise((resolve) => setTimeout(resolve, 500))
	}
	throw new Error(`Core did not become healthy at ${baseUrl}.`)
}

main().catch((error) => {
	console.error(error instanceof Error ? error.message : error)
	process.exit(1)
})
