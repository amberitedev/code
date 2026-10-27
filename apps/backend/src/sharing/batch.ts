import { Readable, pipeline } from 'node:stream'
import type { ReadableStream as NodeReadableStream } from 'node:stream/web'
import { createZstdDecompress } from 'node:zlib'

import { ApiError } from '../common/http'

// The App publishes a version's files in batches: one `multipart/form-data` body, compressed with
// zstd, whose first part is a JSON manifest and whose remaining parts are the files in manifest
// order. The parser here reads one part at a time so a batch is never held in memory whole.

const encoder = new TextEncoder()
const decoder = new TextDecoder()
const CRLF_CRLF = encoder.encode('\r\n\r\n')
const MAX_HEADERS_SIZE = 8192

/** The request body with its content encoding undone. Workers decode gzip natively, not zstd. */
export function decodedBody(request: Request): ReadableStream<Uint8Array> {
	const body =
		request.body ?? new ReadableStream<Uint8Array>({ start: (controller) => controller.close() })
	const encoding = request.headers.get('content-encoding')?.trim().toLowerCase() ?? ''
	if (!encoding || encoding === 'identity') return body
	if (encoding !== 'zstd') throw new ApiError(415, 'invalid_input', 'Unsupported content encoding')
	// The Workers and Node stream types describe the same object; only Node's zlib knows zstd.
	const compressed = body as unknown as NodeReadableStream<Uint8Array>
	const decompressor = createZstdDecompress()
	// A pipeline, unlike pipe(), fails the decompressor when the client gives up, so a read after
	// an aborted upload errors instead of waiting forever.
	pipeline(Readable.fromWeb(compressed), decompressor, () => undefined)
	return Readable.toWeb(decompressor) as unknown as ReadableStream<Uint8Array>
}

export function multipartBoundary(contentType: string | null): string {
	const match = /^multipart\/form-data\s*;.*?\bboundary=(?:"([^"]+)"|([^;\s]+))/i.exec(
		contentType ?? '',
	)
	const boundary = match?.[1] ?? match?.[2]
	if (!boundary || boundary.length > 70)
		throw new ApiError(400, 'invalid_input', 'Expected a multipart body')
	return boundary
}

export type MultipartPart = { name: string; bytes: Uint8Array }

const malformed = () => new ApiError(400, 'invalid_input', 'Malformed multipart body')
const tooLarge = () => new ApiError(413, 'invalid_input', 'Request body too large')

/**
 * Yields the parts of a multipart/form-data body in order. `limit` says how many bytes the part at
 * `index` may hold; a longer part fails with 413 as soon as that is certain, before it is read.
 */
export async function* multipartParts(
	stream: ReadableStream<Uint8Array>,
	boundary: string,
	limit: (index: number, name: string) => number,
): AsyncGenerator<MultipartPart> {
	const delimiter = encoder.encode(`\r\n--${boundary}`)
	const buffer = new ByteBuffer(stream)
	try {
		// The first delimiter has no line break before it.
		const opening = await buffer.find(delimiter.subarray(2), MAX_HEADERS_SIZE, malformed)
		if (opening < 0) throw malformed()
		buffer.consume(opening + delimiter.length - 2)
		for (let index = 0; ; index++) {
			const lead = await buffer.take(2)
			if (lead[0] === 0x2d && lead[1] === 0x2d) return
			if (lead[0] !== 0x0d || lead[1] !== 0x0a) throw malformed()
			const headersEnd = await buffer.find(CRLF_CRLF, MAX_HEADERS_SIZE, malformed)
			if (headersEnd < 0) throw malformed()
			const headers = decoder.decode(await buffer.take(headersEnd + CRLF_CRLF.length))
			const name = /^content-disposition:.*?\bname="([^"]*)"/im.exec(headers)?.[1]
			if (name === undefined) throw malformed()
			const end = await buffer.find(delimiter, limit(index, name), tooLarge)
			if (end < 0) throw malformed()
			const bytes = await buffer.take(end)
			buffer.consume(delimiter.length)
			yield { name, bytes }
		}
	} finally {
		await buffer.close()
	}
}

/** Bytes read from a stream so far, grown as needed and trimmed from the front once consumed. */
class ByteBuffer {
	private data = new Uint8Array(64 * 1024)
	private length = 0
	private ended = false
	private readonly reader: ReadableStreamDefaultReader<Uint8Array>

	constructor(stream: ReadableStream<Uint8Array>) {
		this.reader = stream.getReader()
	}

	/** Appends the next chunk, or returns false once the stream has ended. */
	private async fill(): Promise<boolean> {
		if (this.ended) return false
		// A read fails only when the body stops early or is not valid zstd: both are the client's.
		const { done, value } = await this.reader.read().catch(() => {
			throw malformed()
		})
		if (done) {
			this.ended = true
			return false
		}
		if (this.length + value.length > this.data.length) {
			const grown = new Uint8Array(Math.max(this.data.length * 2, this.length + value.length))
			grown.set(this.data.subarray(0, this.length))
			this.data = grown
		}
		this.data.set(value, this.length)
		this.length += value.length
		return true
	}

	/**
	 * Offset of `needle`, reading more of the stream until it appears. Throws `fail` once more than
	 * `limit` bytes are known to precede it, and returns -1 if the stream ends first.
	 */
	async find(needle: Uint8Array, limit: number, fail: () => ApiError): Promise<number> {
		let from = 0
		while (true) {
			// Search only the filled part: the spare capacity behind it would be scanned on every chunk.
			const filled = this.data.subarray(0, this.length)
			const last = this.length - needle.length
			for (
				let offset = filled.indexOf(needle[0], from);
				offset !== -1 && offset <= last;
				offset = filled.indexOf(needle[0], offset + 1)
			) {
				let matched = 1
				while (matched < needle.length && filled[offset + matched] === needle[matched]) matched++
				if (matched === needle.length) return offset
			}
			// Every byte before the last possible start of the needle is part of the content.
			from = Math.max(0, this.length - needle.length + 1)
			if (from > limit) throw fail()
			if (!(await this.fill())) return -1
		}
	}

	/** Removes and returns the first `count` bytes, reading more of the stream when needed. */
	async take(count: number): Promise<Uint8Array> {
		while (this.length < count) if (!(await this.fill())) throw malformed()
		const bytes = this.data.slice(0, count)
		this.consume(count)
		return bytes
	}

	consume(count: number): void {
		this.data.copyWithin(0, count, this.length)
		this.length -= count
	}

	/** Reads the rest of the stream, so an early error response does not cut off the upload. */
	async close(): Promise<void> {
		await drain(this.reader)
	}
}

async function drain(reader: ReadableStreamDefaultReader<Uint8Array>): Promise<void> {
	try {
		while (!(await reader.read()).done);
	} catch {
		// The sender gave up; there is nothing left to read.
	}
}

/**
 * Reads the request body to its end unless something else is reading it. Called before answering
 * an upload with an error, so the client sees the answer instead of a closed connection.
 */
export async function drainBody(request: Request): Promise<void> {
	if (!request.body || request.body.locked) return
	await drain(request.body.getReader())
}
