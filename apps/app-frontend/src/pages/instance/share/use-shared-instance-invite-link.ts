import {
	DEFAULT_INVITE_LINK_MAX_USES,
	inviteLinkMaxAge,
	inviteLinkMaxUses,
	type InviteLinkSettings,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, type Ref, ref, watch } from 'vue'

import { create_shared_instance_invite_link } from '@/helpers/instance'

import { sharedInstanceInviteUrl } from './shared-instance-share-types'

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
		details.value ? sharedInstanceInviteUrl(details.value.inviteId) : undefined,
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
		if (Math.floor(maxInviteUses.value) <= 0) return

		const replaced = details.value.inviteId
		pending.value = true
		try {
			details.value = await create_shared_instance_invite_link(instanceId.value, {
				maxAgeSeconds: inviteLinkMaxAge(settings.expiresAt),
				maxUses: inviteLinkMaxUses(settings.maxUses, maxInviteUses.value),
				replaceInviteId: replaced,
			})
			void refreshInvites()
		} catch (error) {
			// Reported here and not rethrown, so the link editor does not show the raw request error.
			// The new link is made before the old one is deleted, so a failure keeps the old one.
			onError(error)
			void refreshInvites()
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
