import { defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'

import {
	getErrorMessage,
	getSharedInstanceUnavailableReason,
	isSharedInstancesApiError,
	type SharedInstanceUnavailableReason,
} from '@/helpers/install'

/**
 * Everything that can stop sharing from working on an instance. `missing` and `wrong_account` are
 * the owner's side: the backend no longer has the shared instance, or another account owns it.
 */
export type SharedInstanceAvailability =
	| SharedInstanceUnavailableReason
	| 'missing'
	| 'wrong_account'

export const sharedInstanceErrorMessages = defineMessages({
	missingTitle: {
		id: 'instance.shared-instance.missing.title',
		defaultMessage: 'No longer shared',
	},
	missingText: {
		id: 'instance.shared-instance.missing.text',
		defaultMessage:
			'This instance is no longer shared. Share it again to invite friends, or stop sharing.',
	},
	shareAgainButton: {
		id: 'instance.shared-instance.missing.share-again-button',
		defaultMessage: 'Share again',
	},
	stopSharingButton: {
		id: 'instance.shared-instance.missing.stop-sharing-button',
		defaultMessage: 'Stop sharing',
	},
	wrongAccountTitle: {
		id: 'instance.shared-instance.wrong-account.title',
		defaultMessage: 'Wrong account',
	},
	wrongAccountText: {
		id: 'instance.shared-instance.wrong-account.text',
		defaultMessage: 'Sign in as the account that shared this instance to manage sharing.',
	},
	pushUnfinishedTitle: {
		id: 'instance.shared-instance.push-unfinished.title',
		defaultMessage: 'Update not pushed',
	},
	pushUnfinishedText: {
		id: 'instance.shared-instance.push-unfinished.text',
		defaultMessage: 'The push did not finish. Push again.',
	},
	inviteFailedTitle: {
		id: 'instance.shared-instance.invite-failed.title',
		defaultMessage: 'Invite not sent',
	},
	inviteFailedText: {
		id: 'instance.shared-instance.invite-failed.text',
		defaultMessage: '{username} could not be invited. They may not accept invites from you.',
	},
	unavailableTitle: {
		id: 'instance.shared-instance.unavailable.title',
		defaultMessage: 'Shared instance no longer available',
	},
	lockedTitle: {
		id: 'instance.shared-instance.unavailable.locked-title',
		defaultMessage: 'Instance locked',
	},
	unavailableText: {
		id: 'instance.shared-instance.unavailable.text',
		defaultMessage:
			"Your local instance is still available, but it is no longer linked and won't receive updates.",
	},
	deletedText: {
		id: 'instance.shared-instance.unavailable.deleted-text',
		defaultMessage:
			'The primary instance was deleted. This instance is still available, but it is no longer linked and will no longer receive updates.',
	},
	accessRevokedText: {
		id: 'instance.shared-instance.unavailable.access-revoked-text',
		defaultMessage:
			'Your access to the shared instance was revoked. This instance is still available, but it is no longer linked and will no longer receive updates.',
	},
	lockedText: {
		id: 'instance.shared-instance.unavailable.locked-text',
		defaultMessage:
			'This shared instance was locked by the Content Moderation team. It will no longer receive updates from the primary instance and cannot be played.',
	},
	unavailableFallbackManager: {
		id: 'instance.shared-instance.unavailable.manager-fallback',
		defaultMessage: 'the instance manager',
	},
	errorTitle: {
		id: 'instance.shared-instance.error.title',
		defaultMessage: 'Something has gone wrong',
	},
	networkErrorTitle: {
		id: 'instance.shared-instance.network-error.title',
		defaultMessage: 'Network error',
	},
	networkErrorText: {
		id: 'instance.shared-instance.network-error.text',
		defaultMessage: 'Unable to connect to shared instances API',
	},
})

export function sharedInstanceUnavailableTextMessage(
	reason: SharedInstanceUnavailableReason | null,
) {
	if (reason === 'deleted') return sharedInstanceErrorMessages.deletedText
	if (reason === 'access_revoked') return sharedInstanceErrorMessages.accessRevokedText
	if (reason === 'quarantined') return sharedInstanceErrorMessages.lockedText
	return sharedInstanceErrorMessages.unavailableText
}

export function sharedInstanceUnavailableTitleMessage(
	reason: SharedInstanceUnavailableReason | null,
) {
	return reason === 'quarantined'
		? sharedInstanceErrorMessages.lockedTitle
		: sharedInstanceErrorMessages.unavailableTitle
}

export function useSharedInstanceErrors() {
	const { formatMessage } = useVIntl()
	const { addNotification } = injectNotificationManager()

	function formatSharedInstanceUnavailable(
		reason: SharedInstanceUnavailableReason | null = null,
		manager?: string | null,
	) {
		return formatMessage(sharedInstanceUnavailableTextMessage(reason), {
			manager: manager || formatMessage(sharedInstanceErrorMessages.unavailableFallbackManager),
		})
	}

	function notifySharedInstanceUnavailable(
		reason: SharedInstanceUnavailableReason | null = null,
		manager?: string | null,
	) {
		addNotification({
			type: 'warning',
			title: formatMessage(sharedInstanceUnavailableTitleMessage(reason)),
			text: formatSharedInstanceUnavailable(reason, manager),
		})
	}

	/** The one message for a sharing action that hit an availability problem. */
	function notifySharedInstanceAvailability(
		availability: SharedInstanceAvailability,
		manager?: string | null,
	) {
		if (availability === 'missing' || availability === 'wrong_account') {
			const missing = availability === 'missing'
			addNotification({
				type: 'warning',
				title: formatMessage(
					missing
						? sharedInstanceErrorMessages.missingTitle
						: sharedInstanceErrorMessages.wrongAccountTitle,
				),
				text: formatMessage(
					missing
						? sharedInstanceErrorMessages.missingText
						: sharedInstanceErrorMessages.wrongAccountText,
				),
			})
			return
		}
		notifySharedInstanceUnavailable(availability, manager)
	}

	function notifySharedInstanceConnectionError() {
		addNotification({
			type: 'error',
			title: formatMessage(sharedInstanceErrorMessages.networkErrorTitle),
			text: formatMessage(sharedInstanceErrorMessages.networkErrorText),
		})
	}

	function notifySharedInstanceError(error: unknown) {
		const unavailableReason = getSharedInstanceUnavailableReason(error)
		if (unavailableReason) {
			notifySharedInstanceUnavailable(unavailableReason)
			return
		}

		if (isSharedInstancesApiError(error)) {
			notifySharedInstanceConnectionError()
			return
		}

		addNotification({
			type: 'error',
			title: formatMessage(sharedInstanceErrorMessages.errorTitle),
			text: getErrorMessage(error),
		})
	}

	return {
		formatSharedInstanceUnavailable,
		notifySharedInstanceAvailability,
		notifySharedInstanceConnectionError,
		notifySharedInstanceError,
		notifySharedInstanceUnavailable,
	}
}
