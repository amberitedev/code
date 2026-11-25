import { core as coreRoutes, type Server } from '@amberite/contracts/core'
import { ModrinthApiError } from '@modrinth/api-client'
import {
	useIsFetching,
	useIsMutating,
	useMutation,
	useQuery,
	useQueryClient,
} from '@tanstack/vue-query'
import { computed, onScopeDispose, type Ref, ref, watch } from 'vue'

import type ServerSharePushModal from '#ui/components/servers/sharing/ServerSharePushModal.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { useServerPermissions } from '#ui/composables/server-permissions'
import { serverKeys, shareHistoryKey } from '#ui/composables/server-queries'
import { injectAuth, injectModrinthServerContext, injectNotificationManager } from '#ui/providers'

import { serverShareableFilesQueryOptions, serverShareDiffQueryOptions } from './query-options'
import { ensureShareable, injectShareUnlinkPrompt, ShareUnlinkDeclined } from './shareable'

export type ServerShareActionTarget = {
	userId: string | undefined
	configPaths: string[]
	/** Configs the share carries that this push stops sharing. */
	removedConfigPaths: string[]
	restart?: boolean
	/** Set once `publish` lands, so a later failure in the same action does not reopen the review. */
	pushed?: boolean
	isCurrent: () => boolean
}

export function useServerShareReview<Action extends string = 'push'>(options?: {
	execute: (action: Action, target: ServerShareActionTarget) => Promise<void>
	disabled?: Ref<boolean>
}) {
	const auth = injectAuth()
	const { serverId, core, server, busyReasons, isServerRunning } = injectModrinthServerContext()
	const { canSetup } = useServerPermissions()
	const { handleError } = injectNotificationManager()
	const { formatMessage } = useVIntl()
	const queryClient = useQueryClient()
	const diffModal = ref<InstanceType<typeof ServerSharePushModal>>()
	const previewOpen = ref(false)
	const sharedInstanceId = computed(() => server.value?.shared_instance_id)
	const shareUnlink = injectShareUnlinkPrompt(null)
	let disposed = false
	onScopeDispose(() => {
		disposed = true
	})
	const previewQuery = useQuery(
		computed(() => ({
			...serverShareDiffQueryOptions(core, serverId, isServerRunning.value, auth.user.value?.id),
			enabled: false,
		})),
	)
	const shareActions = useIsMutating({ mutationKey: ['servers', 'share-action', serverId] })
	const sharePreviews = useIsFetching({ queryKey: serverKeys.shareDiff(serverId) })
	const actionMutation = useMutation({
		mutationKey: ['servers', 'share-action', serverId],
		mutationFn: async ({
			action,
			reviewed,
			target,
		}: {
			action: Action
			reviewed: boolean
			target: ServerShareActionTarget
		}) => {
			if (
				!target.isCurrent() ||
				busyReasons.value.length ||
				(reviewed && (!previewOpen.value || !canSetup.value))
			)
				return
			if (options) await options.execute(action, target)
			else await publish(target)
		},
	})
	const pending = computed(
		() =>
			actionMutation.isPending.value ||
			shareActions.value > 0 ||
			sharePreviews.value > 0 ||
			busyReasons.value.length > 0 ||
			!!options?.disabled?.value,
	)

	function currentTarget(): ServerShareActionTarget | undefined {
		const userId = auth.user.value?.id
		if (disposed) return
		return {
			userId,
			configPaths: [],
			removedConfigPaths: [],
			isCurrent: () => !disposed && auth.user.value?.id === userId,
		}
	}

	/** Warms the shareable files cache so the review's file tree is ready when it opens. */
	function prefetchFiles() {
		if (!canSetup.value) return
		void queryClient.prefetchQuery({
			...serverShareableFilesQueryOptions(core, serverId, auth.user.value?.id),
			staleTime: 30_000,
		})
	}

	// A share action, so every share button on the server waits while it asks and unlinks.
	const unlinkMutation = useMutation({
		mutationKey: ['servers', 'share-action', serverId],
		mutationFn: async (current: Server) => {
			if (await ensureShareable(core, current, shareUnlink))
				await queryClient.invalidateQueries({ queryKey: serverKeys.detail(serverId) })
		},
	})

	async function unlinkUploadedPack() {
		if (server.value?.uploaded_modpack) await unlinkMutation.mutateAsync(server.value)
	}

	/**
	 * Opens the diff review for the current world. When the shared instance has no ready
	 * version yet there is nothing to diff against, so `action` runs straight away instead.
	 */
	async function showPreview(action = 'push' as Action) {
		if (!canSetup.value || pending.value) return
		const target = currentTarget()
		if (!target) return
		try {
			await unlinkUploadedPack()
		} catch (error) {
			if (!(error instanceof ShareUnlinkDeclined)) handleError(error)
			return
		}
		if (!target.isCurrent()) return
		previewOpen.value = true
		prefetchFiles()
		const result = await previewQuery.refetch()
		if (!previewOpen.value || !target.isCurrent()) return
		if (result.error instanceof ModrinthApiError && result.error.statusCode === 404) {
			previewOpen.value = false
			await executeAction(action, target)
		} else if (result.error) {
			previewOpen.value = false
			handleError(result.error)
		} else diffModal.value?.show()
	}

	async function publish(target: ServerShareActionTarget) {
		if (!canSetup.value) throw new Error(formatMessage(messages.permission))
		await unlinkUploadedPack()
		const shared = await core.call(coreRoutes.ShareServer, {
			params: { id: serverId },
			body: {
				file_paths: target.configPaths,
				removed_paths: target.removedConfigPaths,
				restart: target.restart ?? false,
			},
		})
		target.pushed = true
		await Promise.all([
			queryClient.invalidateQueries({ queryKey: serverKeys.detail(serverId) }),
			queryClient.invalidateQueries({ queryKey: serverKeys.shareDiff(serverId) }),
			queryClient.invalidateQueries({ queryKey: shareHistoryKey(serverId) }),
		])
		return shared
	}

	/**
	 * Runs `action` for the current world, pushing `configPaths` alongside the world's content
	 * and no longer sharing `removedConfigPaths`. `restart` decides whether a push restarts the
	 * server to load server-side changes.
	 */
	async function runAction(
		action: Action,
		reviewed = false,
		configPaths: string[] = [],
		restart?: boolean,
		removedConfigPaths: string[] = [],
	) {
		if (pending.value) return
		const target = currentTarget()
		if (!target) return
		target.configPaths = configPaths
		target.removedConfigPaths = removedConfigPaths
		target.restart = restart
		await executeAction(action, target, reviewed)
	}

	async function executeAction(action: Action, target: ServerShareActionTarget, reviewed = false) {
		try {
			await actionMutation.mutateAsync({ action, reviewed, target })
			if (target.isCurrent()) previewOpen.value = false
		} catch (error) {
			if (error instanceof ShareUnlinkDeclined) {
				if (target.isCurrent()) previewOpen.value = false
				return
			}
			handleError(error)
			if (!reviewed || !previewOpen.value || !target.isCurrent()) return
			// The review holds the diff from before the push. Once the push landed it stays closed;
			// otherwise it reopens on a fresh diff, since a failed request may still have pushed.
			if (target.pushed) {
				previewOpen.value = false
				return
			}
			const fresh = await previewQuery.refetch()
			if (!previewOpen.value || !target.isCurrent()) return
			if (fresh.error) previewOpen.value = false
			else diffModal.value?.show({ keepSelection: true })
		}
	}

	watch([sharedInstanceId, () => auth.user.value?.id], () => {
		previewOpen.value = false
		diffModal.value?.hide()
	})

	return {
		diffModal,
		previewOpen,
		previewQuery,
		shareActions,
		sharePreviews,
		actionMutation,
		pending,
		showPreview,
		prefetchFiles,
		publish,
		runAction,
	}
}

const messages = defineMessages({
	permission: {
		id: 'servers.play.permission',
		defaultMessage: 'You do not have permission to share this world.',
	},
})
