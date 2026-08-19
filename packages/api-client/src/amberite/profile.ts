import { AmberiteAuthError } from './errors'

export interface AmberiteAccountUser {
	id: string
	userId: string
	username: string
	display_name: string
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
	role: 'developer'
	badges: number
	allow_friend_requests: boolean
	friendCode?: string
}

export interface AmberiteProfilePatch {
	displayName?: string
	bio?: string | null
	avatar?: null | {
		url: string
		storageId?: string
		mimeType?: string
		sizeBytes?: number
	}
	allowFriendRequests?: boolean
}

export function normalizeAmberiteAccountUser(value: unknown): AmberiteAccountUser {
	if (!value || typeof value !== 'object')
		throw new AmberiteAuthError('invalid Amberite profile response')
	const raw = value as Record<string, unknown>
	const id = requiredString(raw.id ?? raw.minecraftUuid, 'id')
	const username = requiredString(raw.username ?? raw.verifiedMinecraftHandle, 'username')
	const displayName = stringValue(raw.display_name ?? raw.displayName ?? raw.name) ?? username
	return {
		id,
		userId: requiredString(raw.userId, 'userId'),
		username,
		display_name: displayName,
		minecraftUuid: requiredString(raw.minecraftUuid ?? raw.id, 'minecraftUuid'),
		verifiedMinecraftHandle: requiredString(
			raw.verifiedMinecraftHandle ?? raw.username,
			'verifiedMinecraftHandle',
		),
		name: displayName,
		avatar_url: nullableString(raw.avatar_url ?? raw.image),
		bio: nullableString(raw.bio),
		created: stringValue(raw.created) ?? new Date(0).toISOString(),
		email: nullableString(raw.email),
		email_verified: booleanValue(raw.email_verified),
		auth_providers: stringArray(raw.auth_providers),
		has_password: booleanValue(raw.has_password),
		has_totp: booleanValue(raw.has_totp),
		role: 'developer',
		badges: numberValue(raw.badges),
		allow_friend_requests:
			typeof raw.allow_friend_requests === 'boolean' ? raw.allow_friend_requests : true,
		...(typeof raw.friendCode === 'string' ? { friendCode: raw.friendCode } : {}),
	}
}

export function mapAmberiteUserToAccountUser(value: object): AmberiteAccountUser {
	return normalizeAmberiteAccountUser(value)
}

function requiredString(value: unknown, field: string): string {
	if (typeof value !== 'string' || !value)
		throw new AmberiteAuthError(`invalid Amberite profile response: missing ${field}`)
	return value
}
function stringValue(value: unknown) {
	return typeof value === 'string' ? value : undefined
}
function nullableString(value: unknown) {
	return typeof value === 'string' ? value : null
}
function booleanValue(value: unknown) {
	return typeof value === 'boolean' ? value : false
}
function numberValue(value: unknown) {
	return typeof value === 'number' && Number.isFinite(value) ? value : 0
}
function stringArray(value: unknown) {
	return Array.isArray(value)
		? value.filter((entry): entry is string => typeof entry === 'string')
		: []
}
