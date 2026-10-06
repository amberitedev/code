import { describe, expect, it } from 'vite-plus/test'
import { AuthFeature } from '../packages/api-client/src/features/auth'
import { SelfHostedFeature } from '../packages/api-client/src/features/self-hosted'
import { GenericModrinthClient } from '../packages/api-client/src/platform/generic'
import type { RequestContext, RequestOptions } from '../packages/api-client/src/types/request'

async function prepared(path: string, options: Partial<RequestOptions> = {}) {
	let preparedRequest: RequestContext | undefined
	const client = new GenericModrinthClient({
		features: [
			new SelfHostedFeature({
				baseUrl: 'http://127.0.0.1:8787',
				token: async () => 'local-session',
			}),
			new AuthFeature({ token: 'local-session' }),
		],
		hooks: {
			onRequest(context) {
				preparedRequest = context
				throw new Error('Request captured before network')
			},
		},
	})
	await expect(client.request(path, { api: 'labrinth', version: 3, ...options })).rejects.toThrow(
		'Request captured before network',
	)
	if (!preparedRequest) throw new Error('Client did not prepare a request')
	return preparedRequest
}

describe('self-hosted account routing through the complete client chain', () => {
	it('routes private accounts and shared instances with the current session', async () => {
		const account = await prepared('/friends')
		expect(account.url).toBe('http://127.0.0.1:8787/v3/friends')
		expect(new Headers(account.options.headers).get('authorization')).toBe('Bearer local-session')
		const sharing = await prepared('/instances', { api: 'sharedinstances', version: 1 })
		expect(sharing.url).toBe('http://127.0.0.1:8787/v1/instances')
		const globals = await prepared('/globals', { version: 'internal', skipAuth: true })
		expect(globals.url).toBe('http://127.0.0.1:8787/_internal/globals')
	})
	it('keeps public content and explicit public creator identities on Modrinth without local credentials', async () => {
		for (const [path, options] of [
			['/project/sodium', {}],
			['/user/same-id', { accountSource: 'modrinth' }],
		] as const) {
			const request = await prepared(path, options)
			expect(new URL(request.url).origin).toBe('https://api.modrinth.com')
			expect(new Headers(request.options.headers).has('authorization')).toBe(false)
		}
	})
	it('retains the explicit token used to bootstrap login', async () => {
		const request = await prepared('/user', {
			skipAuth: true,
			headers: { Authorization: 'newly-signed-in-session' },
		})
		expect(new Headers(request.options.headers).get('authorization')).toBe(
			'newly-signed-in-session',
		)
	})
	it('routes account administration to the private backend with private credentials', async () => {
		for (const action of ['lock', 'sessions', 'password-reset', '2fa']) {
			const request = await prepared(`/admin/user/private-id/${action}`, { version: 'internal' })
			expect(request.url).toBe(`http://127.0.0.1:8787/_internal/admin/user/private-id/${action}`)
			expect(new Headers(request.options.headers).get('authorization')).toBe('Bearer local-session')
		}
		for (const path of ['/user_email', '/user_discord']) {
			const request = await prepared(path)
			expect(new URL(request.url).origin).toBe('http://127.0.0.1:8787')
		}
	})
	it('keeps anonymous login anonymous', async () => {
		const request = await prepared('/auth/login', { method: 'POST', skipAuth: true })
		expect(new Headers(request.options.headers).has('authorization')).toBe(false)
	})
	it('rejects report attachments before network while preserving public content images', async () => {
		let reachedNetwork = false
		const client = new GenericModrinthClient({
			features: [
				new SelfHostedFeature({
					baseUrl: 'http://127.0.0.1:8787',
					token: async () => 'local-session',
				}),
			],
			hooks: {
				onRequest() {
					reachedNetwork = true
					throw new Error('Network must not be reached')
				},
			},
		})
		await expect(
			client.labrinth.images_v3.uploadImage(new Blob(['private screenshot']), 'png', {
				context: 'report',
			}).promise,
		).rejects.toThrow('Report attachments are not supported')
		await expect(
			client.request('/image?context=report', { api: 'labrinth', version: 3, method: 'POST' }),
		).rejects.toThrow('Report attachments are not supported')
		expect(reachedNetwork).toBe(false)
		const publicImage = await prepared('/image', {
			method: 'POST',
			params: { context: 'project', project_id: 'public-project' },
		})
		expect(new URL(publicImage.url).origin).toBe('https://api.modrinth.com')
		expect(new Headers(publicImage.options.headers).has('authorization')).toBe(false)
	})
})
