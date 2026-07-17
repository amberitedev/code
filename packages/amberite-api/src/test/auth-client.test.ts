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

function profile(username = 'amber') {
	return {
		id: 'user-1',
		userId: 'user-1',
		username,
		minecraftUuid: '12345678-1234-1234-1234-123456789abc',
		verifiedMinecraftHandle: username,
		name: 'Amber',
		avatar_url: null,
		bio: 'hello',
		created: '2026-01-01T00:00:00.000Z',
		email: null,
		email_verified: false,
		auth_providers: ['minecraft'],
		has_password: false,
		has_totp: false,
	}
}

function createAdapter(fetchFn: ReturnType<typeof vi.fn>): PlatformAdapter {
	let tokens = { token: 'old-token', refreshToken: 'old-refresh' }
	return {
		fetchFn: fetchFn as unknown as typeof fetch,
		convexUrl: 'https://test.convex.cloud',
		getCoreUrl: async () => null,
		getCurrentJwt: async () => tokens.token,
		setCurrentJwt: async (token) => {
			tokens = { ...tokens, token: token ?? '' }
		},
		readAmberiteSession: async () => tokens,
		writeAmberiteSession: async (next) => {
			tokens = next
		},
		clearAmberiteSession: async () => {
			tokens = { token: '', refreshToken: '' }
		},
		openExternalAuth: vi.fn(),
	}
}

async function requestBodies(fetchFn: ReturnType<typeof vi.fn>) {
	return fetchFn.mock.calls.map((call) => JSON.parse(String((call[1] as RequestInit).body)))
}

describe('ConvexAmberiteAuthClient', () => {
	it('refreshes stored sessions and maps verified Minecraft identity', async () => {
		const fetchFn = vi.fn(async (_url: string, init: RequestInit) => {
			const body = JSON.parse(String(init.body))
			return body.path === 'auth:signIn'
				? success({ tokens: { token: 'new-token', refreshToken: 'new-refresh' } })
				: success(profile())
		})
		const adapter = createAdapter(fetchFn)
		const session = await new ConvexAmberiteAuthClient({ adapter }).restoreSession()

		expect(session?.user).toMatchObject({
			username: 'amber',
			verifiedMinecraftHandle: 'amber',
			minecraftUuid: '12345678-1234-1234-1234-123456789abc',
		})
		expect(await adapterSessionStorage(adapter).read()).toEqual({
			token: 'new-token',
			refreshToken: 'new-refresh',
		})
	})

	it('single-flights concurrent refreshes', async () => {
		let release!: () => void
		const pending = new Promise<void>((resolve) => (release = resolve))
		const fetchFn = vi.fn(async (_url: string, init: RequestInit) => {
			const body = JSON.parse(String(init.body))
			if (body.path === 'auth:signIn') {
				await pending
				return success({ tokens: { token: 'new-token', refreshToken: 'new-refresh' } })
			}
			return success(profile())
		})
		const client = new ConvexAmberiteAuthClient({ adapter: createAdapter(fetchFn) })
		const first = client.refreshSession()
		const second = client.refreshSession()
		release()
		await Promise.all([first, second])

		const authCalls = (await requestBodies(fetchFn)).filter((body) => body.path === 'auth:signIn')
		expect(authCalls).toHaveLength(1)
	})

	it('passes expected UUID through Minecraft token sign-in', async () => {
		const fetchFn = vi.fn(async (_url: string, init: RequestInit) => {
			const body = JSON.parse(String(init.body))
			return body.path === 'auth:signIn'
				? success({ tokens: { token: 'token', refreshToken: 'refresh' } })
				: success(profile())
		})
		const client = new ConvexAmberiteAuthClient({ adapter: createAdapter(fetchFn) })
		await client.signInWithMinecraftToken({
			minecraftAccessToken: 'minecraft-token',
			expectedMinecraftUuid: '12345678-1234-1234-1234-123456789abc',
		})

		expect((await requestBodies(fetchFn))[0]).toMatchObject({
			args: {
				provider: 'minecraft-token',
				params: {
					minecraftAccessToken: 'minecraft-token',
					expectedMinecraftUuid: '12345678-1234-1234-1234-123456789abc',
				},
			},
		})
	})

	it('uses the native platform capability without exposing a refresh token', async () => {
		const fetchFn = vi.fn(async () => success(profile()))
		const adapter = createAdapter(fetchFn)
		adapter.signInWithMinecraft = vi.fn(async () => ({
			accessToken: 'native-access',
			user: profile(),
		}))
		const session = await new ConvexAmberiteAuthClient({ adapter }).signInWithMinecraft({
			mode: 'continue',
			expectedMinecraftUuid: '12345678-1234-1234-1234-123456789abc',
		})

		expect(session.tokens).toEqual({ token: 'native-access', refreshToken: '' })
		expect(adapter.signInWithMinecraft).toHaveBeenCalledWith({
			mode: 'continue',
			expectedMinecraftUuid: '12345678-1234-1234-1234-123456789abc',
		})
		expect(fetchFn).not.toHaveBeenCalled()
	})

	it('clears local tokens after logout even when remote revocation fails', async () => {
		const adapter = createAdapter(vi.fn(async () => new Response('nope', { status: 401 })))
		await new ConvexAmberiteAuthClient({ adapter }).logOut()
		expect(await adapterSessionStorage(adapter).read()).toEqual({ token: '', refreshToken: '' })
	})
})

describe('MockAmberiteAuthClient', () => {
	it('keeps the verified handle immutable while editing display data', async () => {
		const client = new MockAmberiteAuthClient()
		const updated = await client.updateCurrentProfile({ displayName: 'Local', bio: 'dev' })
		expect(updated).toMatchObject({ username: 'devuser', name: 'Local', bio: 'dev' })
	})
})
