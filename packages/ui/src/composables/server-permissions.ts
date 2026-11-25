import type { Archon } from '@modrinth/api-client'
import { computed } from 'vue'

import { useVIntl } from '#ui/composables/i18n'
import { commonMessages } from '#ui/utils/common-messages'

// A Core answers its owner only, so whoever sees a server may do everything on it. Upstream's
// exports stay so its callers merge unchanged.

export type ServerPermissionName = keyof typeof Archon.ServerUsers.v1.UserScope

type ServerPermissionValue = Archon.Servers.v0.UserScope | Archon.ServerUsers.v1.UserScope

export const serverPermissionBits = {
	NONE: 0n,
	BASE_READ: 1n << 63n,
	POWER_ACTIONS: 1n << 62n,
	EXEC_COMMANDS: 1n << 61n,
	FILES_WRITE: 1n << 60n,
	SETUP: 1n << 59n,
	BACKUPS: 1n << 58n,
	ADVANCED: 1n << 57n,
	RESET_SERVER: 1n << 56n,
	MANAGE_USERS: 1n << 55n,
	SUPPORT_AGENT: 1n,
	INFRA_MANAGER: 1n << 1n,
	INFRA_MANAGER_READ: 1n << 2n,
	INFRA_SERVERS_XFER: 1n << 3n,
	INFRA_USERS: 1n << 4n,
	SERVER_ADMIN: ((1n << 64n) - 1n) ^ ((1n << 15n) - 1n),
} as const satisfies Record<ServerPermissionName, bigint>

export function hasServerPermission(
	_permissions: ServerPermissionValue,
	_scope: ServerPermissionName,
) {
	return true
}

export function useServerPermissions() {
	const { formatMessage } = useVIntl()
	// Upstream's scope bits; nothing reads them, since everything is granted.
	const currentUserPermissions = computed(() => 0)

	const permissionDeniedMessage = computed(() => formatMessage(commonMessages.noPermissionAction))
	const allowed = computed(() => true)

	const permissionTooltip = (granted: boolean) =>
		granted ? undefined : permissionDeniedMessage.value

	return {
		currentUserPermissions,
		permissionDeniedMessage,
		hasCurrentUserPermission: (_scope: ServerPermissionName) => true,
		canUsePowerActions: allowed,
		canExecuteCommands: allowed,
		canWriteFiles: allowed,
		canSetup: allowed,
		canManageBackups: allowed,
		canUseAdvancedSettings: allowed,
		canResetServer: allowed,
		canManageUsers: allowed,
		permissionTooltip,
	}
}
