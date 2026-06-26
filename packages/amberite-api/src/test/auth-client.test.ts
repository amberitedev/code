import { describe, expect, it, vi } from 'vitest'

import type { PlatformAdapter } from '../adapter'
import { ConvexAmberiteAuthClient } from '../auth-client'
import { MockAmberiteAuthClient } from '../mock-client'
import { adapterSessionStorage } from '../session'

function success(value: unknown): Response {
	return new Response(JSON.stringify({ status: 'success', value }), {
		status: 200,
		headers: { 'Content-Type': 'application/json' },
	})
}

function createAdapter(fetchFn: ReturnType<typeof vi.fn>): PlatformAdapter {
	let token: string | null = 'old-token'
	let refreshToken: string | null = 'old-refresh'
	return {
		fetchFn: fetchFn as unknown as typeof fetch,
		convexUrl: 'https://test.convex.cloud',
		getCoreUrl: async () => null,
		getCurrentJwt: async () => token,
		setCurrentJwt: async (next) => {
			token = next
		},
		getCurrentRefreshToken: async () => refreshToken,
		setCurrentRefreshToken: async (next) => {
			refreshToken = next
		},
		openExternalAuth: vi.fn(),
	}
}

async function requestBodies(fetchFn: ReturnType<typeof vi.fn>) {
	return fetchFn.mock.calls.map((call) => JSON.parse(String((call[1] as RequestInit).body)))
}

describe('ConvexAmberiteAuthClient', () => {
	it('refreshes stored sessions and maps the current profile to the compatibility user shape', async () => {
		const fetchFn = vi.fn(async (_url: string, init: RequestInit) => {
			const body = JSON.parse(String(init.body))
			if (body.path === 'auth:signIn') {
				return success({ tokens: { token: 'new-token', refreshToken: 'new-refresh' } })
			}
			return success({
				id: 'user-1',
				userId: 'user-1',
				username: 'amber',
				name: 'Amber',
				avatar_url: null,
				bio: 'hello',
				created: '2026-01-01T00:00:00.000Z',
				email: 'amber@example.com',
				email_verified: true,
				auth_providers: ['github'],
				has_password: true,
				has_totp: false,
			})
		})
		const adapter = createAdapter(fetchFn)
		const client = new ConvexAmberiteAuthClient({ adapter })

		const session = await client.restoreSession()

		expect(session?.tokens).toEqual({ token: 'new-token', refreshToken: 'new-refresh' })
		expect(session?.user).toMatchObject({
			id: 'user-1',
			username: 'amber',
			email: 'amber@example.com',
			auth_providers: ['github'],
			has_password: true,
		})
		expect(await adapterSessionStorage(adapter).read()).toEqual({
			token: 'new-token',
			refreshToken: 'new-refresh',
		})
	})

	it('routes password and Modrinth-token sign-in through explicit Amberite auth providers', async () => {
		const fetchFn = vi.fn(async (_url: string, init: RequestInit) => {
			const body = JSON.parse(String(init.body))
			if (body.path === 'auth:signIn') {
				return success({ tokens: { token: 'token', refreshToken: 'refresh' } })
			}
			return success({
				id: 'user-1',
				userId: 'user-1',
				username: 'amber',
				created: '2026-01-01T00:00:00.000Z',
			})
		})
		const client = new ConvexAmberiteAuthClient({ adapter: createAdapter(fetchFn) })

		await client.signInWithPassword({ login: 'amber@example.com', password: 'password123' })
		await client.signInWithModrinthToken('mra_token')

		const bodies = await requestBodies(fetchFn)
		expect(bodies[0]).toMatchObject({
			path: 'auth:signIn',
			args: { provider: 'web-password', params: { flow: 'signIn' } },
		})
		expect(bodies[2]).toMatchObject({
			path: 'auth:signIn',
			args: { provider: 'modrinth-token', params: { modrinthToken: 'mra_token' } },
		})
	})

	it('clears local tokens after logout even when the backend rejects the request', async () => {
		const fetchFn = vi.fn(async () => new Response('nope', { status: 401 }))
		const adapter = createAdapter(fetchFn)
		const client = new ConvexAmberiteAuthClient({ adapter })

		await client.logOut()

		expect(await adapterSessionStorage(adapter).read()).toBeNull()
	})
})

describe('MockAmberiteAuthClient', () => {
	it('provides a typed local client without patching fetch', async () => {
		const client = new MockAmberiteAuthClient()

		const session = await client.restoreSession()
		const updated = await client.updateCurrentProfile({ username: 'local', bio: 'dev' })

		expect(session?.user.username).toBe('devuser')
		expect(updated).toMatchObject({ username: 'local', bio: 'dev' })
		await client.logOut()
		expect(await client.currentUser()).toBeNull()
	})
})
