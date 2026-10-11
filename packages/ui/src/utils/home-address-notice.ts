import { defineMessage, type MessageDescriptor } from '../composables/i18n'

const SHOWN_KEY = 'visible-to-friends-home-address-notice'

const homeAddressNotice = defineMessage({
	id: 'servers.visible-to-friends.home-address-notice',
	defaultMessage: 'People who join this server can see your home address.',
})

/**
 * Says once per installation what showing a server to friends gives away. Call when a server
 * becomes visible to friends: when it is created that way, or when the setting is turned on.
 */
export function notifyHomeAddressOnce(
	addNotification: (notification: { type: 'info'; title: string }) => unknown,
	formatMessage: (message: MessageDescriptor) => string,
) {
	if (localStorage.getItem(SHOWN_KEY)) return
	localStorage.setItem(SHOWN_KEY, 'true')
	addNotification({ type: 'info', title: formatMessage(homeAddressNotice) })
}
