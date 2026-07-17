import { describe, expect, it, vi } from 'vitest'

import type { PlatformAdapter } from '../adapter'
import { adapterSessionStorage } from '../session'

function baseAdapter(): PlatformAdapter {
	return {
		fetchFn: vi.fn() as unknown as typeof fetch,
		convexUrl: 'https://test.convex.cloud',
		getCoreUrl: async () => null,
		getCurrentJwt: async () => null,
		openExternalAuth: vi.fn(),
	}
}

describe('adapterSessionStorage', () => {
	it('prefers platform atomic session storage', async () => {
		const write = vi.fn(async () => undefined)
		const adapter = {
			...baseAdapter(),
			writeAmberiteSession: write,
			setCurrentJwt: vi.fn(async () => undefined),
			setCurrentRefreshToken: vi.fn(async () => undefined),
		}
		await adapterSessionStorage(adapter).write({ token: 'access', refreshToken: 'refresh' })
		expect(write).toHaveBeenCalledWith({ token: 'access', refreshToken: 'refresh' })
		expect(adapter.setCurrentJwt).not.toHaveBeenCalled()
	})

	it('rolls back legacy split writes when the second write fails', async () => {
		let access = 'old-access'
		let refresh = 'old-refresh'
		let fail = true
		const adapter: PlatformAdapter = {
			...baseAdapter(),
			getCurrentJwt: async () => access,
			setCurrentJwt: async (value) => {
				access = value ?? ''
			},
			getCurrentRefreshToken: async () => refresh,
			setCurrentRefreshToken: async (value) => {
				if (fail) {
					fail = false
					throw new Error('disk failed')
				}
				refresh = value ?? ''
			},
		}
		await expect(
			adapterSessionStorage(adapter).write({ token: 'new-access', refreshToken: 'new-refresh' }),
		).rejects.toThrow('disk failed')
		expect({ access, refresh }).toEqual({ access: 'old-access', refresh: 'old-refresh' })
	})
})
