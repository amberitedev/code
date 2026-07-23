import { describe, expect, it, vi } from 'vitest'

import type { PlatformAdapter } from '../adapter'
import { ConvexApiClient } from '../convex-api'
import { AuthError, ConvexError } from '../errors'

function jsonResponse(value: unknown): Response {
	return new Response(JSON.stringify({ status: 'success', value }), {
		status: 200,
		headers: { 'Content-Type': 'application/json' },
	})
}

function adapter(fetchFn: ReturnType<typeof vi.fn>, jwt = 'real-session-token'): PlatformAdapter {
	return {
		fetchFn: fetchFn as unknown as typeof fetch,
		convexUrl: 'https://test.convex.cloud',
		getCoreUrl: async () => null,
		getCurrentJwt: async () => jwt,
		openExternalAuth: vi.fn(),
	}
}

async function lastBody(fetchFn: ReturnType<typeof vi.fn>) {
	const init = fetchFn.mock.calls.at(-1)?.[1] as RequestInit
	return JSON.parse(String(init.body)) as {
		path: string
		args: Record<string, unknown>
		format: string
	}
}

describe('ConvexApiClient', () => {
	it('sends the real session JWT to Convex calls without injecting an acting user', async () => {
		const fetchFn = vi.fn(async () => jsonResponse(null))
		const client = new ConvexApiClient(adapter(fetchFn))

		await client.friendsList()

		expect(fetchFn).toHaveBeenCalledWith(
			'https://test.convex.cloud/api/query',
			expect.objectContaining({
				method: 'POST',
				headers: expect.objectContaining({
					Authorization: 'Bearer real-session-token',
				}),
			}),
		)
		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'friends:friendsList',
			args: {},
			format: 'json',
		})
	})

	it('can call public auth actions without an expired session JWT', async () => {
		const fetchFn = vi.fn(async () => jsonResponse({ tokens: null }))
		const client = new ConvexApiClient(adapter(fetchFn, 'expired-session-token'))

		await client.rawAction('auth:signIn', { provider: 'minecraft-token' }, false)

		expect(fetchFn).toHaveBeenCalledWith(
			'https://test.convex.cloud/api/action',
			expect.objectContaining({
				headers: expect.not.objectContaining({ Authorization: expect.anything() }),
			}),
		)
	})

	it('routes friend request actions to the Amberite social backend', async () => {
		const fetchFn = vi.fn(async () => jsonResponse({ requestId: 'req-1', status: 'pending' }))
		const client = new ConvexApiClient(adapter(fetchFn))

		const result = await client.sendFriendRequest({ targetUserId: 'user-2' })

		expect(result).toEqual({ requestId: 'req-1', status: 'pending' })
		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'friends:sendFriendRequest',
			args: { targetUserId: 'user-2' },
		})
	})

	it('searches for users with a two-character query', async () => {
		const fetchFn = vi.fn(async () => jsonResponse([]))
		const client = new ConvexApiClient(adapter(fetchFn))

		await client.searchUsers('th')

		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'friends:searchUsers',
			args: { query: 'th' },
		})
	})

	it('routes profile and core list reads to the authenticated backend', async () => {
		const fetchFn = vi.fn(async () => jsonResponse(null))
		const client = new ConvexApiClient(adapter(fetchFn))

		await client.getProfile('amber')
		await client.listLinkedCoreList()

		expect(fetchFn).toHaveBeenCalledTimes(2)
		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'coreList:listLinkedForCurrent',
			args: {},
		})
	})

	it('routes profile views to the sanitized Convex profile view query', async () => {
		const fetchFn = vi.fn(async () => jsonResponse(null))
		const client = new ConvexApiClient(adapter(fetchFn))

		await client.getProfileView('amber')

		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'profiles:view',
			args: { idOrUsername: 'amber' },
		})
	})

	it('routes profile updates and Modrinth linking through redacted contracts', async () => {
		const fetchFn = vi.fn(async () => jsonResponse(null))
		const client = new ConvexApiClient(adapter(fetchFn))

		await client.updateCurrentProfile({ displayName: 'Amber', bio: 'hello' })
		await client.linkedModrinthAccount()
		await client.disconnectModrinthAccount()

		expect(fetchFn).toHaveBeenCalledTimes(3)
		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'modrinth:disconnectCurrent',
			args: {},
		})
	})

	it('claims and acknowledges pending friend request notifications', async () => {
		const fetchFn = vi.fn(async () => jsonResponse([]))
		const client = new ConvexApiClient(adapter(fetchFn))

		await client.claimFriendRequestNotifications()
		await client.acknowledgeFriendRequestNotification('req-1')

		expect(fetchFn).toHaveBeenCalledTimes(2)
		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'friends:acknowledgeFriendRequestNotification',
			args: { requestId: 'req-1' },
		})
	})

	it('routes friend response and removal actions to Convex mutations', async () => {
		const fetchFn = vi.fn(async () => jsonResponse(null))
		const client = new ConvexApiClient(adapter(fetchFn))

		await client.respondFriendRequest('req-1', true)
		await client.cancelFriendRequest('req-2')
		await client.removeFriend('user-2')

		expect(fetchFn).toHaveBeenCalledTimes(3)
		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'friends:removeFriend',
			args: { userId: 'user-2' },
		})
	})

	it('refreshes once and retries one authenticated request after a 401', async () => {
		let jwt = 'expired'
		const fetchFn = vi.fn(async () =>
			jwt === 'expired'
				? new Response(JSON.stringify({ errorMessage: 'not authenticated' }), { status: 401 })
				: jsonResponse(null),
		)
		const testAdapter = adapter(fetchFn)
		testAdapter.getCurrentJwt = async () => jwt
		testAdapter.setCurrentJwt = async (value) => {
			jwt = value ?? ''
		}
		testAdapter.refreshAmberiteSession = vi.fn(async () => ({ accessToken: 'fresh' }))

		await new ConvexApiClient(testAdapter).friendsList()
		expect(fetchFn).toHaveBeenCalledTimes(2)
		expect(testAdapter.refreshAmberiteSession).toHaveBeenCalledTimes(1)
	})

	it('preserves typed Convex and terminal auth failures', async () => {
		const rejected = (status: number, errorMessage: string) =>
			new Response(JSON.stringify({ errorMessage }), {
				status,
				headers: { 'Content-Type': 'application/json' },
			})
		await expect(
			new ConvexApiClient(
				adapter(vi.fn(async () => rejected(401, 'session expired'))),
			).friendsList(),
		).rejects.toMatchObject({
			code: 'expired_session',
			recovery: 'clear_session',
		} satisfies Partial<AuthError>)
		await expect(
			new ConvexApiClient(
				adapter(vi.fn(async () => rejected(500, 'database failed'))),
			).friendsList(),
		).rejects.toBeInstanceOf(ConvexError)
	})
})
