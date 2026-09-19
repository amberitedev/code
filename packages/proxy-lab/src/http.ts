import type { IncomingMessage, ServerResponse } from 'node:http'

const BODY_LIMIT = 4 * 1024 * 1024

export class HttpError extends Error {
	readonly status: number
	readonly details: unknown

	constructor(status: number, message: string, details?: unknown) {
		super(message)
		this.status = status
		this.details = details
	}
}

export function json(response: ServerResponse, status: number, body: unknown): void {
	const value = JSON.stringify(body)
	response.writeHead(status, {
		'access-control-allow-headers': 'authorization, content-type, last-event-id',
		'access-control-allow-methods': 'GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS',
		'access-control-allow-origin': 'http://localhost:1420',
		'cache-control': 'no-store',
		'content-length': Buffer.byteLength(value),
		'content-type': 'application/json; charset=utf-8',
	})
	response.end(value)
}

export function empty(response: ServerResponse, status = 204): void {
	response.writeHead(status, {
		'access-control-allow-headers': 'authorization, content-type, last-event-id',
		'access-control-allow-methods': 'GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS',
		'access-control-allow-origin': 'http://localhost:1420',
		'cache-control': 'no-store',
	})
	response.end()
}

export async function readJson(request: IncomingMessage): Promise<unknown> {
	const body = await readBody(request)
	if (body.length === 0) return undefined
	try {
		return JSON.parse(body.toString('utf8')) as unknown
	} catch {
		throw new HttpError(400, 'Request body must contain valid JSON')
	}
}

export async function readBody(request: IncomingMessage): Promise<Buffer> {
	const chunks: Buffer[] = []
	let size = 0
	for await (const chunk of request) {
		const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)
		size += buffer.length
		if (size > BODY_LIMIT) throw new HttpError(413, `Request body exceeds ${BODY_LIMIT} bytes`)
		chunks.push(buffer)
	}
	return Buffer.concat(chunks)
}

export function objectBody(value: unknown): Record<string, unknown> {
	if (typeof value !== 'object' || value === null || Array.isArray(value)) {
		throw new HttpError(400, 'Request body must be a JSON object')
	}
	return value as Record<string, unknown>
}

export function stringField(body: Record<string, unknown>, field: string): string {
	const value = body[field]
	if (typeof value !== 'string' || value.length === 0) {
		throw new HttpError(400, `Field "${field}" must be a non-empty string`)
	}
	return value
}

export function stringArrayField(body: Record<string, unknown>, field: string): string[] {
	const value = body[field]
	if (!Array.isArray(value) || !value.every((item) => typeof item === 'string')) {
		throw new HttpError(400, `Field "${field}" must be an array of strings`)
	}
	return value
}
