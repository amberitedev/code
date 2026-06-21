import { describe, expect, it, vi } from 'vitest'

import type { PlatformAdapter } from '../adapter'
import { ConvexApiClient } from '../convex-api'

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

	it('routes social presence heartbeat to Convex', async () => {
		const fetchFn = vi.fn(async () => jsonResponse({ online: true, status: null, lastSeenAt: 123 }))
		const client = new ConvexApiClient(adapter(fetchFn))

		const result = await client.heartbeat()

		expect(result).toEqual({ online: true, status: null, lastSeenAt: 123 })
		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'friends:heartbeat',
			args: {},
		})
	})
})
