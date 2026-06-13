import { describe, expect, it, vi } from 'vitest'

import type { PlatformAdapter } from '../adapter'
import { ConvexApiClient } from '../convex-api'

function jsonResponse(value: unknown): Response {
	return new Response(JSON.stringify({ status: 'success', value }), {
		status: 200,
		headers: { 'Content-Type': 'application/json' },
	})
}

function adapter(fetchFn: ReturnType<typeof vi.fn>, actingUserId?: string): PlatformAdapter {
	return {
		fetchFn: fetchFn as unknown as typeof fetch,
		convexUrl: 'https://test.convex.cloud',
		getCoreUrl: async () => null,
		getCurrentJwt: async () => (actingUserId ? `dev:${actingUserId}` : null),
		getDevActingUserId: () => actingUserId ?? null,
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
	it('injects the dev acting user into Convex calls without sending a fake JWT', async () => {
		const fetchFn = vi.fn(async () => jsonResponse(null))
		const client = new ConvexApiClient(adapter(fetchFn, 'user-1'))

		await client.friendsList()

		expect(fetchFn).toHaveBeenCalledWith(
			'https://test.convex.cloud/api/query',
			expect.objectContaining({
				method: 'POST',
				headers: expect.not.objectContaining({
					Authorization: expect.any(String),
				}),
			}),
		)
		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'friends:friendsList',
			args: { __actAs: 'user-1' },
			format: 'json',
		})
	})

	it('routes friend request actions to the Amberite social backend', async () => {
		const fetchFn = vi.fn(async () => jsonResponse({ requestId: 'req-1', status: 'pending' }))
		const client = new ConvexApiClient(adapter(fetchFn, 'user-1'))

		const result = await client.sendFriendRequest({ targetUserId: 'user-2' })

		expect(result).toEqual({ requestId: 'req-1', status: 'pending' })
		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'friends:sendFriendRequest',
			args: { targetUserId: 'user-2', __actAs: 'user-1' },
		})
	})

	it('routes friend response and removal actions to Convex mutations', async () => {
		const fetchFn = vi.fn(async () => jsonResponse(null))
		const client = new ConvexApiClient(adapter(fetchFn, 'user-1'))

		await client.respondFriendRequest('req-1', true)
		await client.cancelFriendRequest('req-2')
		await client.removeFriend('user-2')

		expect(fetchFn).toHaveBeenCalledTimes(3)
		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'friends:removeFriend',
			args: { userId: 'user-2', __actAs: 'user-1' },
		})
	})

	it('routes social presence heartbeat to Convex', async () => {
		const fetchFn = vi.fn(async () =>
			jsonResponse({ online: true, status: null, lastSeenAt: 123 }),
		)
		const client = new ConvexApiClient(adapter(fetchFn, 'user-1'))

		const result = await client.heartbeat()

		expect(result).toEqual({ online: true, status: null, lastSeenAt: 123 })
		expect(await lastBody(fetchFn)).toMatchObject({
			path: 'friends:heartbeat',
			args: { __actAs: 'user-1' },
		})
	})
})
