import type { BodyKind, Endpoint, ErrorCode } from '@amberite/contracts'

import { ApiCallError } from './errors'

/** Where a `ContractClient` sends its calls, and what each call carries. */
export interface ContractTransport {
	/** A string when it is known up front, so `url` works before any call. */
	base: string | (() => Promise<string>)
	/** Given the route, so a transport can leave out credentials or attach a fresh ticket. */
	headers(endpoint: Endpoint): Promise<Record<string, string>>
	fetch: typeof fetch
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

type Args<E extends Endpoint> = {} extends InputOf<E> ? [input?: InputOf<E>] : [input: InputOf<E>]

type Input = { params?: unknown; query?: unknown; body?: unknown }

/** Calls the routes of one server, typed by their generated `Endpoint`s. */
export class ContractClient {
	private knownBase: string | undefined

	constructor(private readonly transport: ContractTransport) {
		if (typeof transport.base === 'string') this.knownBase = trimBase(transport.base)
	}

	/** Throws `ApiCallError` when the route answers with an error. */
	async call<E extends Endpoint>(endpoint: E, ...[input]: Args<E>): Promise<ResponseOf<E>> {
		const response = await this.send(endpoint, input ?? {})
		return (await read(response, endpoint.response)) as ResponseOf<E>
	}

	/** The route's URL, for an `<img>` or a link. An async base is known once a call resolved it. */
	url<E extends Endpoint>(endpoint: E, params: ParamsOf<E>): string {
		if (this.knownBase === undefined) throw new Error('No call has resolved the base URL yet')
		return this.knownBase + fillPath(endpoint.path, params)
	}

	/** The response, to read as it arrives; `from` resumes a download at that byte. */
	async stream<E extends Endpoint>(
		endpoint: E,
		input: InputOf<E>,
		from?: number,
	): Promise<Response> {
		return this.send(endpoint, input, from)
	}

	private async send(endpoint: Endpoint, input: Input, from?: number): Promise<Response> {
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
		if (from) headers.Range = `bytes=${from}-`
		const url = baseUrl + fillPath(endpoint.path, input.params) + search(input.query)
		const fetch = this.transport.fetch
		const response = await fetch(url, { method: endpoint.method, headers, body })
		if (!response.ok) throw await decodeError(response)
		if (from && response.status !== 206) {
			await response.body?.cancel()
			throw new ApiCallError(response.status, 'unknown', 'The server ignored the requested range')
		}
		return response
	}
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
