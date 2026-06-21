import type {
	CoreAccessMember,
	CoreAccessRole,
	CoreActivityLogEntry,
	CorePermissionPreset,
} from '../types'

export type AmberiteAccessUiRole = 'owner' | 'editor' | 'viewer'

export interface AmberiteAccessUiMember {
	id: string
	user: {
		id: string
		username: string
		avatarUrl?: string
	}
	role: AmberiteAccessUiRole
	joinedAt: string | null
	pending?: boolean
	inviteCandidate?: boolean
	isOwner?: boolean
	source?: 'core' | 'instance'
}

export interface AmberiteAccessUiActivityEntry {
	id: string
	actor: {
		id: string
		username: string
		avatarUrl?: string
	}
	world: {
		id: string
		name: string
	} | null
	event: {
		type: string
		label: string
		details?: string
	}
	timestamp: string
}

export const amberiteAccessRoleOptions = [
	{ value: 'owner' as const, label: 'Owner', description: 'Full Core access.' },
	{ value: 'editor' as const, label: 'Admin', description: 'Can manage servers and members.' },
	{ value: 'viewer' as const, label: 'Member', description: 'Can view and use shared access.' },
]

export function coreAccessRoleToUi(role: CoreAccessRole, preset?: CorePermissionPreset) {
	if (role === 'owner') return 'owner'
	if (role === 'admin') return 'editor'
	if (preset === 'viewer' || preset === 'client-only') return 'viewer'
	return 'viewer'
}

export function uiAccessRoleToCore(role: AmberiteAccessUiRole): CoreAccessRole {
	if (role === 'owner') return 'owner'
	if (role === 'editor') return 'admin'
	return 'member'
}

export function uiAccessRoleToPreset(role: AmberiteAccessUiRole): CorePermissionPreset {
	if (role === 'owner') return 'owner'
	if (role === 'editor') return 'admin'
	return 'member'
}

export function toAmberiteAccessUiMember(member: CoreAccessMember): AmberiteAccessUiMember {
	return {
		id: `${member.source}-${member.user_id}`,
		user: {
			id: member.user_id,
			username: member.display_name ?? member.user_id,
		},
		role: coreAccessRoleToUi(member.role, member.permission_preset),
		joinedAt: member.joined_at,
		isOwner: member.role === 'owner',
		source: member.source,
	}
}

export function toAmberiteActivityEntry(
	entry: CoreActivityLogEntry,
): AmberiteAccessUiActivityEntry {
	const metadata = parseMetadata(entry.metadata_json)
	const instanceName =
		typeof metadata.instance_name === 'string'
			? metadata.instance_name
			: entry.instance_id
				? `Instance ${entry.instance_id}`
				: null
	return {
		id: entry.id,
		actor: {
			id: entry.actor_user_id,
			username: String(metadata.actor_name ?? entry.actor_user_id),
		},
		world: entry.instance_id
			? {
					id: entry.instance_id,
					name: instanceName ?? entry.instance_id,
				}
			: null,
		event: {
			type: entry.action,
			label: formatActivityAction(entry.action),
			details:
				typeof metadata.name === 'string'
					? metadata.name
					: entry.target_user_id
						? `Target: ${entry.target_user_id}`
						: undefined,
		},
		timestamp: entry.created_at,
	}
}

export function canManageAmberiteAccess(permissions: string[]) {
	return permissions.includes('members:manage')
}

export function formatActivityAction(action: string) {
	return action
		.split('_')
		.map((part) => part.charAt(0).toUpperCase() + part.slice(1))
		.join(' ')
}

function parseMetadata(raw: string | null | undefined): Record<string, unknown> {
	if (!raw) return {}
	try {
		const parsed = JSON.parse(raw)
		return parsed && typeof parsed === 'object' ? parsed : {}
	} catch {
		return {}
	}
}
