import type { IconConfig } from '@modrinth/api-client'

import type {
	AppNotification,
	SharedInstanceInvite,
	SharedInstanceInviteNotificationBody,
} from './shared-instance-invite-types'

function optionalString(value: unknown) {
	return typeof value === 'string' ? value : null
}

/** The icon editor settings, when `value` has their shape. The App checks the colors and the
 * symbol when it draws them. */
function optionalIconConfig(value: unknown): IconConfig | null {
	if (typeof value !== 'object' || value === null) return null
	if (!('symbol' in value) || typeof value.symbol !== 'string') return null
	if (!('background' in value) || typeof value.background !== 'object' || !value.background)
		return null
	const { background, symbol } = value
	if (
		'type' in background &&
		background.type === 'color' &&
		'value' in background &&
		typeof background.value === 'string'
	)
		return { background: { type: 'color', value: background.value }, symbol }
	if (
		'type' in background &&
		background.type === 'linear-top-down-gradient' &&
		'top_color' in background &&
		typeof background.top_color === 'string' &&
		'bottom_color' in background &&
		typeof background.bottom_color === 'string'
	)
		return {
			background: {
				type: 'linear-top-down-gradient',
				top_color: background.top_color,
				bottom_color: background.bottom_color,
			},
			symbol,
		}
	return null
}

export function parseSharedInstanceInviteNotification(
	notification: AppNotification,
): SharedInstanceInvite | null {
	if (notification.body?.type !== 'shared_instance_invite') return null

	const body = notification.body as SharedInstanceInviteNotificationBody
	const sharedInstanceId = optionalString(body.shared_instance_id)
	const sharedInstanceName = optionalString(body.shared_instance_name)
	if (!sharedInstanceId || !sharedInstanceName) return null

	return {
		sharedInstanceId,
		sharedInstanceName,
		invitedById: optionalString(body.invited_by),
		invitedByUsername: optionalString(body.invited_by_username),
		invitedByAvatarUrl: optionalString(body.invited_by_avatar_url),
		instanceIconUrl: optionalString(body.shared_instance_icon),
		instanceIconConfig: optionalIconConfig(body.shared_instance_icon_config),
	}
}
