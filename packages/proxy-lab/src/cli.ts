import { rm } from 'node:fs/promises'
import { resolve } from 'node:path'

import { runDev } from './dev.ts'
import { cleanAppLibEnv, prepareAppLibEnv } from './env.ts'
import { HOST, PORT } from './fixtures.ts'
import { startProxyLabServer } from './server.ts'
import { createStateStore } from './state.ts'

const dataDir = resolve(import.meta.dirname, '../../../modrinthclonedata/service')
const command = process.argv[2] ?? 'start'

switch (command) {
	case 'dev':
		await runDev(process.argv.slice(3))
		break
	case 'start':
		await start()
		break
	case 'reset':
		await reset()
		break
	case 'status':
		await status()
		break
	case 'env:prepare':
		console.log(`Proxy-lab App environment: ${await prepareAppLibEnv()}`)
		break
	case 'env:clean':
		console.log(`Proxy-lab App environment: ${await cleanAppLibEnv()}`)
		break
	default:
		throw new Error(`Unknown proxy-lab command "${command}"`)
}

async function start(): Promise<void> {
	const publicCatalog = process.argv.includes('--public-catalog')
	const server = await startProxyLabServer({
		host: HOST,
		port: PORT,
		dataDir,
		publicCatalog,
	})
	console.log(`Modrinth proxy lab listening on ${server.origin}`)
	console.log(`Process: ${process.pid}`)
	console.log(`Public Modrinth catalog reads: ${publicCatalog ? 'enabled' : 'disabled'}`)
	console.log(`State and redacted traces: ${dataDir}`)

	let closing = false
	const close = async () => {
		if (closing) return
		closing = true
		await server.close()
		process.exitCode = 0
	}
	process.once('SIGINT', () => void close())
	process.once('SIGTERM', () => void close())
	await new Promise<void>((resolve) => process.once('beforeExit', () => resolve()))
}

async function reset(): Promise<void> {
	const store = await createStateStore(dataDir)
	await store.reset()
	const sharedFiles = resolve(dataDir, 'shared-files')
	if (!sharedFiles.startsWith(`${dataDir}\\`) && !sharedFiles.startsWith(`${dataDir}/`)) {
		throw new Error(`Refusing to reset unexpected path ${sharedFiles}`)
	}
	await rm(sharedFiles, { recursive: true, force: true })
	console.log(`Reset proxy-lab state in ${dataDir}`)
}

async function status(): Promise<void> {
	const state = (await createStateStore(dataDir)).read()
	console.log(
		JSON.stringify(
			{
				data_dir: dataDir,
				hosting_server: state.archon.server_v0.server_id,
				power_state: state.archon.power_state,
				shared_instances: state.shared_instances.map((instance) => ({
					id: instance.id,
					versions: instance.versions.length,
					users: instance.users.map((user) => user.id),
				})),
			},
			null,
			2,
		),
	)
}
