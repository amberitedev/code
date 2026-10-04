export class ApiError extends Error {
	constructor(
		public status: number,
		public error: string,
		public description: string,
	) {
		super(description)
	}
}

export function json(value: unknown, status = 200): Response {
	return Response.json(value, { status, headers: { 'Cache-Control': 'no-store' } })
}

export function invalid(description: string): never {
	throw new ApiError(400, 'invalid_input', description)
}

export function unauthorized(): never {
	throw new ApiError(401, 'invalid_credentials', 'Invalid Authentication Credentials')
}

export function stringField(body: Record<string, unknown>, key: string, max = 1024): string {
	const value = body[key]
	if (typeof value !== 'string' || value.length === 0 || value.length > max)
		invalid(`Invalid ${key}`)
	return value
}

export async function readJson(request: Request): Promise<Record<string, unknown>> {
	const text = new TextDecoder().decode(await readBytes(request, 65536))
	let value: unknown
	try {
		value = JSON.parse(text)
	} catch {
		invalid('Invalid JSON')
	}
	if (!value || typeof value !== 'object' || Array.isArray(value)) invalid('Expected a JSON object')
	return value as Record<string, unknown>
}

export async function readBytes(request: Request, limit: number): Promise<Uint8Array> {
	if (Number(request.headers.get('Content-Length')) > limit)
		throw new ApiError(413, 'invalid_input', 'Request body too large')
	const reader = request.body?.getReader()
	if (!reader) return new Uint8Array()
	const chunks: Uint8Array[] = []
	let size = 0
	try {
		while (true) {
			const { done, value } = await reader.read()
			if (done) break
			size += value.byteLength
			if (size > limit) {
				await reader.cancel()
				throw new ApiError(413, 'invalid_input', 'Request body too large')
			}
			chunks.push(value)
		}
	} finally {
		reader.releaseLock()
	}
	const bytes = new Uint8Array(size)
	let offset = 0
	for (const chunk of chunks) {
		bytes.set(chunk, offset)
		offset += chunk.length
	}
	return bytes
}

export function routePath(request: Request) {
	return new URL(request.url).pathname.replace(/^\/v[23](?=\/)/, '').replace(/\/$/, '') || '/'
}
