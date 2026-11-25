import type { BodyKind, Endpoint, ErrorCode } from '@amberite/contracts/protocol'

import type { UploadProgress } from '../types/upload'
import { ApiCallError } from './errors'

/** What a `ContractClient` hands its transport's `fetch`. */
export type TransportInit = RequestInit & {
	/** Set only on a call that asked for upload progress. `fetchWithProgress` reports it. */
	onUploadProgress?: (progress: UploadProgress) => void
}

/** Where a `ContractClient` sends its calls, and what each call carries. */
export interface ContractTransport {
	/** A string when it is known up front, so `url` works before any call. */
	base: string | (() => Promise<string>)
	/** Given the route, so a transport can leave out credentials or attach a fresh ticket. */
	headers(endpoint: Endpoint): Promise<Record<string, string>>
	fetch(url: string, init: TransportInit): Promise<Response>
	/**
	 * Milliseconds a call without a file body waits for its answer, for every route or by route.
	 * None when unset.
	 */
	timeout?: number | ((endpoint: Endpoint) => number | undefined)
}

/** How one call is sent. */
export interface CallOptions {
	/** Aborting rejects the call with `cancelled`. */
	signal?: AbortSignal
	/** Milliseconds to wait for the answer, in place of the transport's. */
	timeout?: number
	/** Reports the body as it is sent, when the transport supports it (see `TransportInit`). */
	onUploadProgress?: (progress: UploadProgress) => void
}

type TypesOf<E> =
	E extends Endpoint<infer P, infer Q, infer B, infer R>
		? { params: P; query: Q; body: B; response: R }
		: never

export type ParamsOf<E extends Endpoint> = TypesOf<E>['params']
export type QueryOf<E extends Endpoint> = TypesOf<E>['query']
export type BodyOf<E extends Endpoint> = TypesOf<E>['body']
export type ResponseOf<E extends Endpoint> = TypesOf<E>['response']

/** Rust's `()` arrives as `null` and an empty body as `void`: neither is passed. */
type Slot<K extends string, T> = [T] extends [null | void]
	? { [_ in K]?: undefined }
	: { [_ in K]: T }

export type InputOf<E extends Endpoint> = Slot<'params', ParamsOf<E>> &
	Slot<'query', QueryOf<E>> &
	Slot<'body', BodyOf<E>>

/** What a route's URL is made of. */
export type LocationOf<E extends Endpoint> = Slot<'params', ParamsOf<E>> & Slot<'query', QueryOf<E>>

/** An input with nothing required may be left out. */
type Args<I, O = never> = {} extends I ? [input?: I, options?: O] : [input: I, options?: O]

type Input = { params?: unknown; query?: unknown; body?: unknown }

/**
 * Calls the routes of one server, typed by their generated `Endpoint`s. A call fails with an
 * `ApiCallError`: the route's own error, or `network_error` or `cancelled` with status 0.
 */
export class ContractClient {
	private knownBase: string | undefined

	constructor(private readonly transport: ContractTransport) {
		if (typeof transport.base === 'string') this.knownBase = trimBase(transport.base)
	}

	async call<E extends Endpoint>(
		endpoint: E,
		...[input, options]: Args<InputOf<E>, CallOptions>
	): Promise<ResponseOf<E>> {
		const answer = await this.send(endpoint, input ?? {}, options ?? {}, async (response) =>
			read(response, endpoint.response),
		)
		return answer as ResponseOf<E>
	}

	/** The route's URL, for an `<img>` or a link. An async base is known once a call resolved it. */
	url<E extends Endpoint>(endpoint: E, ...[location]: Args<LocationOf<E>>): string {
		if (this.knownBase === undefined) throw new Error('No call has resolved the base URL yet')
		return this.knownBase + target(endpoint, location ?? {})
	}

	/**
	 * The response, to read as it arrives; `from` resumes a download at that byte. A timeout ends
	 * with the answer's headers, and `signal` still aborts reading the body.
	 */
	async stream<E extends Endpoint>(
		endpoint: E,
		input: InputOf<E>,
		options: CallOptions & { from?: number } = {},
	): Promise<Response> {
		return this.send(endpoint, input, options, async (response) => response)
	}

	private async send<T>(
		endpoint: Endpoint,
		input: Input,
		options: CallOptions & { from?: number },
		finish: (response: Response) => Promise<T>,
	): Promise<T> {
		const { base } = this.transport
		const baseUrl = trimBase(typeof base === 'string' ? base : await base())
		this.knownBase = baseUrl
		const headers = { ...(await this.transport.headers(endpoint)) }
		let body: BodyInit | undefined
		if (endpoint.body === 'json') {
			headers['Content-Type'] = 'application/json'
			body = JSON.stringify(input.body)
		} else if (
			(endpoint.body === 'raw' || endpoint.body === 'stream') &&
			input.body instanceof Blob
		) {
			headers['Content-Type'] = input.body.type || 'application/octet-stream'
			body = input.body
		}
		const { from, signal, onUploadProgress } = options
		if (from) headers.Range = `bytes=${from}-`

		// A file can take any time to send, so only a timeout asked for limits it.
		const defaultTimeout =
			typeof this.transport.timeout === 'function'
				? this.transport.timeout(endpoint)
				: this.transport.timeout
		const timeout = options.timeout ?? (body instanceof Blob ? undefined : defaultTimeout)
		const controller = new AbortController()
		let timedOut: ApiCallError | undefined
		const timer =
			timeout === undefined
				? undefined
				: setTimeout(() => {
						timedOut = new ApiCallError(
							0,
							'network_error',
							`No answer within ${timeout / 1000} seconds`,
						)
						controller.abort()
					}, timeout)
		const abort = () => controller.abort()
		if (signal?.aborted) abort()
		signal?.addEventListener('abort', abort, { once: true })

		try {
			const response = await this.transport.fetch(baseUrl + target(endpoint, input), {
				method: endpoint.method,
				headers,
				body,
				signal: controller.signal,
				...(onUploadProgress ? { onUploadProgress } : {}),
			})
			if (!response.ok) throw await decodeError(response)
			if (from && response.status !== 206) {
				await response.body?.cancel()
				throw new ApiCallError(response.status, 'unknown', 'The server ignored the requested range')
			}
			return await finish(response)
		} catch (error) {
			if (error instanceof ApiCallError) throw error
			if (controller.signal.aborted) {
				throw timedOut ?? new ApiCallError(0, 'cancelled', 'Cancelled')
			}
			// What fetch rejects with when the request never got an answer.
			if (error instanceof TypeError) throw new ApiCallError(0, 'network_error', error.message)
			throw error
		} finally {
			clearTimeout(timer)
			// A stream's body is still being read, so the caller's signal keeps aborting it.
			if (endpoint.response !== 'stream') signal?.removeEventListener('abort', abort)
		}
	}
}

/**
 * `fetch`, but sent with `XMLHttpRequest` when `init.onUploadProgress` is set, since fetch cannot
 * report an upload's progress.
 */
export function fetchWithProgress(url: string, init: TransportInit = {}): Promise<Response> {
	const { onUploadProgress, ...rest } = init
	if (!onUploadProgress) return fetch(url, rest)
	return new Promise((resolve, reject) => {
		const aborted = () => new DOMException('The upload was aborted', 'AbortError')
		if (init.signal?.aborted) return reject(aborted())
		const xhr = new XMLHttpRequest()
		xhr.open(init.method ?? 'GET', url)
		xhr.responseType = 'blob'
		new Headers(init.headers).forEach((value, name) => xhr.setRequestHeader(name, value))
		xhr.upload.onprogress = (event) => {
			if (!event.lengthComputable) return
			onUploadProgress({
				loaded: event.loaded,
				total: event.total,
				progress: event.total > 0 ? event.loaded / event.total : 1,
			})
		}
		xhr.onload = () => {
			const empty = [101, 204, 205, 304].includes(xhr.status)
			resolve(
				new Response(empty ? null : (xhr.response as Blob), {
					status: xhr.status,
					statusText: xhr.statusText,
					headers: responseHeaders(xhr.getAllResponseHeaders()),
				}),
			)
		}
		xhr.onerror = () => reject(new TypeError('The upload failed'))
		xhr.onabort = () => reject(aborted())
		init.signal?.addEventListener('abort', () => xhr.abort(), { once: true })
		xhr.send((init.body ?? null) as XMLHttpRequestBodyInit | null)
	})
}

function responseHeaders(raw: string): Headers {
	const headers = new Headers()
	for (const line of raw.trim().split(/[\r\n]+/)) {
		const colon = line.indexOf(':')
		if (colon > 0) headers.append(line.slice(0, colon).trim(), line.slice(colon + 1).trim())
	}
	return headers
}

function read(response: Response, kind: BodyKind): unknown {
	switch (kind) {
		case 'json':
			return response.json()
		case 'raw':
			return response.blob()
		case 'stream':
			return response
		case 'empty':
			return undefined
		case 'upgrade':
			throw new Error('A socket route is opened, not called')
	}
}

function trimBase(base: string): string {
	return base.replace(/\/+$/, '')
}

/** The route's path with its params filled in, and its query. */
function target(endpoint: Endpoint, { params, query }: Input): string {
	return fillPath(endpoint.path, params) + search(query)
}

/** Fills each `{name}` (or `{*name}`, which keeps its slashes) from `params`. */
function fillPath(path: string, params: unknown): string {
	return path.replace(/\{(\*?)(\w+)\}/g, (_, wildcard: string, name: string) => {
		const value = isRecord(params) ? params[name] : undefined
		if (typeof value !== 'string' && typeof value !== 'number') {
			throw new Error(`${path} has no value for ${name}`)
		}
		const pieces = wildcard ? String(value).split('/') : [String(value)]
		// URL parsers resolve dot segments, even percent-encoded ones.
		if (pieces.some((piece) => piece === '' || piece === '.' || piece === '..')) {
			throw new Error(`${JSON.stringify(value)} is not a valid ${name} for ${path}`)
		}
		return pieces.map(encodeURIComponent).join('/')
	})
}

function search(query: unknown): string {
	if (!isRecord(query)) return ''
	const params = new URLSearchParams()
	for (const [key, value] of Object.entries(query)) {
		if (value !== undefined && value !== null) params.append(key, String(value))
	}
	const text = params.toString()
	return text ? `?${text}` : ''
}

async function decodeError(response: Response): Promise<ApiCallError> {
	const text = await response.text()
	const body = parseJson(text)
	if (isRecord(body) && typeof body.error === 'string' && typeof body.description === 'string') {
		// An unknown code is passed on as is; Rust reads it as `unknown`.
		return new ApiCallError(response.status, body.error as ErrorCode, body.description)
	}
	return new ApiCallError(response.status, 'unknown', text || `HTTP ${response.status}`)
}

function parseJson(text: string): unknown {
	try {
		return JSON.parse(text)
	} catch {
		return undefined
	}
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null
}
