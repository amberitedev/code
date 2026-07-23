export const AMBERITE_SESSION_POLICY = {
	totalDurationMs: 365 * 24 * 60 * 60 * 1_000,
	inactiveDurationMs: 90 * 24 * 60 * 60 * 1_000,
	jwtDurationMs: 15 * 60 * 1_000,
} as const

export const MINECRAFT_UUID_PATTERN =
	/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/

export function normalizeMinecraftUuid(raw: string): string {
	const value = raw.trim().toLowerCase()
	if (!/^[0-9a-f]{32}$/.test(value) && !MINECRAFT_UUID_PATTERN.test(value)) {
		throw new Error('invalid Minecraft UUID')
	}
	const compact = value.replaceAll('-', '')
	const normalized = [
		compact.slice(0, 8),
		compact.slice(8, 12),
		compact.slice(12, 16),
		compact.slice(16, 20),
		compact.slice(20),
	].join('-')
	if (!MINECRAFT_UUID_PATTERN.test(normalized)) throw new Error('invalid Minecraft UUID')
	return normalized
}

export function normalizeMinecraftHandle(raw: string): string {
	const handle = raw.trim()
	if (!/^[a-zA-Z0-9_]{3,16}$/.test(handle)) throw new Error('invalid Minecraft handle')
	return handle
}

export function shouldSyncDefaultMinecraftDisplayName(
	displayName: string | undefined,
	legacyName: string | undefined,
	previousHandle: string | undefined,
): boolean {
	const currentDisplayName = displayName ?? legacyName
	return currentDisplayName === undefined || currentDisplayName === previousHandle
}
