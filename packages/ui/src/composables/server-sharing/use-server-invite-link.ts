import { ref, watch } from 'vue'

import { useVIntl } from '#ui/composables/i18n'
import { injectModrinthClient } from '#ui/providers'
import { commonMessages } from '#ui/utils/common-messages'

import { createServerInviteLink } from './invite-link'
import { injectShareUnlinkPrompt, ShareUnlinkDeclined } from './shareable'

export type ServerInviteLinkTarget = {
	serverId: string
	siteUrl: string
	/** Wait for the server's install to end first, as setup does after a pack upload. */
	afterInstall?: boolean
}

export type ServerInviteLink = ReturnType<typeof useServerInviteLink>

/** Loads a share URL for a server world, with retry and copied state for the invite link UI. */
export function useServerInviteLink() {
	const client = injectModrinthClient()
	const shareUnlink = injectShareUnlinkPrompt(null)
	const { formatMessage } = useVIntl()
	const link = ref<string | null>(null)
	const loading = ref(false)
	const error = ref<string | null>(null)
	const copied = ref(false)
	let controller: AbortController | null = null
	let target: ServerInviteLinkTarget | null = null

	watch(link, () => {
		copied.value = false
	})

	async function retry() {
		if (!target || loading.value) return
		controller?.abort()
		const { signal } = (controller = new AbortController())
		const { serverId, siteUrl, afterInstall } = target
		loading.value = true
		error.value = null
		try {
			const next = await createServerInviteLink(client, serverId, siteUrl, {
				signal,
				shareUnlink,
				afterInstall,
			})
			if (!signal.aborted) link.value = next
		} catch (err) {
			if (signal.aborted) return
			error.value =
				err instanceof ShareUnlinkDeclined
					? formatMessage(commonMessages.shareRequiresUnlinkBody)
					: err instanceof Error
						? err.message
						: String(err)
		} finally {
			if (!signal.aborted) loading.value = false
		}
	}

	function start(next: ServerInviteLinkTarget) {
		controller?.abort()
		target = next
		link.value = null
		loading.value = false
		error.value = null
		copied.value = false
		void retry()
	}

	/** Stops an in-flight load and keeps whatever link is already shown. */
	function cancel() {
		controller?.abort()
		loading.value = false
	}

	function reset() {
		cancel()
		target = null
		link.value = null
		error.value = null
		copied.value = false
	}

	return { link, loading, error, copied, start, retry, cancel, reset }
}
