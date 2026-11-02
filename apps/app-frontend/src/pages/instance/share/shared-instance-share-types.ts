import { inviteLinkUrl } from '@modrinth/ui'

import { config } from '@/config'

export {
	invitedPlayerMethodMessages as methodMessages,
	normalizeInviteKey,
	SHARED_INSTANCE_MAX_PLAYERS as SHARED_INSTANCE_USER_LIMIT,
	type InvitedPlayerMethod as ShareMethod,
	type InvitedPlayerRow as ShareRow,
} from '@modrinth/ui'

/** The link that joins one of this App's shares, on Amberite's account site. */
export function sharedInstanceInviteUrl(inviteId: string) {
	return inviteLinkUrl(config.accountSiteUrl, inviteId)
}
