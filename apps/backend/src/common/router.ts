import type { Endpoint } from '@amberite/contracts'

import { invalid, json, readBytes, readJsonBody } from './http'
import type { Env } from './types'

type TypesOf<E> =
	E extends Endpoint<infer P, infer Q, infer B, infer R>
		? { params: P; query: Q; body: B; response: R }
		: never

export type BodyOf<E extends Endpoint> = TypesOf<E>['body']

/** A request to `E`, its path params read into their type. */
export interface Context<E extends Endpoint> {
	request: Request
	env: Env
	params: TypesOf<E>['params']
	/** Reads and parses the body. Call it after authenticating, so no caller gets it read unchecked. */
	body(): Promise<BodyOf<E>>
}

/**
 * What a handler answers: the route's response, or for a socket, raw bytes or a stream, the
 * `Response` it built with its own headers.
 */
type Answer<E extends Endpoint> = TypesOf<E>['response'] extends
	| { receive: unknown; send: unknown }
	| Blob
	| Response
	? Response
	: TypesOf<E>['response']

export interface Route {
	method: string
	segments: string[]
	serve(request: Request, env: Env, params: Record<string, string>): Promise<Response>
}

/** Read a body larger than this as too large unless the route allows more. */
const MAX_BODY_BYTES = 65536

/**
 * `handle` as the route of `endpoint`. `parse` checks the untrusted body (parsed JSON, or a `Blob`
 * of raw bytes) and returns it typed; a body that does not fit throws `invalid_input`. A body over
 * `maxBytes` is refused unread.
 */
export function route<E extends Endpoint>(
	endpoint: E,
	parse: (raw: unknown) => BodyOf<E>,
	handle: (context: Context<E>) => Promise<Answer<E>>,
	maxBytes = MAX_BODY_BYTES,
): Route {
	return {
		method: endpoint.method,
		segments: endpoint.path.split('/').slice(1),
		async serve(request, env, params) {
			const answer = await handle({
				request,
				env,
				// A route without `{name}` segments has `()` params, which arrive as `null`.
				params: (Object.keys(params).length ? params : null) as TypesOf<E>['params'],
				body: async () => parse(await readBody(endpoint, request, maxBytes)),
			})
			if (answer instanceof Response) return answer
			if (endpoint.response === 'empty') return new Response(null, { status: 204 })
			return json(answer)
		},
	}
}

async function readBody(endpoint: Endpoint, request: Request, maxBytes: number) {
	if (endpoint.body === 'json') return readJsonBody(request, maxBytes)
	if (endpoint.body === 'raw')
		return new Blob([await readBytes(request, maxBytes)], {
			type: request.headers.get('Content-Type') ?? '',
		})
	return undefined
}

/** For routes without a body. */
export function noBody(): void {}

/** For routes whose body is raw bytes. */
export function rawBody(raw: unknown): Blob {
	return raw instanceof Blob ? raw : invalid('Expected a body')
}

/**
 * The response of the route `path` names, or null when none of `routes` does. `path` is the
 * request's own unless the caller strips a prefix first.
 */
export async function serve(
	routes: Route[],
	request: Request,
	env: Env,
	path = new URL(request.url).pathname,
): Promise<Response | null> {
	const segments = path.split('/').slice(1)
	for (const candidate of routes) {
		if (candidate.method !== request.method) continue
		const params = match(candidate.segments, segments)
		if (params) return candidate.serve(request, env, params)
	}
	return null
}

/** The `{name}` segments of `template` filled from `path`, or null when the two do not match. */
function match(template: string[], path: string[]): Record<string, string> | null {
	if (template.length !== path.length) return null
	const params: Record<string, string> = {}
	for (const [index, segment] of template.entries()) {
		const name = segment.match(/^\{(\w+)\}$/)?.[1]
		if (!name) {
			if (segment !== path[index]) return null
			continue
		}
		let value: string
		try {
			value = decodeURIComponent(path[index])
		} catch {
			return null
		}
		if (!value) return null
		params[name] = value
	}
	return params
}
