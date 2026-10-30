import type { InviteLinkSettings } from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, type Ref, ref, watch } from 'vue'

import { config } from '@/config'
import { create_shared_instance_invite_link } from '@/helpers/instance'

const DEFAULT_INVITE_LINK_MAX_USES = 10

export function useSharedInstanceInviteLink(
	instanceId: Ref<string>,
	maxInviteUses: Ref<number>,
	onError: (error: unknown) => void,
) {
	const queryClient = useQueryClient()
	// The Share tab lists the links, so a new or replaced one shows there.
	const refreshInvites = () =>
		queryClient.invalidateQueries({ queryKey: ['sharedInstanceInvites', instanceId.value] })
	const details = ref<Awaited<ReturnType<typeof create_shared_instance_invite_link>>>()
	const pending = ref(false)
	const link = computed(() =>
		details.value
			? `${config.accountWebUrl || config.accountApiUrl || config.siteUrl}/share/${encodeURIComponent(details.value.inviteId)}`
			: undefined,
	)

	async function ensure() {
		if (details.value) return true
		if (pending.value) return false
		const maxUses = Math.min(DEFAULT_INVITE_LINK_MAX_USES, Math.floor(maxInviteUses.value))
		if (maxUses <= 0) return false

		pending.value = true
		try {
			details.value = await create_shared_instance_invite_link(instanceId.value, { maxUses })
			void refreshInvites()
			return true
		} catch (error) {
			onError(error)
			return false
		} finally {
			pending.value = false
		}
	}

	async function update(settings: InviteLinkSettings) {
		if (!details.value) return
		const maxInviteLinkUses = Math.floor(maxInviteUses.value)
		if (maxInviteLinkUses <= 0) return

		pending.value = true
		try {
			const maxAgeSeconds = Math.max(
				1,
				Math.min(604800, Math.floor((settings.expiresAt.getTime() - Date.now()) / 1000)),
			)
			details.value = await create_shared_instance_invite_link(instanceId.value, {
				maxAgeSeconds,
				maxUses: Math.min(settings.maxUses, maxInviteLinkUses),
				replaceInviteId: details.value.inviteId,
			})
			void refreshInvites()
		} catch (error) {
			// Reported here and not rethrown, so the link editor does not show the raw request error.
			onError(error)
		} finally {
			pending.value = false
		}
	}

	/** Forgets the current link, for when the shared instance it belongs to is gone. */
	function reset() {
		details.value = undefined
		pending.value = false
	}

	watch(instanceId, reset)

	return { details, pending, link, ensure, update, reset }
}
