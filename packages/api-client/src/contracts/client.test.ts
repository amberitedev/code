import type { Endpoint } from '@amberite/contracts'
import { describe, expect, it } from 'vite-plus/test'

import { SelfHostedFeature } from '../features/self-hosted'
import { GenericModrinthClient } from '../platform/generic'
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
		expect(client.url(renameFile, { id: 'i', path: 'p' })).toBe(
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
			expect(() => client.url(renameFile, params)).toThrow('is not a valid')
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
		await expect(ignored.client.stream(download, {}, 10)).rejects.toMatchObject({ status: 200 })
		expect(ignored.sent[0].headers.Range).toBe('bytes=10-')

		const resumed = serve(() => new Response('rest', { status: 206 }))
		await expect(resumed.client.stream(download, {}, 10)).resolves.toBeInstanceOf(Response)
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

describe('client.backend', () => {
	it('follows the SelfHostedFeature the client has now', () => {
		const backend = (baseUrl: string) =>
			new SelfHostedFeature({ baseUrl, token: async () => undefined })
		const first = backend('https://one.example')
		const client = new GenericModrinthClient({ features: [first] })
		expect(client.backend.url(deleteCache, null)).toBe('https://one.example/v1/cache')

		client.removeFeature(first).addFeature(backend('https://two.example'))
		expect(client.backend.url(deleteCache, null)).toBe('https://two.example/v1/cache')
	})
})
