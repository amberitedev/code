import { SharedInstancesApiError } from '@modrinth/api-client'
import { MinecraftServerIcon } from '@modrinth/assets'
import { createContext, injectAuth } from '@modrinth/ui'
import { queryOptions, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, type Ref, ref, watch } from 'vue'

import { useUserQuery } from '@/composables/users/use-user-query'
import { injectInstanceLaunchState } from '@/features/instances/launch-state'
import {
	getSharedInstanceUnavailableReason,
	isSharedInstancesApiError,
	isSharedInstanceUnavailableError,
	type SharedInstanceUnavailableReason,
} from '@/helpers/install'
import {
	can_current_user_use_shared_instances,
	getInstanceIconUrl,
	unpublish_shared_instance,
} from '@/helpers/instance'
import {
	type SharedInstanceAvailability,
	useSharedInstanceErrors,
} from '@/helpers/shared-instance-errors'
import type { GameInstance } from '@/helpers/types'
import { apiClient } from '@/platform/modrinth-client'

import { instanceKeys, sharedInstanceUpdatePreviewQueryOptions } from './query-options'

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
	const auth = injectAuth()
	const queryClient = useQueryClient()
	const { notifySharedInstanceAvailability } = useSharedInstanceErrors()
	const instanceLaunch = injectInstanceLaunchState()
	const forcedUnavailableReason = ref<SharedInstanceUnavailableReason | null>(null)
	// What the backend said about an instance this App shares: it is gone, or another account owns it.
	const ownerProblem = ref<'missing' | 'wrong_account' | null>(null)
	// The local record is detached as soon as the backend reports the instance gone, so the role it
	// had is kept to tell an owner's "no longer shared" from a member's "deleted".
	let lastRole: 'owner' | 'member' | null = null

	const expectedUserId = computed(() => instance.value?.shared_instance?.linked_user_id ?? null)
	const linkedToOtherAccount = computed(() => {
		if (auth.isReady && !auth.isReady.value) return false
		if (!expectedUserId.value) return false
		return auth.user.value?.id !== expectedUserId.value
	})
	// A shared instance that is gone has no account to switch to, so "missing" wins.
	const wrongAccount = computed(
		() =>
			ownerProblem.value !== 'missing' &&
			(linkedToOtherAccount.value || ownerProblem.value === 'wrong_account'),
	)
	const actionsLocked = computed(() => wrongAccount.value)
	const signedOut = computed(() => !auth.session_token.value)
	const managerUserId = computed(() => {
		const attachment = instance.value?.shared_instance
		if (!attachment) return null
		if (attachment.role === 'owner') {
			return linkedToOtherAccount.value ? (attachment.linked_user_id ?? null) : null
		}
		return attachment.manager_id ?? null
	})
	const managerUserQuery = useUserQuery(managerUserId)
	const manager = computed<SharedInstanceManager | null>(() => {
		const attachment = instance.value?.shared_instance
		if (!attachment) return null

		if (attachment.server_manager_name) {
			return {
				type: 'server',
				name: attachment.server_manager_name,
				avatarUrl: getInstanceIconUrl(attachment.server_manager_icon_url) ?? MinecraftServerIcon,
				tintBy: attachment.server_manager_name,
			}
		}

		const user = managerUserQuery.data.value
		if (!user) return null
		return {
			type: 'user',
			name: user.username,
			avatarUrl: user.avatar_url ?? undefined,
			tintBy: user.id,
		}
	})
	const unavailableManager = computed(() => manager.value?.name ?? null)

	const eligibilityQuery = useQuery({
		queryKey: computed(() => instanceKeys.sharedEligibility(auth.user.value?.id)),
		queryFn: can_current_user_use_shared_instances,
		enabled: () => !!auth.session_token.value && !!auth.user.value?.id,
		retry: false,
		staleTime: Infinity,
		refetchOnWindowFocus: false,
		refetchOnReconnect: false,
	})
	const currentUserCanUseSharedInstances = computed(
		() => !auth.session_token.value || eligibilityQuery.data.value !== false,
	)

	const updatePreviewQuery = useQuery(
		computed(() =>
			queryOptions({
				...sharedInstanceUpdatePreviewQueryOptions(instance.value?.id ?? '', auth.user.value?.id),
				enabled:
					!!instance.value?.id &&
					instance.value.install_stage === 'installed' &&
					!!instance.value.shared_instance &&
					!actionsLocked.value &&
					!offline.value &&
					(auth.isReady?.value ?? true) &&
					!!auth.session_token.value &&
					!!auth.user.value?.id,
			}),
		),
	)

	watch(updatePreviewQuery.data, (preview) => {
		if (preview !== undefined) forcedUnavailableReason.value = null
	})
	watch(
		updatePreviewQuery.error,
		(error, previousError) => {
			if (!error) return
			if (isSharedInstanceUnavailableError(error)) {
				setUnavailable(getSharedInstanceUnavailableReason(error))
			} else if (
				!previousError &&
				updatePreviewQuery.data.value === undefined &&
				(!instance.value?.id || !instanceLaunch.isCheckingPreview(instance.value.id))
			) {
				notifyError(error)
			}
		},
		{ flush: 'sync' },
	)

	const unavailableReason = computed(() => forcedUnavailableReason.value)
	const missing = computed(() => ownerProblem.value === 'missing')
	/** The single state every sharing surface shows. `unavailableReason` is its member-side part. */
	const availability = computed<SharedInstanceAvailability | null>(() => {
		if (missing.value) return 'missing'
		if (wrongAccount.value) return 'wrong_account'
		return unavailableReason.value
	})
	const shareActionsLocked = computed(
		() => actionsLocked.value || unavailableReason.value !== null || missing.value,
	)
	const updatePreview = computed(() =>
		unavailableReason.value ? null : (updatePreviewQuery.data.value ?? null),
	)
	const lastUpdateCheckAt = computed(() => updatePreviewQuery.dataUpdatedAt.value || undefined)

	watch(
		() => [instance.value?.id, instance.value?.shared_instance?.id] as const,
		([id, sharedId], previous) => {
			if (id !== previous?.[0]) {
				forcedUnavailableReason.value = null
				ownerProblem.value = null
				lastRole = null
			} else if (sharedId && sharedId !== previous?.[1]) {
				// Attached to a different shared instance, so whatever was wrong with the old one is over.
				ownerProblem.value = null
			}
			lastRole = instance.value?.shared_instance?.role ?? lastRole
		},
		{ immediate: true },
	)
	watch(
		() => auth.user.value?.id,
		() => {
			if (ownerProblem.value === 'wrong_account') ownerProblem.value = null
		},
	)

	async function refreshAvailability() {
		forcedUnavailableReason.value = null
		if (!instance.value?.id) return
		await queryClient.invalidateQueries({
			queryKey: instanceKeys.sharedUpdatePreview(instance.value.id, auth.user.value?.id),
		})
	}

	async function refreshUpdatePreview() {
		forcedUnavailableReason.value = null
		if (!instance.value?.id || !auth.user.value?.id) return null
		const result = await instanceLaunch.runPreviewCheck(instance.value.id, () =>
			updatePreviewQuery.refetch({ throwOnError: true }),
		)
		return result.data ?? null
	}

	function setUnavailable(reason: SharedInstanceUnavailableReason | null) {
		// To its owner a shared instance that is gone is "no longer shared", with a way to share again.
		if (reason && reason !== 'quarantined' && lastRole === 'owner') {
			forcedUnavailableReason.value = null
			ownerProblem.value = 'missing'
			return
		}
		forcedUnavailableReason.value = reason
		if (!reason) ownerProblem.value = null
	}

	/**
	 * Asks the backend whether the shared instance this App owns still exists and belongs to this
	 * account, and updates the state to match. Call it when a sharing surface opens. Returns the
	 * problem it found, if any; a failed connection leaves the state as it was.
	 *
	 * When the instance is linked to another account, only "gone" is taken from the backend: that
	 * offers Stop sharing, so a record pointing at a deleted instance or account can be detached
	 * without deleting the instance, while a working link stays locked to its account.
	 */
	async function verifyOwner() {
		const instanceId = instance.value?.id
		const attachment = instance.value?.shared_instance
		const userId = auth.user.value?.id
		if (attachment?.role !== 'owner' || !userId || offline.value) {
			return ownerProblem.value
		}
		const otherAccount = linkedToOtherAccount.value

		let problem: typeof ownerProblem.value = null
		try {
			const { users } = await apiClient.sharedinstances.instances_v1.getUsers(attachment.id)
			const owner = users.find((user) => user.join_type === 'owner')
			if (owner && owner.id !== userId) problem = 'wrong_account'
		} catch (error) {
			if (!(error instanceof SharedInstancesApiError)) return ownerProblem.value
			if (error.status === 404) problem = 'missing'
			else if (otherAccount) return ownerProblem.value
			// A session the backend no longer accepts is a sign-in problem, not the wrong account.
			else if (error.status === 401 && error.code !== 'invalid_credentials')
				problem = 'wrong_account'
			else return ownerProblem.value
		}

		if (instance.value?.id !== instanceId || auth.user.value?.id !== userId) return null
		ownerProblem.value = problem
		return problem
	}

	/**
	 * Call with the error of any failed sharing action. A shared instance that is gone or belongs to
	 * another account becomes the availability state and gets its own message; everything else goes
	 * to `notifyOther`.
	 */
	async function reportActionError(
		error: unknown,
		notifyOther: (error: unknown) => void = notifyError,
	) {
		const reason = getSharedInstanceUnavailableReason(error)
		if (reason) {
			setUnavailable(reason)
			notifySharedInstanceAvailability(availability.value ?? reason, unavailableManager.value)
			return
		}

		// The App's sharing commands report a refused request as plain text, so ask the backend.
		const problem = isSharedInstancesApiError(error) ? null : await verifyOwner()
		if (problem) notifySharedInstanceAvailability(problem)
		else notifyOther(error)
	}

	/**
	 * Ends sharing for an instance this App owns. The backend's copy is deleted when it still has
	 * one; when it has none, only the local record is detached.
	 */
	async function stopSharing() {
		const current = instance.value
		if (!current) return
		if (current.shared_instance?.role === 'owner') await unpublish_shared_instance(current.id)
		queryClient.setQueryData(instanceKeys.sharedMembers(current.id), [])
		ownerProblem.value = null
	}

	return {
		actionsLocked,
		shareActionsLocked,
		unavailableReason,
		missing,
		availability,
		unavailableManager,
		manager,
		updatePreview,
		lastUpdateCheckAt,
		expectedUserId,
		wrongAccount,
		signedOut,
		eligibilityQuery,
		currentUserCanUseSharedInstances,
		refreshAvailability,
		refreshUpdatePreview,
		setUnavailable,
		verifyOwner,
		reportActionError,
		stopSharing,
	}
}

export type SharedInstanceContext = ReturnType<typeof createSharedInstanceContext>

export const [injectSharedInstance, provideSharedInstance] = createContext<SharedInstanceContext>(
	'InstancePage',
	'sharedInstance',
)
