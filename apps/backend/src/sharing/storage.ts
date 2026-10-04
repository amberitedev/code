import { ApiError, json, readJson } from '../common/http'
import type { Env } from '../common/types'

export type StoredBlob = { id: string; instance_id: string; sha256: string; size: number }
export type StorageNode = { id: string; url: string; secret: string }
export type Transfer = {
	op: 'upload' | 'read' | 'receipt'
	node_id: string
	file_id: string
	sha256: string | null
	size: number
	expires: number
	receipt_url?: string
	receipt_token?: string
}
const MAX_FILE_SIZE = 500 * 1024 * 1024 // The existing native installer has the same limit.

export function storageNodes(env: Env): StorageNode[] {
	const parsed: unknown = JSON.parse(env.STORAGE_NODES ?? '[]')
	if (!Array.isArray(parsed)) throw new Error('STORAGE_NODES must be a JSON array')
	return parsed.map((node: unknown) => {
		if (
			!node ||
			typeof node !== 'object' ||
			!('id' in node) ||
			!('url' in node) ||
			!('secret' in node) ||
			typeof node.id !== 'string' ||
			typeof node.url !== 'string' ||
			typeof node.secret !== 'string' ||
			node.secret.length < 32
		) {
			throw new Error('Invalid storage node configuration')
		}
		const url = new URL(node.url)
		if (!['http:', 'https:'].includes(url.protocol)) throw new Error('Invalid storage node URL')
		return { id: node.id, url: node.url.replace(/\/$/, ''), secret: node.secret }
	})
}

function encode(bytes: Uint8Array): string {
	return btoa(String.fromCharCode(...bytes))
		.replaceAll('+', '-')
		.replaceAll('/', '_')
		.replaceAll('=', '')
}

function decode(value: string): Uint8Array<ArrayBuffer> {
	return Uint8Array.from(atob(value.replaceAll('-', '+').replaceAll('_', '/')), (character) =>
		character.charCodeAt(0),
	)
}

async function signingKey(secret: string) {
	return crypto.subtle.importKey(
		'raw',
		new TextEncoder().encode(secret),
		{ name: 'HMAC', hash: 'SHA-256' },
		false,
		['sign', 'verify'],
	)
}

export async function signTransfer(node: StorageNode, transfer: Transfer): Promise<string> {
	const payload = encode(new TextEncoder().encode(JSON.stringify(transfer)))
	const signature = await crypto.subtle.sign(
		'HMAC',
		await signingKey(node.secret),
		new TextEncoder().encode(payload),
	)
	return `${payload}.${encode(new Uint8Array(signature))}`
}

export async function verifyTransfer(node: StorageNode, token: string): Promise<Transfer> {
	const [payload, signature, extra] = token.split('.')
	if (!payload || !signature || extra)
		throw new ApiError(401, 'unauthorized', 'Invalid storage receipt')
	let valid = false
	try {
		valid = await crypto.subtle.verify(
			'HMAC',
			await signingKey(node.secret),
			decode(signature),
			new TextEncoder().encode(payload),
		)
	} catch {
		/* Invalid base64. */
	}
	if (!valid) throw new ApiError(401, 'unauthorized', 'Invalid storage receipt')
	const transfer = JSON.parse(new TextDecoder().decode(decode(payload))) as Transfer
	if (transfer.node_id !== node.id || transfer.expires < Date.now())
		throw new ApiError(401, 'unauthorized', 'Expired storage receipt')
	return transfer
}

export function validSize(value: unknown): number {
	if (
		typeof value !== 'number' ||
		!Number.isSafeInteger(value) ||
		value < 0 ||
		value > MAX_FILE_SIZE
	)
		throw new ApiError(400, 'invalid_input', 'Shared files must be at most 500 MiB')
	return value
}

export function validHash(value: unknown): string {
	if (typeof value !== 'string' || !/^[a-f0-9]{64}$/.test(value))
		throw new ApiError(400, 'invalid_input', 'A SHA256 content hash is required')
	return value
}

export async function onlineNodes(env: Env): Promise<StorageNode[]> {
	const rows = (
		await env.DB.prepare('SELECT id FROM storage_nodes WHERE last_seen > ?')
			.bind(Date.now() - 60000)
			.all<{ id: string }>()
	).results
	const online = new Set(rows.map((row) => row.id))
	return storageNodes(env).filter((node) => online.has(node.id))
}

export async function readUrl(
	node: StorageNode,
	fileId: string,
	blob: StoredBlob,
): Promise<string> {
	const capability = await signTransfer(node, {
		op: 'read',
		node_id: node.id,
		file_id: fileId,
		sha256: blob.sha256,
		size: blob.size,
		expires: Date.now() + 15 * 60 * 1000,
	})
	return `${node.url}/v1/files/${blob.sha256}?cap=${capability}`
}

export async function uploadUrl(
	node: StorageNode,
	origin: string,
	fileId: string,
	hash: string | null,
	size: number,
): Promise<string> {
	const grant: Transfer = {
		op: 'receipt',
		node_id: node.id,
		file_id: fileId,
		sha256: hash,
		size,
		expires: Date.now() + 60 * 60 * 1000,
	}
	const receiptToken = await signTransfer(node, grant)
	const capability = await signTransfer(node, {
		...grant,
		op: 'upload',
		receipt_url: `${origin}/v1/storage/receipt`,
		receipt_token: receiptToken,
	})
	return `${node.url}/v1/uploads/${fileId}?cap=${capability}`
}

export async function availableReplicas(env: Env, blob: StoredBlob): Promise<StorageNode[]> {
	const copies = (
		await env.DB.prepare('SELECT node_id FROM shared_replicas WHERE blob_id = ?')
			.bind(blob.id)
			.all<{ node_id: string }>()
	).results
	const ids = new Set(copies.map((copy) => copy.node_id))
	return (await onlineNodes(env)).filter((node) => ids.has(node.id))
}

/** Check the signed object metadata, not only a node heartbeat, before claiming availability. */
export async function verifiedReplica(
	env: Env,
	blob: StoredBlob,
	fileId: string,
): Promise<StorageNode | null> {
	for (const node of await availableReplicas(env, blob)) {
		try {
			const result = await fetch(await readUrl(node, fileId, blob), {
				method: 'HEAD',
				signal: AbortSignal.timeout(3000),
			})
			if (
				result.ok &&
				Number(result.headers.get('content-length')) === blob.size &&
				result.headers.get('x-content-sha256') === blob.sha256
			)
				return node
			// A confirmed missing copy is different from an unreachable node. Outages do not trigger replacement.
			if (result.ok || result.status === 404 || result.status === 422)
				await env.DB.prepare('DELETE FROM shared_replicas WHERE blob_id = ? AND node_id = ?')
					.bind(blob.id, node.id)
					.run()
		} catch {
			/* Preserve placement during transient failures. */
		}
	}
	return null
}

export async function prepareUpload(env: Env, origin: string, fileId: string, blob: StoredBlob) {
	if (await verifiedReplica(env, blob, fileId))
		return { status: 'available' as const, sha256: blob.sha256, size: blob.size }
	// Keep placements through node outages. Only a confirmed missing/corrupt
	// object removes a replica record and permits owner-snapshot recovery.
	if (
		await env.DB.prepare('SELECT 1 FROM shared_replicas WHERE blob_id = ? LIMIT 1')
			.bind(blob.id)
			.first()
	)
		return { status: 'pending' as const, retry_after: 15 }
	const candidates = await onlineNodes(env)
	if (!candidates.length) return { status: 'pending' as const, retry_after: 15 }
	return {
		status: 'upload' as const,
		upload_url: await uploadUrl(candidates[0]!, origin, fileId, blob.sha256, blob.size),
		sha256: blob.sha256,
		size: blob.size,
	}
}

export async function downloadBlob(
	request: Request,
	env: Env,
	blob: StoredBlob,
	fileId: string,
): Promise<Response> {
	const node = await verifiedReplica(env, blob, fileId)
	if (!node)
		throw new ApiError(
			503,
			'storage_unavailable',
			'No verified storage copy is currently online; retry later',
		)
	// The account token stays at the metadata service. Only a short-lived file capability goes to storage.
	return new Response(null, {
		status: 307,
		headers: { Location: await readUrl(node, fileId, blob), 'Cache-Control': 'private, no-store' },
	})
}

export async function storageHeartbeat(
	request: Request,
	env: Env,
	nodeId: string,
): Promise<Response> {
	const node = storageNodes(env).find((candidate) => candidate.id === nodeId)
	if (!node) throw new ApiError(401, 'unauthorized', 'Unknown storage node')
	const authorization = request.headers.get('authorization')?.replace(/^Bearer /, '') ?? ''
	const proof = await crypto.subtle.sign(
		'HMAC',
		await signingKey(node.secret),
		new TextEncoder().encode('heartbeat'),
	)
	const check = await crypto.subtle.sign(
		'HMAC',
		await signingKey(authorization),
		new TextEncoder().encode('heartbeat'),
	)
	if (!crypto.subtle.timingSafeEqual(proof, check))
		throw new ApiError(401, 'unauthorized', 'Invalid node authorization')
	await env.DB.prepare(
		'INSERT INTO storage_nodes (id,last_seen) VALUES (?,?) ON CONFLICT(id) DO UPDATE SET last_seen = excluded.last_seen',
	)
		.bind(node.id, Date.now())
		.run()
	const candidates = (
		await env.DB.prepare(
			`SELECT b.*, (SELECT id FROM shared_files WHERE blob_id = b.id LIMIT 1) AS file_id
		FROM shared_blobs b WHERE EXISTS (SELECT 1 FROM shared_replicas WHERE blob_id = b.id)
		AND EXISTS (SELECT 1 FROM shared_files WHERE blob_id = b.id)
		AND NOT EXISTS (SELECT 1 FROM shared_replicas WHERE blob_id = b.id AND node_id = ?)
		AND (SELECT COUNT(*) FROM shared_replicas WHERE blob_id = b.id) < 2 LIMIT 10`,
		)
			.bind(node.id)
			.all<StoredBlob & { file_id: string | null }>()
	).results
	for (const blob of candidates) {
		if (!blob.file_id) continue
		const source = await verifiedReplica(env, blob, blob.file_id)
		if (!source) continue
		return json({
			jobs: [
				{
					source_url: await readUrl(source, blob.file_id, blob),
					upload_url: await uploadUrl(
						node,
						new URL(request.url).origin,
						blob.file_id,
						blob.sha256,
						blob.size,
					),
					sha256: blob.sha256,
					size: blob.size,
				},
			],
		})
	}
	return json({ jobs: [] })
}

export async function acceptStorageReceipt(
	request: Request,
	env: Env,
): Promise<{ fileId: string; blob: StoredBlob }> {
	const body = await readJson(request)
	const node = storageNodes(env).find((candidate) => candidate.id === body.node_id)
	if (!node || typeof body.receipt !== 'string')
		throw new ApiError(401, 'unauthorized', 'Invalid storage receipt')
	const grant = await verifyTransfer(node, body.receipt)
	const hash = validHash(body.sha256)
	const size = validSize(body.size)
	if (
		grant.op !== 'receipt' ||
		grant.file_id !== body.file_id ||
		(grant.sha256 && grant.sha256 !== hash) ||
		grant.size !== size
	)
		throw new ApiError(409, 'integrity_error', 'Receipt does not match the upload grant')
	const file = await env.DB.prepare('SELECT instance_id,blob_id FROM shared_files WHERE id = ?')
		.bind(grant.file_id)
		.first<{ instance_id: string; blob_id: string | null }>()
	if (!file) throw new ApiError(404, 'not_found', 'Upload is no longer retained')
	const stored: StoredBlob = {
		id: crypto.randomUUID(),
		instance_id: file.instance_id,
		sha256: hash,
		size,
	}
	const verification = await fetch(await readUrl(node, grant.file_id, stored), {
		method: 'HEAD',
		signal: AbortSignal.timeout(5000),
	})
	if (
		!verification.ok ||
		Number(verification.headers.get('content-length')) !== size ||
		verification.headers.get('x-content-sha256') !== hash
	)
		throw new ApiError(409, 'integrity_error', 'Destination did not confirm the committed content')
	if (file.blob_id) {
		const expected = await env.DB.prepare('SELECT sha256,size FROM shared_blobs WHERE id = ?')
			.bind(file.blob_id)
			.first<{ sha256: string; size: number }>()
		if (!expected || expected.sha256 !== hash || expected.size !== size)
			throw new ApiError(409, 'integrity_error', 'A committed version cannot be changed')
	}
	await env.DB.batch([
		env.DB.prepare(
			'INSERT OR IGNORE INTO shared_blobs (id,instance_id,sha256,size,created) VALUES (?,?,?,?,?)',
		).bind(stored.id, file.instance_id, hash, size, new Date().toISOString()),
		env.DB.prepare(
			'UPDATE shared_files SET blob_id = (SELECT id FROM shared_blobs WHERE instance_id = ? AND sha256 = ?) WHERE id = ? AND (blob_id IS NULL OR blob_id = (SELECT id FROM shared_blobs WHERE instance_id = ? AND sha256 = ?))',
		).bind(file.instance_id, hash, grant.file_id, file.instance_id, hash),
		env.DB.prepare(
			'INSERT INTO shared_replicas (blob_id,node_id,verified) SELECT f.blob_id,?,? FROM shared_files f JOIN shared_blobs b ON b.id = f.blob_id WHERE f.id = ? AND b.sha256 = ? AND b.size = ? ON CONFLICT(blob_id,node_id) DO UPDATE SET verified = excluded.verified',
		).bind(node.id, Date.now(), grant.file_id, hash, size),
	])
	const result = await env.DB.prepare(
		'SELECT b.* FROM shared_blobs b JOIN shared_files f ON f.blob_id = b.id WHERE f.id = ?',
	)
		.bind(grant.file_id)
		.first<StoredBlob>()
	if (!result) throw new ApiError(500, 'database_error', 'Could not persist upload receipt')
	if (result.sha256 !== hash || result.size !== size)
		throw new ApiError(409, 'integrity_error', 'Another upload already committed different content')
	return { fileId: grant.file_id, blob: result }
}
