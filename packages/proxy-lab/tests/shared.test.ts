import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import type { SharedInstances } from '../../api-client/src/modules/shared-instances/types.ts'

import { PERSONAS, SHARED_INSTANCE_ID } from '../src/fixtures.ts'
import { startProxyLabServer } from '../src/server.ts'

test('shared-instance owner publishes bytes and recipient reads the ready version', async () => {
	const dataDir = await mkdtemp(join(tmpdir(), 'modrinth-proxy-lab-'))
	const server = await startProxyLabServer({ dataDir, port: 0 })
	const ownerAuth = { Authorization: `Bearer ${PERSONAS.owner.token}` }
	const recipientAuth = { Authorization: `Bearer ${PERSONAS.recipient.token}` }

	try {
		const seeded = await fetchJson<SharedInstances.Instances.v1.InstanceVersion>(
			`${server.origin}/v1/instances/${SHARED_INSTANCE_ID}/versions`,
			{ headers: recipientAuth },
		)
		assert.equal(seeded.ready, true)
		assert.equal(seeded.external_files[0]?.file_size, 22)
		const seededDownload = await fetch(seeded.external_files[0]!.url, {
			headers: recipientAuth,
		})
		assert.equal(seededDownload.status, 200)
		assert.equal((await seededDownload.arrayBuffer()).byteLength, 22)

		const created = await fetchJson<{ id: string }>(
			`${server.origin}/v1/instances`,
			jsonRequest(ownerAuth, { name: 'Published in test' }),
		)
		const pendingVersion = await fetchJson<SharedInstances.Instances.v1.InstanceVersion>(
			`${server.origin}/v1/instances/${created.id}/versions`,
			jsonRequest(ownerAuth, {
				modrinth_ids: [],
				external_files: [{ file_name: 'options.txt', file_type: 'config' }],
				modpack_id: null,
				game_version: '1.21.1',
				loader: 'fabric',
				loader_version: '0.16.10',
			}),
		)
		assert.equal(pendingVersion.ready, false)
		const upload = pendingVersion.external_files[0]
		assert.ok(upload)

		const bytes = Buffer.from('proxy-lab-local-file')
		const uploadResponse = await fetch(upload.url, {
			method: 'PUT',
			headers: {
				...ownerAuth,
				'content-type': 'application/octet-stream',
				'x-file-sha512': createHash('sha512').update(bytes).digest('hex'),
			},
			body: bytes,
		})
		assert.equal(uploadResponse.status, 204)

		const readyVersion = await fetchJson<SharedInstances.Instances.v1.InstanceVersion>(
			`${server.origin}/v1/instances/${created.id}/versions`,
			{ headers: ownerAuth },
		)
		assert.equal(readyVersion.ready, true)
		assert.equal(readyVersion.external_files[0]?.file_size, bytes.length)
		assert.deepEqual(
			Buffer.from(
				await (
					await fetch(readyVersion.external_files[0]!.url, {
						headers: ownerAuth,
					})
				).arrayBuffer(),
			),
			bytes,
		)

		const invite = await fetchJson<{ id: string }>(
			`${server.origin}/v1/instances/${created.id}/invites`,
			jsonRequest(ownerAuth, { max_age: 600, max_uses: 1 }),
		)
		const inviteInfo = await fetchJson<SharedInstances.Invites.v1.Invite>(
			`${server.origin}/v1/invites/${invite.id}`,
			{},
		)
		assert.equal(inviteInfo.instance_id, created.id)

		const accept = await fetch(`${server.origin}/v1/instances/${created.id}/invites/${invite.id}`, {
			method: 'POST',
			headers: recipientAuth,
		})
		assert.equal(accept.status, 204)
		const recipientVersion = await fetchJson<SharedInstances.Instances.v1.InstanceVersion>(
			`${server.origin}/v1/instances/${created.id}/versions`,
			{ headers: recipientAuth },
		)
		assert.equal(recipientVersion.version, readyVersion.version)
	} finally {
		await server.close()
		await rm(dataDir, { recursive: true, force: true })
	}
})

function jsonRequest(
	headers: Record<string, string>,
	body: unknown,
): RequestInit & {
	headers: Record<string, string>
} {
	return {
		method: 'POST',
		headers: { ...headers, 'content-type': 'application/json' },
		body: JSON.stringify(body),
	}
}

async function fetchJson<T>(url: string, init: RequestInit): Promise<T> {
	const response = await fetch(url, init)
	if (!response.ok) assert.fail(await response.text())
	return (await response.json()) as T
}
