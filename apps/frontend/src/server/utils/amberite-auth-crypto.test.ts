import { describe, expect, it } from 'vitest'

import {
	createPkceChallenge,
	normalizeAuthRedirect,
	normalizeAuthUuid,
	requestIsSameOrigin,
	sealAuthFlow,
	unsealAuthFlow,
} from './amberite-auth-crypto'

const secret = 'test-only-cookie-signing-secret-32-bytes-minimum'
const flow = {
	state: 'state',
	verifier: 'verifier',
	intent: 'continue' as const,
	expectedMinecraftUuid: '12345678-90ab-cdef-1234-567890abcdef',
	redirect: '/settings/profile?tab=identity',
	expiresAt: 2_000,
}

describe('website Minecraft auth contract', () => {
	it('creates the RFC 7636 SHA-256 challenge', async () => {
		expect(await createPkceChallenge('dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk')).toBe(
			'E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM',
		)
	})

	it('integrity-protects intent, expected UUID, destination, and expiry', async () => {
		const sealed = await sealAuthFlow(flow, secret)
		expect(await unsealAuthFlow(sealed, secret, 1_000)).toEqual(flow)
		await expect(unsealAuthFlow(`${sealed}tampered`, secret, 1_000)).rejects.toThrow(
			'minecraft_auth_state_invalid',
		)
		await expect(unsealAuthFlow(sealed, secret, 2_001)).rejects.toThrow(
			'minecraft_auth_state_expired',
		)
	})

	it('rejects signed but malformed flow payloads', async () => {
		const malformed = await sealAuthFlow(
			{ ...flow, redirect: 'https://evil.example' } as typeof flow,
			secret,
		)
		await expect(unsealAuthFlow(malformed, secret, 1_000)).rejects.toThrow(
			'minecraft_auth_state_invalid',
		)
		await expect(unsealAuthFlow('not-base64.not-base64', secret, 1_000)).rejects.toThrow(
			'minecraft_auth_state_invalid',
		)
	})

	it('normalizes only path-local destinations and canonical UUIDs', () => {
		expect(normalizeAuthRedirect('/groups/one?tab=members')).toBe('/groups/one?tab=members')
		expect(normalizeAuthRedirect('https://evil.example')).toBe('/dashboard')
		expect(normalizeAuthRedirect('//evil.example')).toBe('/dashboard')
		expect(normalizeAuthUuid('1234567890ABCDEF1234567890ABCDEF')).toBe(
			'12345678-90ab-cdef-1234-567890abcdef',
		)
		expect(() => normalizeAuthUuid('bad')).toThrow('invalid Minecraft UUID')
	})

	it('rejects cross-site session requests', () => {
		expect(
			requestIsSameOrigin({
				expectedOrigin: 'https://amberite.example',
				origin: 'https://amberite.example',
				fetchSite: 'same-origin',
			}),
		).toBe(true)
		expect(
			requestIsSameOrigin({
				expectedOrigin: 'https://amberite.example',
				origin: 'https://evil.example',
				fetchSite: 'cross-site',
			}),
		).toBe(false)
		expect(
			requestIsSameOrigin({
				expectedOrigin: 'https://amberite.example',
				referer: 'https://amberite.example/settings/profile',
			}),
		).toBe(true)
		expect(requestIsSameOrigin({ expectedOrigin: 'https://amberite.example' })).toBe(false)
	})
})
