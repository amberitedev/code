import type { Endpoint } from '@amberite/contracts'
import { describe, expect, it } from 'vite-plus/test'

import { ContractClient } from './client'
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
		base: async () => 'http://127.0.0.1:5000/secret/',
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
		expect(client.url(renameFile, { id: 'i', path: 'p' })).toBe(
			'http://127.0.0.1:5000/secret/v1/instances/i/files/p',
		)
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

	it('throws an unknown error when the body is not an ApiError', async () => {
		const { client } = serve(() => new Response('Bad Gateway', { status: 502 }))

		await expect(client.call(deleteCache)).rejects.toMatchObject({
			status: 502,
			code: 'unknown',
			description: 'Bad Gateway',
		})
	})
})
