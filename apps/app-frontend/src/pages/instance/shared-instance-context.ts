import { createContext } from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, type Ref, ref, watch } from 'vue'

import { useAmberiteAuth } from '@/composables/useAmberiteAuth'
import { useSocialClient } from '@/composables/useSocialClient'
import {
	getSharedInstanceUnavailableReason,
	install_get_shared_instance_update_preview,
	isSharedInstanceUnavailableError,
	type SharedInstanceUnavailableReason,
} from '@/helpers/install'
import { can_current_user_use_shared_instances } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types'

import { instanceKeys } from './query-options'

export type SharedInstanceManager =
	| {
			type: 'user'
			name: string
			avatarUrl?: string
			tintBy: string
	  }
	| {
			type: 'server'
			name: string
			avatarUrl?: string
			tintBy: string
	  }

export function createSharedInstanceContext(
	instance: Ref<GameInstance | undefined>,
	offline: Ref<boolean>,
	notifyError: (error: unknown) => void,
) {
	const auth = useAmberiteAuth()
	const socialClient = useSocialClient()
	const queryClient = useQueryClient()
	const forcedUnavailableReason = ref<SharedInstanceUnavailableReason | null>(null)

	const expectedUserId = computed(() => instance.value?.shared_instance?.linked_user_id ?? null)
	const wrongAccount = computed(() => {
		if (!auth.isReady.value) return false
		if (!expectedUserId.value) return false
		return auth.user.value?.userId !== expectedUserId.value
	})
	const actionsLocked = computed(() => wrongAccount.value)
	const signedOut = computed(() => auth.status.value !== 'authenticated')
	const managerUserId = computed(() => {
		const attachment = instance.value?.shared_instance
		if (!attachment) return null
		if (attachment.role === 'owner') {
			return actionsLocked.value ? (attachment.linked_user_id ?? null) : null
		}
		return attachment.manager_id ?? null
	})
	const managerUserQuery = useQuery({
		queryKey: computed(() => ['amberite-profile', managerUserId.value]),
		queryFn: async () =>
			managerUserId.value ? await socialClient.getProfile(managerUserId.value) : null,
		enabled: () => !!managerUserId.value,
		staleTime: 30_000,
	})
	const manager = computed<SharedInstanceManager | null>(() => {
		const attachment = instance.value?.shared_instance
		if (!attachment) return null

		if (attachment.server_manager_name) {
			return {
				type: 'server',
				name: attachment.server_manager_name,
				avatarUrl: attachment.server_manager_icon_url ?? undefined,
				tintBy: attachment.server_manager_name,
			}
		}

		const user = managerUserQuery.data.value
		if (!user) return null
		return {
			type: 'user',
			name: user.username ?? user.displayName ?? user.userId,
			avatarUrl: user.image ?? undefined,
			tintBy: user.userId,
		}
	})
	const unavailableManager = computed(() => manager.value?.name ?? null)

	const eligibilityQuery = useQuery({
		queryKey: computed(() => instanceKeys.sharedEligibility(auth.user.value?.userId)),
		queryFn: can_current_user_use_shared_instances,
		enabled: () => auth.status.value === 'authenticated' && !!auth.user.value?.userId,
		retry: false,
		staleTime: Infinity,
		refetchOnWindowFocus: false,
		refetchOnReconnect: false,
	})
	const currentUserCanUseSharedInstances = computed(
		() => auth.status.value !== 'authenticated' || eligibilityQuery.data.value !== false,
	)

	const updatePreviewQuery = useQuery({
		queryKey: computed(() =>
			instanceKeys.sharedUpdatePreview(instance.value?.id ?? '', auth.user.value?.userId),
		),
		queryFn: () => install_get_shared_instance_update_preview(instance.value!.id),
		enabled: computed(
			() =>
				!!instance.value?.id &&
				!!instance.value.shared_instance &&
				!actionsLocked.value &&
				!offline.value &&
				auth.isReady.value &&
				auth.status.value === 'authenticated' &&
				!!auth.user.value?.userId,
		),
		retry: false,
		staleTime: 30_000,
		refetchOnWindowFocus: false,
	})

	watch(updatePreviewQuery.data, (preview) => {
		if (preview !== undefined) forcedUnavailableReason.value = null
	})
	watch(updatePreviewQuery.error, (error) => {
		if (!error) return
		if (isSharedInstanceUnavailableError(error)) {
			forcedUnavailableReason.value = getSharedInstanceUnavailableReason(error)
		} else {
			notifyError(error)
		}
	})

	const unavailableReason = computed(() => forcedUnavailableReason.value)
	const shareActionsLocked = computed(() => actionsLocked.value || unavailableReason.value !== null)
	const updatePreview = computed(() =>
		unavailableReason.value ? null : (updatePreviewQuery.data.value ?? null),
	)

	watch(
		() => instance.value?.id,
		() => {
			forcedUnavailableReason.value = null
		},
	)

	async function refreshAvailability() {
		forcedUnavailableReason.value = null
		if (!instance.value?.id) return
		await queryClient.invalidateQueries({
			queryKey: instanceKeys.sharedUpdatePreview(instance.value.id, auth.user.value?.userId),
		})
	}

	async function refreshUpdatePreview() {
		forcedUnavailableReason.value = null
		if (!instance.value?.id || !auth.user.value?.userId) return null
		const result = await updatePreviewQuery.refetch({ throwOnError: true })
		return result.data ?? null
	}

	function setUnavailable(reason: SharedInstanceUnavailableReason | null) {
		forcedUnavailableReason.value = reason
	}

	return {
		actionsLocked,
		shareActionsLocked,
		unavailableReason,
		unavailableManager,
		manager,
		updatePreview,
		expectedUserId,
		wrongAccount,
		signedOut,
		eligibilityQuery,
		currentUserCanUseSharedInstances,
		refreshAvailability,
		refreshUpdatePreview,
		setUnavailable,
	}
}

export type SharedInstanceContext = ReturnType<typeof createSharedInstanceContext>

export const [injectSharedInstance, provideSharedInstance] = createContext<SharedInstanceContext>(
	'InstancePage',
	'sharedInstance',
)
