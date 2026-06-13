import { UserAccessEvent } from '@modrinth/ui'
import type { EventEntity, ServerAccessRole, ServerAuditLogEntry } from '@modrinth/ui'
import { ref } from 'vue'

const auditEntries = ref<ServerAuditLogEntry[]>([])

export function useCoreActivityLog() {
	function recordUserAccessEvent(
		kind: 'invited' | 'permission_modified' | 'removed',
		userId: string,
		username: string,
		avatarUrl?: string,
		role?: ServerAccessRole,
	) {
		const timestamp = new Date().toISOString()
		const actor = { id: 'core', username: 'Core' }
		const targetUser: EventEntity = {
			id: userId,
			label: username,
			iconUrl: avatarUrl,
			iconShape: 'circle',
			to: `https://modrinth.com/user/${encodeURIComponent(username)}`,
		}
		const permissions = role === 'editor' ? 'BASE_READ | POWER_ACTIONS | FILES_WRITE' : 'BASE_READ'

		auditEntries.value = [
			{
				id: `${kind}-${userId}-${timestamp}`,
				actor,
				world: null,
				event: {
					key: `user_${kind}`,
					component: UserAccessEvent,
					props: {
						action: `user_${kind}`,
						timestamp,
						actor,
						world: null,
						kind,
						targetUser,
						permissions: kind === 'removed' ? null : permissions,
					},
					searchText: `${kind} ${username}`.toLowerCase(),
				},
				timestamp,
			},
			...auditEntries.value,
		]
	}

	return { auditEntries, recordUserAccessEvent }
}
