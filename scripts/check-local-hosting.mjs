import assert from 'node:assert/strict'
import { readFile, writeFile } from 'node:fs/promises'
import { performance } from 'node:perf_hooks'
import {
	GenericModrinthClient,
	NodeAuthFeature,
	PanelVersionFeature,
	getBackupDownloadUrl,
} from '../packages/api-client/dist/index.js'

async function main() {
	const runtime = JSON.parse(
		await readFile(new URL('../.data/runtime.json', import.meta.url), 'utf8'),
	)
	const coreUrl = process.argv[2] || runtime.urls.core
	assert.equal(
		new URL(coreUrl).hostname,
		'127.0.0.1',
		'This check only runs against the local development Core',
	)
	let fsAuth = null
	const client = new GenericModrinthClient({
		archonBaseUrl: `${coreUrl}/hosting`,
		selfHostedHosting: true,
		features: [
			new NodeAuthFeature({ getAuth: () => fsAuth, refreshAuth: async () => {} }),
			new PanelVersionFeature(),
		],
	})
	const results = []
	async function check(name, fn) {
		const started = performance.now()
		await fn()
		results.push({ name, ms: Math.round(performance.now() - started) })
		console.log(`PASS ${name} (${results.at(-1).ms} ms)`)
	}
	async function waitFor(description, fn, timeoutMs = 60000) {
		const deadline = performance.now() + timeoutMs
		while (performance.now() < deadline) {
			if (await fn()) return
			await new Promise((resolve) => setTimeout(resolve, 500))
		}
		throw new Error(`Timed out waiting for ${description}`)
	}
	const statePath = new URL('../.data/hosting-proof.json', import.meta.url)
	let proof = await readFile(statePath, 'utf8')
		.then(JSON.parse)
		.catch(() => ({}))
	let id = proof.id
	await check('real server creation and v0/v1 details', async () => {
		if (!id) {
			id = (await client.archon.servers_v1.createLocal('Hosting dev check')).id
			await writeFile(statePath, JSON.stringify({ id }, null, 2))
		}
		const list = await client.archon.servers_v0.list({ limit: 100 })
		assert(list.servers.some((server) => server.server_id === id))
		const server = await client.archon.servers_v0.get(id)
		assert(server.node?.instance.startsWith(coreUrl))
		assert.equal(server.net.domain, null)
		const detail = await client.archon.servers_v1.get(id)
		assert.equal(detail.worlds[0].id, id)
		fsAuth = await client.archon.servers_v0.getFilesystemAuth(id)
	})
	await check('rename and properties persistence', async () => {
		await client.archon.servers_v0.updateName(id, 'Hosting dev check')
		assert.equal((await client.archon.servers_v0.get(id)).name, 'Hosting dev check')
		await client.archon.properties_v1.patchProperties(id, id, {
			known: { motd: 'Local hosting contract check' },
		})
		const properties = await client.archon.properties_v1.getProperties(id, id)
		assert.equal(properties.known.motd, 'Local hosting contract check')
	})
	await check('file create, write, rename, read and delete', async () => {
		const path = '/contract-check.txt'
		const previous = await client.kyros.files_v0.listDirectory('/', 1, 100)
		for (const fixture of ['contract-check.txt', 'contract-renamed.txt']) {
			if (previous.items.some((file) => file.name === fixture)) {
				await client.kyros.files_v0.deleteFileOrFolder(`/${fixture}`, false)
			}
		}
		await client.kyros.files_v0.createFileOrFolder(path, 'file')
		await client.kyros.files_v0.updateFile(path, 'real persisted file bytes')
		await client.kyros.files_v0.moveFileOrFolder(path, '/contract-renamed.txt')
		assert.equal(
			await (await client.kyros.files_v0.downloadFile('/contract-renamed.txt')).text(),
			'real persisted file bytes',
		)
		const listing = await client.kyros.files_v0.listDirectory('/', 1, 100)
		assert(listing.items.some((file) => file.name === 'contract-renamed.txt'))
		assert.equal(typeof listing.items[0].modified, 'number')
		await client.kyros.files_v0.deleteFileOrFolder('/contract-renamed.txt', false)
	})
	await check('multipart upload session, finalize and cancel', async () => {
		const session = await client.kyros.upload_sessions_v1.create('files', id)
		const form = new FormData()
		form.append('file', new Blob(['uploaded through a staged session']), 'contract-upload.txt')
		const upload = await fetch(
			`${coreUrl}/hosting/servers/${id}/v1/worlds/${id}/files/upload-session/${session.upload_id}/files`,
			{ method: 'POST', body: form },
		)
		assert.equal(upload.status, 200)
		const staged = await upload.json()
		assert.equal(staged.entry_count, 1)
		assert.equal(staged.uploaded_byte_count, Buffer.byteLength('uploaded through a staged session'))
		const final = await client.kyros.upload_sessions_v1.finalize('files', id, session.upload_id)
		assert.equal(final.status, 'finalized')
		assert.equal(
			await (await client.kyros.files_v0.downloadFile('/contract-upload.txt')).text(),
			'uploaded through a staged session',
		)
		const cancelled = await client.kyros.upload_sessions_v1.create('files', id)
		await client.kyros.upload_sessions_v1.cancel('files', id, cancelled.upload_id)
	})
	await check('path traversal and cross-world requests rejected', async () => {
		const traversal = await fetch(
			`${coreUrl}/hosting/servers/${id}/modrinth/v0/fs/create?path=${encodeURIComponent('/../outside.txt')}&type=file`,
			{ method: 'POST' },
		)
		assert.equal(traversal.status, 400)
		const wrongWorld = await fetch(
			`${coreUrl}/hosting/v1/servers/${id}/worlds/not-this-world/properties`,
		)
		assert.equal(wrongWorld.status, 404)
	})
	await check('backup create, download, restore and delete', async () => {
		const backup = await client.archon.backups_queue_v1.create(id, id, { name: 'Contract proof' })
		await waitFor('backup creation', async () => {
			const queue = await client.archon.backups_queue_v1.list(id, id)
			const item = queue.backups.find((item) => item.id === backup.id)
			if (item?.status === 'error') throw new Error(item.history[0]?.error || 'Backup failed')
			return item?.status === 'done'
		})
		const created = (await client.archon.backups_queue_v1.list(id, id)).backups.find(
			(item) => item.id === backup.id,
		)
		await client.archon.backups_queue_v1.ackCreate(id, id, created.history[0].operation_id)
		const server = await client.archon.servers_v0.get(id)
		const download = await fetch(
			getBackupDownloadUrl(server.node.instance, backup.id, server.node.token),
		)
		assert.equal(download.status, 200)
		assert((await download.arrayBuffer()).byteLength > 0)
		await client.kyros.files_v0.updateFile('/contract-upload.txt', 'changed after backup')
		await client.archon.backups_queue_v1.restore(id, id, backup.id, {
			name: 'Before contract restore',
		})
		await waitFor('backup restoration', async () => {
			const queue = await client.archon.backups_queue_v1.list(id, id)
			const operation = queue.backups
				.find((item) => item.id === backup.id)
				?.history.find((operation) => operation.operation_type === 'restore')
			if (operation?.state === 'failed') throw new Error(operation.error || 'Restore failed')
			return operation?.state === 'completed'
		})
		assert.equal(
			await (await client.kyros.files_v0.downloadFile('/contract-upload.txt')).text(),
			'uploaded through a staged session',
		)
		assert(
			(await client.archon.backups_queue_v1.list(id, id)).backups.some(
				(item) => item.name === 'Before contract restore',
			),
		)
		await client.archon.backups_queue_v1.delete(id, id, backup.id)
		assert(
			!(await client.archon.backups_queue_v1.list(id, id)).backups.some(
				(item) => item.id === backup.id,
			),
		)
	})
	await check('WebSocket authentication, power state and real statistics', async () => {
		const observed = new Set()
		let complete
		const ready = new Promise((resolve) => {
			complete = resolve
		})
		const off = ['state', 'stats'].map((event) =>
			client.archon.sockets.on(id, event, () => {
				observed.add(event)
				if (observed.size === 2) complete()
			}),
		)
		try {
			await client.archon.sockets.safeConnect(id)
			await Promise.race([
				ready,
				new Promise((_, reject) =>
					setTimeout(() => reject(new Error('Missing WebSocket state/statistics')), 15000),
				),
			])
		} finally {
			off.forEach((unsubscribe) => unsubscribe())
			client.archon.sockets.disconnectAll()
		}
	})
	await check('SSE panel synchronization', async () => {
		const response = await fetch(`${coreUrl}/hosting/v1/sync?scope=server:${id}&intent=all`)
		assert.equal(response.status, 200)
		const reader = response.body.getReader()
		const first = await reader.read()
		assert(new TextDecoder().decode(first.value).includes('server.network.patch'))
		await reader.cancel()
	})
	await check('real Minecraft installation and shared runtime binding', async () => {
		if (!(await client.archon.servers_v0.get(id)).mc_version) {
			await client.archon.content_v1.installContent(id, id, {
				content_variant: 'bare',
				loader: 'vanilla',
				version: '1.21.1',
				game_version: '1.21.1',
				soft_override: false,
				properties: { known: { level_seed: null }, custom: {} },
			})
		}
		await waitFor(
			'Minecraft runtime installation',
			async () => {
				const server = await client.archon.servers_v0.get(id)
				if (server.status === 'broken') throw new Error('Minecraft runtime installation failed')
				return server.status === 'available'
			},
			180000,
		)
		const native = await (await fetch(`${coreUrl}/instances/${id}`)).json()
		assert(native.installation_id, 'Instance must use a shared installation')
		await client.archon.servers_v1.endIntro(id)
		assert.equal((await client.archon.servers_v0.get(id)).flows.intro, false)
	})
	await check('local mrpack upload, properties and directory overrides', async () => {
		const pack = await readFile(new URL('../.data/hosting-proof.mrpack', import.meta.url))
		const form = new FormData()
		form.append('file', new Blob([pack]), 'hosting-proof.mrpack')
		form.append(
			'properties',
			JSON.stringify({ known: { level_seed: null, motd: 'Local mrpack proof' }, custom: {} }),
		)
		const response = await fetch(
			`${coreUrl}/hosting/servers/${id}/v1/worlds/${id}/content/upload-modpack-file?soft_override=true`,
			{ method: 'POST', body: form },
		)
		assert.equal(response.status, 204, await response.text())
		assert.equal(
			await (await client.kyros.files_v0.downloadFile('/config/hosting-proof.txt')).text(),
			'real local mrpack override',
		)
		assert.equal(
			(await client.archon.properties_v1.getProperties(id, id)).known.motd,
			'Local mrpack proof',
		)
	})
	await writeFile(
		statePath,
		JSON.stringify({ id, coreUrl, checkedAt: new Date().toISOString(), results }, null, 2),
	)
	console.log(`Checked ${results.length} groups against real Core. Sample server retained: ${id}`)
}

main().catch((error) => {
	console.error(`FAIL ${error.message}`)
	process.exitCode = 1
})
