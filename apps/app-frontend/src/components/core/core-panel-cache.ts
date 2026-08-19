import type {
	CoreAccessMember,
	CoreActivityLogEntry,
	CoreInvitation,
	CoreRole,
} from '@modrinth/api-client'

export const coreAccessCache = new Map<
	string,
	{
		roles: CoreRole[]
		members: CoreAccessMember[]
		invitations: CoreInvitation[]
		canManageUsers: boolean
	}
>()

export const coreActivityCache = new Map<
	string,
	{ entries: CoreActivityLogEntry[]; cursor: string | null }
>()
