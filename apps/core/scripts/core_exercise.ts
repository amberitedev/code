#!/usr/bin/env node
import { existsSync } from 'node:fs'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'

interface InstanceDetail { id: string; name: string; status: string; data_dir: string; game_version: string; loader: string; port: number }

const args = parseArgs(process.argv.slice(2))
const command = String(args._?.[0] ?? 'help')
const baseUrl = String(args.url ?? 'http://localhost:16662').replace(/\/$/, '')

async function main() {
	if (command === 'help') {
		console.log('usage: node scripts/core_exercise.ts vanilla|add-mod|start|stop|call [--url ...]')
		return
	}
	if (command === 'wait-core') return waitForCore()
	if (command === 'vanilla') return vanilla()
	if (command === 'add-mod') return addMod()
	if (command === 'start') return startInstance(String(requireArg('instance')))
	if (command === 'stop') return stopInstance(String(requireArg('instance')))
	if (command === 'call') return callRaw()
	throw new Error(`unknown command: ${command}`)
}

async function vanilla() {
	await waitForCore()
	const version = String(args.version ?? '1.21.1')
	const port = Number(args.port ?? 25565)
	const loader = String(args.loader ?? 'vanilla')
	const instance = await request<InstanceDetail>('POST', '/instances', {
		name: String(args.name ?? `amberite-${loader}-${Date.now()}`),
		game_version: version,
		loader,
		loader_version: args.loaderVersion ? String(args.loaderVersion) : null,
		port,
		memory: {
			min_mb: Number(args.minMb ?? 512),
			max_mb: Number(args.maxMb ?? 2048),
		},
	})
	await request('PATCH', `/instances/${instance.id}/properties`, {
		motd: 'Copal live test',
		'online-mode': 'false',
	})
	await waitForInstall(instance.id)
	if (args.noStart) return printInstance(await request<InstanceDetail>('GET', `/instances/${instance.id}`))
	await request('POST', `/instances/${instance.id}/start`)
	const running = await waitForStatus(instance.id, 'running', Number(args.startTimeoutMs ?? 180000))
	printInstance(running)
}

function printInstance(instance: InstanceDetail) {
	console.log(JSON.stringify({
		id: instance.id,
		address: `localhost:${instance.port}`,
		data_dir: instance.data_dir,
		game_version: instance.game_version,
		loader: instance.loader,
		status: instance.status,
	}, null, 2))
}

async function addMod() {
	await waitForCore()
	const id = String(requireArg('instance'))
	const versionId = String(args.versionId ?? await latestModVersion(
		String(args.project ?? 'lithium'),
		String(args.gameVersion ?? '1.21.1'),
		String(args.loader ?? 'fabric'),
	))
	const mod = await request<Record<string, unknown>>('POST', `/instances/${id}/mods`, { version_id: versionId })
	console.log(JSON.stringify(mod, null, 2))
}

async function callRaw() {
	const method = String(args.method ?? 'GET').toUpperCase()
	const target = String(args.path ?? requireArg('path'))
	const body = args.body ? JSON.parse(String(args.body)) : undefined
	const result = await request(method, target, body)
	console.log(JSON.stringify(result, null, 2))
}

async function waitForCore(timeoutMs = Number(args.timeoutMs ?? 120000)) {
	const deadline = Date.now() + timeoutMs
	let last = ''
	while (Date.now() < deadline) {
		try {
			await request('GET', '/health', undefined, false)
			return
		} catch (error) {
			last = error instanceof Error ? error.message : String(error)
			await sleep(1000)
		}
	}
	throw new Error(`Core did not become healthy: ${last}`)
}

async function waitForInstall(id: string, timeoutMs = Number(args.installTimeoutMs ?? 300000)) {
	const deadline = Date.now() + timeoutMs
	while (Date.now() < deadline) {
		const instance = await request<InstanceDetail>('GET', `/instances/${id}`)
		const launch = path.join(instance.data_dir, 'launch.json')
		if (existsSync(launch)) {
			const config = JSON.parse(await readFile(launch, 'utf8'))
			const jar = config.style?.jar ?? 'server.jar'
			if (existsSync(path.join(instance.data_dir, jar))) return
		}
		await sleep(2000)
	}
	const instance = await request<InstanceDetail>('GET', `/instances/${id}`)
	throw new Error(`server.jar or launch.json was not created in ${instance.data_dir}`)
}

async function startInstance(id: string) {
	await request('POST', `/instances/${id}/start`)
	const running = await waitForStatus(id, 'running', Number(args.startTimeoutMs ?? 240000))
	printInstance(running)
}

async function waitForStatus(id: string, status: string, timeoutMs: number) {
	const deadline = Date.now() + timeoutMs
	let last = await request<InstanceDetail>('GET', `/instances/${id}`)
	while (Date.now() < deadline) {
		last = await request<InstanceDetail>('GET', `/instances/${id}`)
		if (last.status === status) return last
		if (last.status === 'offline' && status !== 'offline') {
			throw new Error(`instance returned to offline before ${status}`)
		}
		await sleep(1000)
	}
	throw new Error(`timed out waiting for ${status}; last status was ${last.status}`)
}

async function stopInstance(id: string) {
	try {
		await request('POST', `/instances/${id}/stop`)
	} catch (error) {
		if (!String(error).includes('409')) throw error
	}
	const stopped = await waitForStatus(id, 'offline', Number(args.stopTimeoutMs ?? 45000))
	console.log(JSON.stringify(stopped, null, 2))
}

async function latestModVersion(project: string, gameVersion: string, loader: string) {
	const url = new URL(`https://api.modrinth.com/v2/project/${project}/version`)
	url.searchParams.set('game_versions', JSON.stringify([gameVersion]))
	url.searchParams.set('loaders', JSON.stringify([loader]))
	const versions = await fetchJson<Array<{ id: string }>>(url.toString())
	if (!versions[0]?.id) throw new Error(`no ${project} version for ${gameVersion}/${loader}`)
	return versions[0].id
}

async function request<T>(method: string, target: string, body?: unknown, parseJson = true): Promise<T> {
	const response = await fetch(`${baseUrl}${target}`, {
		method,
		headers: body ? { 'content-type': 'application/json' } : undefined,
		body: body ? JSON.stringify(body) : undefined,
	})
	const text = await response.text()
	if (!response.ok) throw new Error(`${method} ${target} -> ${response.status}: ${text}`)
	return (parseJson && text ? JSON.parse(text) : undefined) as T
}

async function fetchJson<T>(url: string): Promise<T> {
	const response = await fetch(url)
	const text = await response.text()
	if (!response.ok) throw new Error(`${url} -> ${response.status}: ${text}`)
	return JSON.parse(text) as T
}

function parseArgs(argv: string[]) {
	const parsed: Record<string, string | boolean | string[]> = { _: [] }
	for (let i = 0; i < argv.length; i++) {
		const value = argv[i]
		if (!value.startsWith('--')) parsed._ = [...parsed._ as string[], value]
		else parsed[value.slice(2)] = argv[i + 1]?.startsWith('--') || !argv[i + 1] ? true : argv[++i]
	}
	return parsed
}

function requireArg(name: string) {
	if (args[name] === undefined) throw new Error(`missing --${name}`)
	return args[name]
}

function sleep(ms: number) {
	return new Promise((resolve) => setTimeout(resolve, ms))
}

main().catch((error) => {
	console.error(error instanceof Error ? error.message : error)
	process.exit(1)
})
