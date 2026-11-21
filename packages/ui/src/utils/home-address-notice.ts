import { defineMessage, type MessageDescriptor } from '../composables/i18n'

const SHOWN_KEY = 'home-address-notice'

const homeAddressNotice = defineMessage({
	id: 'servers.home-address-notice',
	defaultMessage: 'People who join this server can see your home address.',
})

/**
 * Says once per installation what a server's players can see. Call when a server is created for a
 * shared instance, whose players can join it.
 */
export function notifyHomeAddressOnce(
	addNotification: (notification: { type: 'info'; title: string }) => unknown,
	formatMessage: (message: MessageDescriptor) => string,
) {
	if (localStorage.getItem(SHOWN_KEY)) return
	localStorage.setItem(SHOWN_KEY, 'true')
	addNotification({ type: 'info', title: formatMessage(homeAddressNotice) })
}
