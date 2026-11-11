import type { Endpoint } from '@amberite/contracts'

import { json, readJsonBody } from './http'
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

/** What a handler answers: the route's response, or the `101` of a socket it upgraded to. */
type Answer<E extends Endpoint> = TypesOf<E>['response'] extends { receive: unknown; send: unknown }
	? Response
	: TypesOf<E>['response']

export interface Route {
	method: string
	segments: string[]
	serve(request: Request, env: Env, params: Record<string, string>): Promise<Response>
}

/** Read a JSON body larger than this as too large unless the route allows more. */
const MAX_JSON_BYTES = 65536

/**
 * `handle` as the route of `endpoint`. `parse` checks the untrusted body and returns it typed; a
 * body that does not fit throws `invalid_input`. A JSON body over `maxBytes` is refused unread.
 */
export function route<E extends Endpoint>(
	endpoint: E,
	parse: (raw: unknown) => BodyOf<E>,
	handle: (context: Context<E>) => Promise<Answer<E>>,
	maxBytes = MAX_JSON_BYTES,
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
				body: async () =>
					parse(endpoint.body === 'json' ? await readJsonBody(request, maxBytes) : undefined),
			})
			if (answer instanceof Response) return answer
			if (endpoint.response === 'empty') return new Response(null, { status: 204 })
			return json(answer)
		},
	}
}

/** For routes without a body. */
export function noBody(): void {}

/** The response of the route `request` names, or null when none of `routes` does. */
export async function serve(routes: Route[], request: Request, env: Env): Promise<Response | null> {
	const segments = new URL(request.url).pathname.split('/').slice(1)
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
