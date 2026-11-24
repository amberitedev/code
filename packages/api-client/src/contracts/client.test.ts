import type { Endpoint } from '@amberite/contracts'
import { afterEach, describe, expect, it, vi } from 'vite-plus/test'

import { ModrinthApiError, ModrinthServerError } from '../core/errors'
import { SelfHostedFeature } from '../features/self-hosted'
import { GenericModrinthClient } from '../platform/generic'
import { ContractClient, type ContractTransport, fetchWithProgress } from './client'
import { ApiCallError } from './errors'

const renameFile = {
	method: 'POST',
	path: '/v1/instances/{id}/files/{*path}',
	auth: 'session',
	dangerous: false,
	body: 'json',
	response: 'json',
} as Endpoint<{ id: string; path: string }, { force: boolean | null }, { name: string }, string[]>

const deleteCache = {
	method: 'DELETE',
	path: '/v1/cache',
	auth: 'session',
	dangerous: false,
	body: 'empty',
	response: 'empty',
} as Endpoint<null, null, void, void>

const writeFile = {
	method: 'PUT',
	path: '/v1/instances/{id}/file',
	auth: 'session',
	dangerous: false,
	body: 'raw',
	response: 'json',
} as Endpoint<{ id: string }, null, Blob, string[]>

const download = {
	method: 'GET',
	path: '/v1/download',
	auth: 'none',
	dangerous: false,
	body: 'empty',
	response: 'stream',
} as Endpoint<null, null, void, Response>

type Sent = {
	url: string
	method?: string
	headers: Record<string, string>
	body?: BodyInit | null
}

/** A client whose server answers every request with `answer` and records what it was sent. */
function serve(answer: () => Response) {
	const sent: Sent[] = []
	const client = new ContractClient({
		base: 'http://127.0.0.1:5000/secret/',
		headers: async () => ({ Authorization: 'Bearer session' }),
		fetch: async (input, init) => {
			sent.push({
				url: String(input),
				method: init?.method,
				headers: { ...(init?.headers as Record<string, string>) },
				body: init?.body,
			})
			return answer()
		},
	})
	return { client, sent }
}

describe('ContractClient', () => {
	it('builds the URL, query and body from the endpoint', async () => {
		const { client, sent } = serve(() => Response.json(['done']))
		expect(client.url(renameFile, { params: { id: 'i', path: 'p' }, query: { force: true } })).toBe(
			'http://127.0.0.1:5000/secret/v1/instances/i/files/p?force=true',
		)
		expect(client.url(renameFile, { params: { id: 'i', path: 'p' }, query: { force: null } })).toBe(
			'http://127.0.0.1:5000/secret/v1/instances/i/files/p',
		)

		const answer = await client.call(renameFile, {
			params: { id: 'a b/c', path: 'config/x y.json' },
			query: { force: true },
			body: { name: 'z' },
		})
		await client.call(renameFile, {
			params: { id: 'a', path: 'b' },
			query: { force: null },
			body: { name: 'z' },
		})

		expect(answer).toEqual(['done'])
		expect(sent[0]).toEqual({
			url: 'http://127.0.0.1:5000/secret/v1/instances/a%20b%2Fc/files/config/x%20y.json?force=true',
			method: 'POST',
			headers: { Authorization: 'Bearer session', 'Content-Type': 'application/json' },
			body: '{"name":"z"}',
		})
		expect(sent[1].url).toBe('http://127.0.0.1:5000/secret/v1/instances/a/files/b')
	})

	it('refuses a param that would reach another route', () => {
		const { client } = serve(() => Response.json([]))

		for (const params of [
			{ id: '..', path: 'x' },
			{ id: '', path: 'x' },
			{ id: 'a', path: 'config/../..' },
			{ id: 'a', path: './x' },
		]) {
			expect(() => client.url(renameFile, { params, query: { force: null } })).toThrow(
				'is not a valid',
			)
		}
	})

	it('sends no body and reads none for empty routes', async () => {
		const { client, sent } = serve(() => new Response(null, { status: 204 }))

		await expect(client.call(deleteCache)).resolves.toBeUndefined()
		expect(sent[0].body).toBeUndefined()
		expect(sent[0].headers).toEqual({ Authorization: 'Bearer session' })
	})

	it('throws the ApiError the route answered with', async () => {
		const { client } = serve(() =>
			Response.json({ error: 'not_found', description: 'No such instance.' }, { status: 404 }),
		)

		const error = await client.call(deleteCache).catch((error: unknown) => error)

		expect(error).toBeInstanceOf(ApiCallError)
		expect(error).toMatchObject({
			status: 404,
			code: 'not_found',
			description: 'No such instance.',
		})
	})

	it('resumes a download only from the range it asked for', async () => {
		const ignored = serve(() => new Response('whole'))
		await expect(ignored.client.stream(download, {}, { from: 10 })).rejects.toMatchObject({
			status: 200,
		})
		expect(ignored.sent[0].headers.Range).toBe('bytes=10-')

		const resumed = serve(() => new Response('rest', { status: 206 }))
		await expect(resumed.client.stream(download, {}, { from: 10 })).resolves.toBeInstanceOf(
			Response,
		)
	})

	it('throws an unknown error when the body is not an ApiError', async () => {
		const { client } = serve(() => new Response('Bad Gateway', { status: 502 }))

		await expect(client.call(deleteCache)).rejects.toMatchObject({
			status: 502,
			code: 'unknown',
			description: 'Bad Gateway',
		})
	})

	it('throws network_error when no answer arrives', async () => {
		const client = new ContractClient({
			base: 'http://127.0.0.1:5000',
			headers: async () => ({}),
			fetch: async () => {
				throw new TypeError('Failed to fetch')
			},
		})

		await expect(client.call(deleteCache)).rejects.toMatchObject({
			status: 0,
			code: 'network_error',
			description: 'Failed to fetch',
		})
	})
})

/** A transport whose server never answers: a request ends only when its signal aborts. */
function unanswered(timeout?: number): ContractTransport {
	return {
		base: 'http://127.0.0.1:5000',
		headers: async () => ({}),
		fetch: (_url, { signal }) =>
			new Promise((_, reject) => {
				const abort = () => reject(new DOMException('Aborted', 'AbortError'))
				if (signal?.aborted) abort()
				signal?.addEventListener('abort', abort)
			}),
		timeout,
	}
}

describe('a call that is not answered', () => {
	afterEach(() => {
		vi.useRealTimers()
	})

	it('fails after the timeout, which a file body waits past unless it asks for one', async () => {
		vi.useFakeTimers()
		const client = new ContractClient(unanswered(5000))
		const call = client.call(deleteCache).catch((error: unknown) => error)
		let uploadSettled = false
		void client
			.call(writeFile, { params: { id: 'a' }, body: new Blob(['file']) })
			.catch(() => undefined)
			.finally(() => (uploadSettled = true))
		const limited = client
			.call(writeFile, { params: { id: 'a' }, body: new Blob(['file']) }, { timeout: 1000 })
			.catch((error: unknown) => error)

		await vi.advanceTimersByTimeAsync(5000)

		expect(await call).toMatchObject({ status: 0, code: 'network_error' })
		expect(await limited).toMatchObject({ status: 0, code: 'network_error' })
		expect(uploadSettled).toBe(false)
	})

	it('is cancelled by its signal', async () => {
		const client = new ContractClient(unanswered())
		const abort = new AbortController()
		const call = client.call(deleteCache, {}, { signal: abort.signal })
		abort.abort()

		await expect(call).rejects.toMatchObject({ status: 0, code: 'cancelled' })
	})
})

/** An `XMLHttpRequest` the test answers. `sent` resolves with the next one sent. */
class FakeXhr {
	static sent: Promise<FakeXhr>
	private static resolveSent: (xhr: FakeXhr) => void
	static expectNext() {
		FakeXhr.sent = new Promise((resolve) => (FakeXhr.resolveSent = resolve))
	}

	upload: { onprogress: ((event: Partial<ProgressEvent>) => void) | null } = { onprogress: null }
	onload: (() => void) | null = null
	onerror: (() => void) | null = null
	onabort: (() => void) | null = null
	responseType = ''
	method = ''
	url = ''
	headers: Record<string, string> = {}
	body: unknown
	status = 0
	statusText = ''
	response: Blob | null = null
	aborted = false

	open(method: string, url: string) {
		this.method = method
		this.url = url
	}
	setRequestHeader(name: string, value: string) {
		this.headers[name] = value
	}
	getAllResponseHeaders() {
		return 'content-type: application/json\r\n'
	}
	send(body: unknown) {
		this.body = body
		FakeXhr.resolveSent(this)
	}
	abort() {
		this.aborted = true
		this.onabort?.()
	}

	progress(loaded: number, total: number) {
		this.upload.onprogress?.({ lengthComputable: true, loaded, total })
	}
	answer(status: number, body: string) {
		this.status = status
		this.response = new Blob([body])
		this.onload?.()
	}
}

describe('an upload with progress', () => {
	afterEach(() => {
		vi.unstubAllGlobals()
	})

	function uploading() {
		vi.stubGlobal('XMLHttpRequest', FakeXhr)
		FakeXhr.expectNext()
		return new ContractClient({
			base: 'http://127.0.0.1:5000',
			headers: async () => ({ Authorization: 'Bearer session' }),
			fetch: fetchWithProgress,
		})
	}

	it('reports the body as it is sent, then reads the answer', async () => {
		const client = uploading()
		const file = new Blob(['abcd'])
		const progress: number[] = []
		const call = client.call(
			writeFile,
			{ params: { id: 'a' }, body: file },
			{ onUploadProgress: ({ progress: part }) => progress.push(part) },
		)

		const xhr = await FakeXhr.sent
		xhr.progress(2, 4)
		xhr.progress(4, 4)
		xhr.answer(200, '["done"]')

		await expect(call).resolves.toEqual(['done'])
		expect(progress).toEqual([0.5, 1])
		expect(xhr).toMatchObject({
			method: 'PUT',
			url: 'http://127.0.0.1:5000/v1/instances/a/file',
			headers: { authorization: 'Bearer session', 'content-type': 'application/octet-stream' },
			body: file,
		})
	})

	it('aborts the request when its signal aborts', async () => {
		const client = uploading()
		const abort = new AbortController()
		const call = client.call(
			writeFile,
			{ params: { id: 'a' }, body: new Blob(['abcd']) },
			{ signal: abort.signal, onUploadProgress: () => undefined },
		)

		const xhr = await FakeXhr.sent
		abort.abort()

		await expect(call).rejects.toMatchObject({ status: 0, code: 'cancelled' })
		expect(xhr.aborted).toBe(true)
	})
})

describe('ApiCallError.from', () => {
	it('types a refused upstream request by the code it answered', () => {
		const refused = ModrinthServerError.fromResponse(403, {
			error: 'not_owner',
			description: 'Only the owner can.',
		})
		expect(ApiCallError.from(refused)).toMatchObject({
			status: 403,
			code: 'not_owner',
			description: 'Only the owner can.',
		})
		expect(ApiCallError.from(ModrinthServerError.fromResponse(502, 'Bad Gateway'))).toMatchObject({
			status: 502,
			code: 'unknown',
		})

		const unanswered = new ModrinthApiError('Network error')
		expect(ApiCallError.from(unanswered)).toBe(unanswered)
	})
})

describe('client.backend', () => {
	it('follows the SelfHostedFeature the client has now', () => {
		const backend = (baseUrl: string) =>
			new SelfHostedFeature({ baseUrl, token: async () => undefined })
		const first = backend('https://one.example')
		const client = new GenericModrinthClient({ features: [first] })
		expect(client.backend.url(deleteCache)).toBe('https://one.example/v1/cache')

		client.removeFeature(first).addFeature(backend('https://two.example'))
		expect(client.backend.url(deleteCache)).toBe('https://two.example/v1/cache')
	})
})
