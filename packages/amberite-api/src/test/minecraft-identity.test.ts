import { describe, expect, it } from 'vitest'

import {
	AMBERITE_SESSION_POLICY,
	normalizeMinecraftHandle,
	normalizeMinecraftUuid,
	shouldSyncDefaultMinecraftDisplayName,
} from '../../../../convex/minecraftIdentity'

describe('Minecraft identity authority', () => {
	it('normalizes dashed and undashed UUIDs to lowercase canonical form', () => {
		const expected = '12345678-90ab-cdef-1234-567890abcdef'
		expect(normalizeMinecraftUuid('1234567890ABCDEF1234567890ABCDEF')).toBe(expected)
		expect(normalizeMinecraftUuid('12345678-90AB-CDEF-1234-567890ABCDEF')).toBe(expected)
	})

	it('rejects malformed UUIDs and handles', () => {
		expect(() => normalizeMinecraftUuid('not-a-uuid')).toThrow('invalid Minecraft UUID')
		expect(() => normalizeMinecraftUuid('1234567-890ab-cdef-1234-567890abcdef')).toThrow(
			'invalid Minecraft UUID',
		)
		expect(() => normalizeMinecraftHandle('email@example.com')).toThrow('invalid Minecraft handle')
	})

	it('updates only default Minecraft display names when the verified handle changes', () => {
		expect(shouldSyncDefaultMinecraftDisplayName('OldHandle', 'OldHandle', 'OldHandle')).toBe(true)
		expect(shouldSyncDefaultMinecraftDisplayName(undefined, 'OldHandle', 'OldHandle')).toBe(true)
		expect(shouldSyncDefaultMinecraftDisplayName('Custom Name', 'OldHandle', 'OldHandle')).toBe(
			false,
		)
		expect(shouldSyncDefaultMinecraftDisplayName(undefined, 'Custom Name', 'OldHandle')).toBe(false)
	})

	it('uses the approved absolute, inactivity, and access-token durations', () => {
		expect(AMBERITE_SESSION_POLICY).toEqual({
			totalDurationMs: 365 * 24 * 60 * 60 * 1_000,
			inactiveDurationMs: 90 * 24 * 60 * 60 * 1_000,
			jwtDurationMs: 15 * 60 * 1_000,
		})
	})
})
