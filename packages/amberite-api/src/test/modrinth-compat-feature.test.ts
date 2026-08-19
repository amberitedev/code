import type { RequestContext } from '@modrinth/api-client'
import { describe, expect, it, vi } from 'vitest'
import type { PlatformAdapter } from '../adapter'
import type { ConvexApiClient } from '../convex-api'
import { AmberiteModrinthCompatFeature } from '../modrinth-compat-feature'

function adapter(fetchFn = vi.fn<typeof fetch>()): PlatformAdapter {
	return {
		fetchFn,
		convexUrl: 'https://amberite.convex.cloud',
		convexSiteUrl: 'https://amberite.convex.site',
		getCoreUrl: async () => null,
		getCurrentJwt: async () => 'amberite-jwt',
		openExternalAuth: () => undefined,
	}
}

function context(
	path: string,
	api: 'labrinth' | 'sharedinstances',
	version: number,
	method: 'GET' | 'POST' | 'DELETE' = 'GET',
): RequestContext {
	return {
		url: `https://upstream.invalid/v${version}${path}`,
		path,
		options: { api, version, method },
		attempt: 1,
		startTime: 0,
	}
}

function transport() {
	return {
		rawQuery: vi.fn(async () => null) as ConvexApiClient['rawQuery'],
		rawMutation: vi.fn(async () => null) as ConvexApiClient['rawMutation'],
	}
}

describe('AmberiteModrinthCompatFeature', () => {
	it('leaves Modrinth content routes alone', () => {
		const feature = new AmberiteModrinthCompatFeature({ adapter: adapter(), client: transport() })
		expect(feature.shouldApply(context('/project/sodium', 'labrinth', 3))).toBe(false)
	})

	it('maps Labrinth friend mutations to Convex', async () => {
		const client = transport()
		const feature = new AmberiteModrinthCompatFeature({ adapter: adapter(), client })
		await feature.execute(
			async () => {
				throw new Error('upstream should not run')
			},
			context('/friend/Steve', 'labrinth', 3, 'POST'),
		)
		expect(client.rawMutation).toHaveBeenCalledWith('socialCompat:addFriend', {
			idOrUsername: 'Steve',
		})
	})

	it('sends sharing traffic to Convex with the Amberite session', async () => {
		const fetchFn = vi.fn<typeof fetch>(async () => Response.json({ name: 'Friends', icon: null }))
		const feature = new AmberiteModrinthCompatFeature({
			adapter: adapter(fetchFn),
			client: transport(),
		})
		await feature.execute(
			async () => {
				throw new Error('upstream should not run')
			},
			context('/instances/client-id', 'sharedinstances', 1),
		)
		const [url, init] = fetchFn.mock.calls[0]
		expect(url).toBe('https://amberite.convex.site/v1/instances/client-id')
		expect(new Headers(init?.headers).get('authorization')).toBe('Bearer amberite-jwt')
	})
})
