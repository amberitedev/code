import { defineMessage, type MessageDescriptor } from '../composables/i18n'

const SHOWN_KEY = 'home-address-notice'

const homeAddressNotice = defineMessage({
	id: 'servers.home-address-notice',
	defaultMessage: 'People who join this server can see your home address.',
})

/**
 * Says once per installation that a server's players see its address. Call when a server is
 * created for a shared instance, whose members are the people likely to be invited to play.
 */
export function notifyHomeAddressOnce(
	addNotification: (notification: { type: 'info'; title: string }) => unknown,
	formatMessage: (message: MessageDescriptor) => string,
) {
	if (localStorage.getItem(SHOWN_KEY)) return
	localStorage.setItem(SHOWN_KEY, 'true')
	addNotification({ type: 'info', title: formatMessage(homeAddressNotice) })
}
