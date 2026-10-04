import assert from 'node:assert/strict'
import { createHash, randomBytes } from 'node:crypto'
import { readFile, writeFile, mkdir, unlink } from 'node:fs/promises'
import http from 'node:http'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// Run against this worktree's already-running backend and two Rust storage nodes.
// Only randomly generated proof objects are removed to exercise replica fallback.
const root = fileURLToPath(new URL('../../../', import.meta.url))
const data = path.join(root, '.data')
const runtime = JSON.parse(await readFile(path.join(data, 'runtime.json'), 'utf8'))
const backend = runtime.urls.backend
const variables = await readFile(path.join(data, 'backend', '.dev.vars'), 'utf8')
const localSecret = variables.match(/^LOCAL_DEV_SECRET=["']?([^\r\n"']+)/m)?.[1]
assert.ok(localSecret, 'The dev-runner must create its local secret first')
const stamp = `${Date.now().toString(36)}${randomBytes(3).toString('hex')}`
const snapshotDirectory = path.join(data, 'sharing-proof', stamp)
await mkdir(snapshotDirectory, { recursive: true })
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex')
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

async function request(endpoint, session, method = 'GET', body, headers = {}) {
	const response = await fetch(`${backend}${endpoint}`, {
		method,
		headers: {
			...(session ? { authorization: `Bearer ${session.session}` } : {}),
			...headers,
			...(body ? { 'content-type': 'application/json' } : {}),
		},
		...(body ? { body: JSON.stringify(body) } : {}),
	})
	if (!response.ok)
		throw new Error(`${method} ${endpoint} failed: ${response.status} ${await response.text()}`)
	return response.status === 204 ? null : response.json()
}

async function login(name) {
	return request(
		'/_dev/session',
		null,
		'POST',
		{ username: name },
		{ 'x-local-dev-secret': localSecret },
	)
}

async function download(version, expected) {
	const response = await fetch(version.external_files[0].url)
	assert.equal(response.status, 200, `Download failed: ${response.status}`)
	assert.equal(hash(Buffer.from(await response.arrayBuffer())), hash(expected))
}

async function waitForCopies(digest) {
	for (let attempt = 0; attempt < 50; attempt++) {
		const present = await Promise.all(
			['storage-a', 'storage-b'].map(async (id) => {
				try {
					return hash(await readFile(path.join(data, id, 'objects', digest))) === digest
				} catch {
					return false
				}
			}),
		)
		if (present.every(Boolean)) return
		await pause(1000)
	}
	throw new Error('Second storage node did not pull a verified copy within 50 seconds')
}

async function removeProofCopy(nodeId, digest) {
	assert.match(digest, /^[a-f0-9]{64}$/)
	assert.ok(['storage-a', 'storage-b'].includes(nodeId))
	const directory = path.resolve(data, nodeId, 'objects')
	const target = path.resolve(directory, digest)
	assert.equal(path.dirname(target), directory)
	await unlink(target).catch((error) => {
		if (error.code !== 'ENOENT') throw error
	})
}

async function corruptProofCopy(nodeId, digest, bytes) {
	assert.match(digest, /^[a-f0-9]{64}$/)
	assert.ok(['storage-a', 'storage-b'].includes(nodeId))
	const directory = path.resolve(data, nodeId, 'objects')
	const target = path.resolve(directory, digest)
	assert.equal(path.dirname(target), directory)
	const corrupt = Buffer.from(bytes)
	corrupt[0] ^= 0xff
	await writeFile(target, corrupt)
}

async function interruptedPut(url, bytes) {
	await new Promise((resolve) => {
		const operation = http.request(url, {
			method: 'PUT',
			headers: { 'Content-Length': bytes.length },
		})
		operation.on('error', () => resolve())
		operation.on('response', (response) => {
			response.resume()
			resolve()
		})
		operation.write(bytes.subarray(0, 17))
		setTimeout(() => operation.destroy(), 100)
	})
}

for (const endpoint of [backend, runtime.urls.storageA, runtime.urls.storageB]) {
	let healthy = false
	for (let attempt = 0; attempt < 60; attempt++) {
		try {
			healthy =
				(await fetch(`${endpoint}/health`, { signal: AbortSignal.timeout(1000) })).status === 200
		} catch {
			/* A runner-owned child may still be starting. */
		}
		if (healthy) break
		await pause(1000)
	}
	assert.ok(healthy, `${endpoint} must already be running`)
}
let owner = await login(`proof_owner_${stamp}`)
const started = performance.now()
const timings = {}
const recipient = await login(`proof_friend_${stamp}`)
const stranger = await login(`proof_other_${stamp}`)
await request(`/v3/friend/${recipient.user_id}`, owner, 'POST')
await request(`/v3/friend/${owner.user_id}`, recipient, 'POST')
const shared = await request('/v1/instances', owner, 'POST', { name: `Storage proof ${stamp}` })
await request(`/v1/instances/${shared.id}/users`, owner, 'POST', { user_ids: [recipient.user_id] })
const bytes = Buffer.concat([
	Buffer.from(`Resource pack proof ${stamp}\n`),
	randomBytes(512 * 1024),
])
const digest = hash(bytes)
await writeFile(path.join(snapshotDirectory, digest), bytes, { flag: 'wx' })
const body = {
	game_version: '1.21.1',
	loader: 'vanilla',
	loader_version: '',
	modrinth_ids: [],
	external_files: [{ file_name: 'textures.zip', file_type: 'resourcepack' }],
}
const version = await request(`/v1/instances/${shared.id}/versions`, owner, 'POST', body, {
	'Idempotency-Key': stamp,
})
const upload = new URL(version.external_files[0].url).pathname
let placement
for (let attempt = 0; attempt < 20; attempt++) {
	placement = await request(`${upload}/prepare`, owner, 'POST', {
		sha256: digest,
		size: bytes.length,
	})
	if (placement.status === 'upload') break
	await pause(1000)
}
assert.equal(placement.status, 'upload')
assert.equal(
	(await fetch(placement.upload_url, { method: 'PUT', body: Buffer.alloc(bytes.length) })).status,
	400,
	'Wrong digest must not commit',
)
await interruptedPut(placement.upload_url, bytes)
assert.equal((await request(`${upload}/status`, owner)).status, 'pending')
const uploadStarted = performance.now()
assert.equal((await fetch(placement.upload_url, { method: 'PUT', body: bytes })).status, 204)
timings.verifiedUploadMs = Math.round(performance.now() - uploadStarted)
const uploadedAt = performance.now()
assert.equal((await request(`${upload}/status`, owner)).status, 'available')
const published = await request(`/v1/instances/${shared.id}/versions`, owner)
assert.equal(published.ready, true)
assert.equal(published.external_files[0].sha256, digest)
await request(`/v1/instances/${shared.id}/invites/pending`, recipient, 'POST')
const recipientVersion = await request(`/v1/instances/${shared.id}/versions`, recipient)
assert.equal(
	(
		await fetch(`${backend}/v1/instances/${shared.id}/versions`, {
			headers: { authorization: `Bearer ${stranger.session}` },
		})
	).status,
	401,
)
await download(recipientVersion, bytes)

const partial = await fetch(recipientVersion.external_files[0].url, {
	headers: { Range: 'bytes=17-1023' },
})
assert.equal(partial.status, 206)
assert.deepEqual(Buffer.from(await partial.arrayBuffer()), bytes.subarray(17, 1024))
const canceled = await fetch(recipientVersion.external_files[0].url)
const reader = canceled.body.getReader()
await reader.read()
await reader.cancel()
await download(recipientVersion, bytes)

// Revoke the owner's only proof session before replication/download checks.
await request(`/v3/session/${owner.id}`, owner, 'DELETE')
const offlineDownloadStarted = performance.now()
await download(recipientVersion, bytes)
timings.ownerOfflineDownloadMs = Math.round(performance.now() - offlineDownloadStarted)
await waitForCopies(digest)
timings.secondCopyObservedAfterUploadMs = Math.round(performance.now() - uploadedAt)
const primaryNode =
	new URL(placement.upload_url).port === String(runtime.ports.storageA) ? 'storage-a' : 'storage-b'
await corruptProofCopy(primaryNode, digest, bytes)
await download(recipientVersion, bytes)
await removeProofCopy('storage-a', digest)
await removeProofCopy('storage-b', digest)
assert.equal((await fetch(recipientVersion.external_files[0].url)).status, 503)

// Restore exactly the immutable owner snapshot, then push a separate next version.
owner = await login(`proof_owner_${stamp}`)
placement = await request(`${upload}/prepare`, owner, 'POST', {
	sha256: digest,
	size: bytes.length,
})
assert.equal(placement.status, 'upload')
assert.equal(
	(
		await fetch(placement.upload_url, {
			method: 'PUT',
			body: await readFile(path.join(snapshotDirectory, digest)),
		})
	).status,
	204,
)
await download(recipientVersion, bytes)
const updateBytes = Buffer.concat([bytes, Buffer.from('\nversion two')])
await writeFile(path.join(snapshotDirectory, hash(updateBytes)), updateBytes, { flag: 'wx' })
const second = await request(`/v1/instances/${shared.id}/versions`, owner, 'POST', body, {
	'Idempotency-Key': `${stamp}-update`,
})
const updateUpload = new URL(second.external_files[0].url).pathname
const updatePlacement = await request(`${updateUpload}/prepare`, owner, 'POST', {
	sha256: hash(updateBytes),
	size: updateBytes.length,
})
assert.equal(
	(await fetch(updatePlacement.upload_url, { method: 'PUT', body: updateBytes })).status,
	204,
)
const latest = await request(`/v1/instances/${shared.id}/versions`, recipient)
assert.equal(latest.version, version.version + 1)
const updateDownloadStarted = performance.now()
await download(latest, updateBytes)
timings.updateDownloadMs = Math.round(performance.now() - updateDownloadStarted)
await request(`/v1/instances/${shared.id}/users`, owner, 'DELETE', {
	user_ids: [recipient.user_id],
})
assert.equal(
	(await fetch(latest.external_files[0].url)).status,
	401,
	'Revocation invalidates the metadata download capability',
)
timings.totalMs = Math.round(performance.now() - started)
const report = {
	passed: true,
	completedAt: new Date().toISOString(),
	instance: shared.id,
	versions: [version.version, latest.version],
	bytes: [bytes.length, updateBytes.length],
	sha256: [digest, hash(updateBytes)],
	timings,
	latestMetadata: {
		version: latest.version,
		ready: latest.ready,
		game_version: latest.game_version,
		loader: latest.loader,
		loader_version: latest.loader_version,
		external_files: latest.external_files.map(({ file_name, file_type, file_size, sha256 }) => ({
			file_name,
			file_type,
			file_size,
			sha256,
		})),
	},
	checks: [
		'friend acceptance',
		'resource-pack bytes',
		'wrong-hash rejected',
		'interrupted PUT retry',
		'recipient download and range',
		'interrupted GET retry',
		'owner session offline',
		'second-node pull',
		'equal-size corruption and replica fallback',
		'owner snapshot recovery',
		'update bytes',
		'unauthorized and revoked access',
	],
}
await writeFile(path.join(snapshotDirectory, 'report.json'), JSON.stringify(report, null, 2) + '\n')
console.log(JSON.stringify({ ...report, artifact: path.relative(root, snapshotDirectory) }))
