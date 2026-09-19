import type { IncomingMessage, ServerResponse } from 'node:http'

import type { Labrinth } from '../../api-client/src/modules/labrinth/types.ts'

import { DEFAULT_PREFERENCES, PERSONAS, v2UserFor } from './fixtures.ts'
import { empty, HttpError, json, objectBody, readJson } from './http.ts'
import type { StateStore } from './state.ts'
import type { Persona } from './types.ts'

type IdentityContext = {
	request: IncomingMessage
	response: ServerResponse
	url: URL
	persona: Persona | null
	store: StateStore
	publicCatalog: boolean
}

export async function handleIdentity(context: IdentityContext): Promise<boolean> {
	const { request, response, url, persona, store } = context
	const method = request.method ?? 'GET'
	const path = url.pathname

	if (method === 'GET' && (path === '/auth/sign-in' || path === '/auth/sign-up')) {
		serveSignIn(response, url)
		return true
	}

	if (method === 'POST' && path === '/v2/session/refresh') {
		const authenticated = requirePersona(persona)
		json(response, 200, { session: authenticated.token })
		return true
	}

	if (method === 'GET' && (path === '/v2/user' || path === '/v3/user')) {
		const authenticated = requirePersona(persona)
		json(response, 200, path.startsWith('/v2/') ? v2UserFor(authenticated) : authenticated.user)
		return true
	}

	const userMatch = path.match(/^\/v(2|3)\/user\/([^/]+)$/)
	if (method === 'GET' && userMatch) {
		const user = findPersonaByIdOrName(decodeURIComponent(userMatch[2]!))
		if (!user) throw new HttpError(404, 'Fixture user was not found')
		json(response, 200, userMatch[1] === '2' ? v2UserFor(user) : user.user)
		return true
	}

	const preferencesMatch = path.match(/^\/v3\/user\/([^/]+)\/preferences$/)
	if (preferencesMatch && method === 'GET') {
		const authenticated = requirePersona(persona)
		const userId = decodeURIComponent(preferencesMatch[1]!)
		if (userId !== authenticated.user.id && userId !== authenticated.user.username) {
			throw new HttpError(403, 'Fixture personas can only access their own preferences')
		}
		json(response, 200, store.read().preferences[authenticated.user.id] ?? DEFAULT_PREFERENCES)
		return true
	}
	if (preferencesMatch && method === 'PATCH') {
		const authenticated = requirePersona(persona)
		const userId = decodeURIComponent(preferencesMatch[1]!)
		if (userId !== authenticated.user.id && userId !== authenticated.user.username) {
			throw new HttpError(403, 'Fixture personas can only update their own preferences')
		}
		const patch = objectBody(await readJson(request)) as Labrinth.Users.v3.PartialUserPreferences
		const preferences = await store.mutate((state) => {
			const current =
				state.preferences[authenticated.user.id] ?? structuredClone(DEFAULT_PREFERENCES)
			state.preferences[authenticated.user.id] = {
				appearance: { ...current.appearance, ...patch.appearance },
				behavior: { ...current.behavior, ...patch.behavior },
				localization: { ...current.localization, ...patch.localization },
				layouts: { ...current.layouts, ...patch.layouts },
				sidebars: { ...current.sidebars, ...patch.sidebars },
				social: { ...current.social, ...patch.social },
			}
			return state.preferences[authenticated.user.id]
		})
		json(response, 200, preferences)
		return true
	}

	if (method === 'GET' && path === '/v2/users') {
		const ids = parseJsonQueryArray(url.searchParams.get('ids'))
		json(
			response,
			200,
			Object.values(PERSONAS)
				.filter((candidate) => ids.length === 0 || ids.includes(candidate.user.id))
				.map(v2UserFor),
		)
		return true
	}

	if (method === 'GET' && path === '/v3/users/search') {
		const query = url.searchParams.get('query')?.toLowerCase() ?? ''
		json(
			response,
			200,
			Object.values(PERSONAS)
				.filter((candidate) => candidate.user.username.toLowerCase().includes(query))
				.map(
					(candidate) =>
						({
							id: candidate.user.id,
							username: candidate.user.username,
							avatar_url: candidate.user.avatar_url ?? null,
						}) satisfies Labrinth.Users.v3.SearchUser,
				),
		)
		return true
	}

	if (method === 'GET' && path.match(/^\/v2\/user\/[^/]+\/(notifications|friends)$/)) {
		json(response, 200, [])
		return true
	}
	if (method === 'GET' && path === '/v2/notifications') {
		json(response, 200, [])
		return true
	}
	if (method === 'GET' && path === '/v3/friends') {
		requirePersona(persona)
		json(response, 200, [])
		return true
	}
	if (['PATCH', 'DELETE'].includes(method) && path.match(/^\/v2\/notifications?(\/[^/]+)?$/)) {
		empty(response)
		return true
	}

	if (
		method === 'GET' &&
		(path === '/_internal/billing/subscriptions' || path === '/_internal/billing/payments')
	) {
		json(response, 200, [])
		return true
	}

	return false
}

export async function proxyPublicCatalog(
	request: IncomingMessage,
	response: ServerResponse,
	url: URL,
): Promise<boolean> {
	const method = request.method ?? 'GET'
	if (!['GET', 'HEAD'].includes(method) || !url.pathname.match(/^\/v[23]\//)) return false

	const headers = new Headers()
	const accept = request.headers.accept
	if (accept) headers.set('accept', accept)
	headers.set('user-agent', 'modrinth-proxy-lab/0.0.0 (local exploration tool)')

	const upstream = await fetch(`https://api.modrinth.com${url.pathname}${url.search}`, {
		method,
		headers,
		redirect: 'manual',
	})
	const body = method === 'HEAD' ? null : Buffer.from(await upstream.arrayBuffer())
	const responseHeaders: Record<string, string | number> = {
		'access-control-allow-origin': 'http://localhost:1420',
		'cache-control': upstream.headers.get('cache-control') ?? 'no-store',
		'content-type': upstream.headers.get('content-type') ?? 'application/octet-stream',
		'x-proxy-lab-upstream': 'read-only-public-catalog',
	}
	if (body) responseHeaders['content-length'] = body.length
	response.writeHead(upstream.status, responseHeaders)
	response.end(body)
	return true
}

function serveSignIn(response: ServerResponse, url: URL): void {
	const port = url.searchParams.get('port')
	const ipver = url.searchParams.get('ipver') === '6' ? '6' : '4'
	if (!port || !/^\d+$/.test(port))
		throw new HttpError(400, 'The App login callback port is missing')
	const callbackHost = ipver === '6' ? '[::1]' : '127.0.0.1'
	const buttons = Object.values(PERSONAS)
		.map(
			(persona) =>
				`<a href="http://${callbackHost}:${port}/?code=${encodeURIComponent(persona.token)}">Continue as ${escapeHtml(persona.user.username)}</a>`,
		)
		.join('\n')
	const body = `<!doctype html>
<html lang="en">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Modrinth proxy lab sign-in</title>
<style>
html { color-scheme: dark; background: #000; color: #fff; font: 16px system-ui, sans-serif; }
body { max-width: 42rem; margin: 4rem auto; padding: 0 1.5rem; }
h1 { font-size: 1.5rem; }
p { color: #bbb; line-height: 1.5; }
a { display: block; margin: .75rem 0; padding: .8rem 1rem; border: 1px solid #555; color: #fff; text-decoration: none; }
a:hover { border-color: #fff; }
</style>
<h1>Local proxy-lab persona</h1>
<p>These accounts and tokens exist only in the local lab. Pick one to return to the isolated App.</p>
${buttons}
</html>`
	response.writeHead(200, {
		'cache-control': 'no-store',
		'content-length': Buffer.byteLength(body),
		'content-type': 'text/html; charset=utf-8',
	})
	response.end(body)
}

function requirePersona(persona: Persona | null): Persona {
	if (!persona) throw new HttpError(401, 'Use a proxy-lab fixture token')
	return persona
}

function findPersonaByIdOrName(value: string): Persona | undefined {
	return Object.values(PERSONAS).find(
		(persona) => persona.user.id === value || persona.user.username === value,
	)
}

function parseJsonQueryArray(value: string | null): string[] {
	if (!value) return []
	try {
		const parsed: unknown = JSON.parse(value)
		return Array.isArray(parsed)
			? parsed.filter((item): item is string => typeof item === 'string')
			: []
	} catch {
		return []
	}
}

function escapeHtml(value: string): string {
	return value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')
}
