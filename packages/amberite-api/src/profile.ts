import type { AmberiteProfile, AmberiteUser, ProfileVisibilitySettings } from './convex-types'
import { AuthError } from './errors'

export interface AmberiteAccountUser {
	id: string
	userId: string
	username: string
	minecraftUuid: string
	verifiedMinecraftHandle: string
	name: string
	avatar_url: string | null
	bio: string | null
	created: string
	email: string | null
	email_verified: boolean
	auth_providers: string[]
	has_password: boolean
	has_totp: boolean
	role: string
	badges: number
}

export interface AmberiteProfilePatch {
	displayName?: string
	bio?: string
	avatar?: null | {
		url: string
		storageId?: string
		mimeType?: string
		sizeBytes?: number
	}
	profileVisibility?: Partial<ProfileVisibilitySettings>
	favoriteModpackProjectIds?: string[]
	showcaseAchievementIds?: string[]
}

export function mapAmberiteUserToAccountUser(
	value: AmberiteUser | AmberiteProfile,
): AmberiteAccountUser {
	const raw = value as unknown as Record<string, unknown>
	const id = requiredString(raw.id ?? raw.userId, 'id')
	const verifiedMinecraftHandle = requiredString(
		raw.verifiedMinecraftHandle ?? raw.username,
		'verifiedMinecraftHandle',
	)
	const minecraftUuid = requiredString(raw.minecraftUuid, 'minecraftUuid')
	const name = stringValue(raw.name ?? raw.displayName) ?? verifiedMinecraftHandle
	return {
		id,
		userId: requiredString(raw.userId ?? raw.id, 'userId'),
		username: verifiedMinecraftHandle,
		minecraftUuid,
		verifiedMinecraftHandle,
		name,
		avatar_url: nullableString(raw.avatar_url ?? raw.image),
		bio: nullableString(raw.bio),
		created: stringValue(raw.created) ?? new Date(0).toISOString(),
		email: nullableString(raw.email),
		email_verified: booleanValue(raw.email_verified),
		auth_providers: stringArray(raw.auth_providers),
		has_password: booleanValue(raw.has_password),
		has_totp: booleanValue(raw.has_totp),
		role: stringValue(raw.role) ?? '',
		badges: numberValue(raw.badges),
	}
}

export function normalizeAmberiteAccountUser(value: unknown): AmberiteAccountUser {
	if (!value || typeof value !== 'object') throw new AuthError('invalid Amberite profile response')
	return mapAmberiteUserToAccountUser(value as AmberiteUser | AmberiteProfile)
}

function requiredString(value: unknown, field: string): string {
	if (typeof value !== 'string' || !value) {
		throw new AuthError(`invalid Amberite profile response: missing ${field}`)
	}
	return value
}

function stringValue(value: unknown): string | undefined {
	return typeof value === 'string' ? value : undefined
}

function nullableString(value: unknown): string | null {
	return typeof value === 'string' ? value : null
}

function booleanValue(value: unknown): boolean {
	return typeof value === 'boolean' ? value : false
}

function numberValue(value: unknown): number {
	return typeof value === 'number' && Number.isFinite(value) ? value : 0
}

function stringArray(value: unknown): string[] {
	return Array.isArray(value)
		? value.filter((item): item is string => typeof item === 'string')
		: []
}
